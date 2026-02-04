use crate::i2p::sam::SamDatagramRecv;
use crate::i2p::sam::protocol::{SamCommand, SamReply};
use anyhow::{Context, Result, anyhow, bail};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};

/// A SAM `STYLE=DATAGRAM` session that sends/receives datagrams over the SAM TCP socket.
///
/// This matches how iMule used SAM: `DATAGRAM SEND ... SIZE=n\n<payload>` and inbound
/// `DATAGRAM RECEIVED ... SIZE=n\n<payload>`.
pub struct SamDatagramTcp {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
    io_timeout: Duration,
    hello_done: bool,
    session_id: String,
}

impl SamDatagramTcp {
    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        let stream: TcpStream = TcpStream::connect((host, port))
            .await
            .with_context(|| format!("SAM connect failed: {host}:{port}"))?;
        let (read_half, write_half) = stream.into_split();
        Ok(Self {
            reader: BufReader::new(read_half),
            writer: write_half,
            io_timeout: Duration::from_secs(8),
            hello_done: false,
            session_id: String::new(),
        })
    }

    pub fn with_timeout(mut self, d: Duration) -> Self {
        self.io_timeout = d;
        self
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub async fn hello(&mut self, min: &str, max: &str) -> Result<SamReply> {
        let cmd = SamCommand::new("HELLO VERSION")
            .arg("MIN", min)
            .arg("MAX", max);
        let reply = self.send_cmd(cmd, "HELLO").await?;
        reply.require_ok()?;
        self.hello_done = true;
        Ok(reply)
    }

    pub async fn session_destroy(&mut self, name: &str) -> Result<SamReply> {
        let reply = self
            .send_cmd(
                SamCommand::new("SESSION DESTROY").arg("ID", name),
                "SESSION",
            )
            .await?;
        reply.require_ok()?;
        Ok(reply)
    }

    pub async fn session_create_datagram(
        &mut self,
        session_id: &str,
        destination_privkey: &str,
        options: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<SamReply> {
        let mut cmd = SamCommand::new("SESSION CREATE")
            .arg("STYLE", "DATAGRAM")
            .arg("ID", session_id)
            .arg("DESTINATION", destination_privkey);

        for opt in options {
            let opt = opt.as_ref();
            let (k, v) = opt
                .split_once('=')
                .ok_or_else(|| anyhow!("SAM session option must be key=value (got '{opt}')"))?;
            cmd = cmd.arg(k, v);
        }

        let reply = self.send_cmd(cmd, "SESSION").await?;
        reply.require_ok()?;
        self.session_id = session_id.to_string();
        Ok(reply)
    }

    /// Send a datagram to a remote destination over the SAM TCP connection.
    pub async fn send_to(&mut self, destination: &str, payload: &[u8]) -> Result<()> {
        if !self.hello_done {
            bail!("SAM protocol error: HELLO must be the first command on a new connection");
        }
        if self.session_id.is_empty() {
            bail!("SAM protocol error: must create a DATAGRAM session before sending");
        }

        let header = format!(
            "DATAGRAM SEND DESTINATION={} SIZE={}\n",
            destination,
            payload.len()
        );

        timeout(self.io_timeout, async {
            self.writer.write_all(header.as_bytes()).await?;
            self.writer.write_all(payload).await?;
            self.writer.flush().await?;
            Result::<()>::Ok(())
        })
        .await
        .context("SAM write timed out")?
        .context("Failed to write DATAGRAM SEND")?;

        Ok(())
    }

    /// Receive an incoming datagram from the SAM TCP connection.
    pub async fn recv(&mut self) -> Result<SamDatagramRecv> {
        loop {
            let line = self.read_line_timeout().await?;
            let reply = SamReply::parse(&line)
                .with_context(|| format!("Bad SAM frame (raw={})", line.trim()))?;

            if reply.verb == "DATAGRAM" && reply.kind == "RECEIVED" {
                let from_destination = reply.kv.get("DESTINATION").cloned().ok_or_else(|| {
                    anyhow!("DATAGRAM RECEIVED missing DESTINATION: {}", reply.raw)
                })?;
                let size: usize = reply
                    .kv
                    .get("SIZE")
                    .ok_or_else(|| anyhow!("DATAGRAM RECEIVED missing SIZE: {}", reply.raw))?
                    .parse()
                    .with_context(|| format!("Bad SIZE in DATAGRAM RECEIVED: {}", reply.raw))?;

                let mut payload = vec![0u8; size];
                timeout(self.io_timeout, async {
                    self.reader.read_exact(&mut payload).await?;
                    Result::<()>::Ok(())
                })
                .await
                .context("SAM read timed out while reading DATAGRAM payload")?
                .context("Failed reading DATAGRAM payload")?;

                return Ok(SamDatagramRecv {
                    from_destination,
                    from_port: None,
                    to_port: None,
                    payload,
                });
            }

            // We expect some control replies while creating the session; after that, most frames
            // should be `DATAGRAM RECEIVED`. Be tolerant and keep going.
            tracing::debug!(raw = %reply.raw, "ignoring unexpected SAM frame on DATAGRAM socket");
        }
    }

    async fn send_cmd(&mut self, cmd: SamCommand, expected_verb: &str) -> Result<SamReply> {
        // SAM requires HELLO as the first command on each TCP connection.
        if !self.hello_done && expected_verb != "HELLO" {
            bail!("SAM protocol error: HELLO must be the first command on a new connection");
        }

        let line = cmd.to_line();
        let line_dbg = cmd.to_line_redacted();
        tracing::debug!(expected_verb, cmd = %line_dbg, "SAM(TCP-DGRAM) ->");

        // iMule uses `\n` here; SAM implementations accept both. Avoid `\r` to keep framing strict.
        self.send_line_lf(&line).await?;

        // Read frames until we see the reply we were waiting for. If a datagram shows up, buffer
        // it by logging and dropping it (session creation happens before bootstrap traffic anyway).
        loop {
            let reply_line = self.read_line_timeout().await?;
            let reply: SamReply = SamReply::parse(&reply_line).with_context(|| {
                format!("Bad SAM reply to: {line_dbg} (raw={})", reply_line.trim())
            })?;
            tracing::debug!(raw = %reply.raw, "SAM(TCP-DGRAM) <-");

            if reply.verb == expected_verb {
                return Ok(reply);
            }

            tracing::debug!(
                expected = expected_verb,
                got = %reply.verb,
                raw = %reply.raw,
                "ignoring out-of-band SAM frame while waiting for reply"
            );
        }
    }

    async fn send_line_lf(&mut self, line: &str) -> Result<()> {
        let payload = format!("{line}\n");
        timeout(self.io_timeout, async {
            self.writer.write_all(payload.as_bytes()).await?;
            self.writer.flush().await?;
            Result::<()>::Ok(())
        })
        .await
        .context("SAM write timed out")?
        .with_context(|| format!("Failed to write SAM line: {line}"))?;
        Ok(())
    }

    async fn read_line_timeout(&mut self) -> Result<String> {
        timeout(self.io_timeout, async {
            let mut buf = String::new();
            let n = self.reader.read_line(&mut buf).await?;
            if n == 0 {
                bail!("SAM closed the connection");
            }
            Ok::<String, anyhow::Error>(buf.trim_end_matches(['\r', '\n']).to_string())
        })
        .await
        .context("SAM read timed out")?
        .context("SAM read failed")
    }
}
