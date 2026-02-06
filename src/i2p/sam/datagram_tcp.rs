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

const MAX_DGRAM_TCP_SIZE: usize = 64 * 1024;

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
            // Intentionally do not apply `io_timeout` here.
            //
            // Higher-level code (e.g. bootstrap loops) should apply its own timeout/deadline
            // around `recv()`. This avoids turning "no traffic yet" into a hard error.
            let line = self.read_line_blocking().await?;
            let reply = match SamReply::parse(&line) {
                Ok(r) => r,
                Err(err) => {
                    // The SAM bridge should never send malformed frames, but in practice we may
                    // occasionally see junk/partial lines (esp. around network hiccups).
                    // Don't crash the whole app; keep reading.
                    tracing::warn!(error = %err, raw = %line.trim(), "bad SAM frame; skipping");
                    continue;
                }
            };

            if reply.verb == "DATAGRAM" && reply.kind == "RECEIVED" {
                let Some(from_destination) = reply.kv.get("DESTINATION").cloned() else {
                    tracing::warn!(raw = %reply.raw_redacted(), "DATAGRAM RECEIVED missing DESTINATION; skipping");
                    continue;
                };

                let Some(size_raw) = reply.kv.get("SIZE") else {
                    tracing::warn!(raw = %reply.raw_redacted(), "DATAGRAM RECEIVED missing SIZE; skipping");
                    continue;
                };

                let size: usize = match size_raw.parse() {
                    Ok(s) => s,
                    Err(err) => {
                        tracing::warn!(error = %err, raw = %reply.raw_redacted(), "bad SIZE in DATAGRAM RECEIVED; skipping");
                        continue;
                    }
                };

                if size > MAX_DGRAM_TCP_SIZE {
                    tracing::warn!(
                        size,
                        max = MAX_DGRAM_TCP_SIZE,
                        raw = %reply.raw_redacted(),
                        "DATAGRAM RECEIVED too large; skipping"
                    );

                    // Best-effort: discard payload to keep framing intact.
                    let mut remaining = size;
                    let mut buf = [0u8; 4096];
                    while remaining > 0 {
                        let n = remaining.min(buf.len());
                        self.reader
                            .read_exact(&mut buf[..n])
                            .await
                            .context("Failed discarding oversized DATAGRAM payload")?;
                        remaining -= n;
                    }
                    continue;
                }

                let mut payload = vec![0u8; size];
                self.reader
                    .read_exact(&mut payload)
                    .await
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
            tracing::debug!(
                raw = %reply.raw_redacted(),
                "ignoring unexpected SAM frame on DATAGRAM socket"
            );
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
            tracing::debug!(raw = %reply.raw_redacted(), "SAM(TCP-DGRAM) <-");

            if reply.verb == expected_verb {
                return Ok(reply);
            }

            tracing::debug!(
                expected = expected_verb,
                got = %reply.verb,
                raw = %reply.raw_redacted(),
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

    async fn read_line_blocking(&mut self) -> Result<String> {
        let mut buf = String::new();
        let n = self
            .reader
            .read_line(&mut buf)
            .await
            .context("Failed to read SAM line")?;
        if n == 0 {
            bail!("SAM closed the connection");
        }
        Ok(buf.trim_end_matches(['\r', '\n']).to_string())
    }
}
