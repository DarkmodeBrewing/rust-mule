use crate::i2p::sam::datagram::build_session_create_datagram_forward;
use crate::i2p::sam::protocol::{SamCommand, SamReply};
use anyhow::{Context, Result, anyhow, bail};
use std::net::IpAddr;
use std::{
    pin::Pin,
    task::{Context as TaskContext, Poll},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, ReadBuf},
    net::TcpStream,
    time::{Duration, timeout},
};

/// A SAM v3 control-channel client (line-based request/reply protocol).
pub struct SamClient {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
    io_timeout: Duration,
    hello_done: bool,
}

impl SamClient {
    pub async fn connect_hello(host: &str, port: u16, min: &str, max: &str) -> Result<Self> {
        let mut c = Self::connect(host, port).await?;
        c.hello(min, max).await?;
        Ok(c)
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

    pub async fn session_create(
        &mut self,
        style: &str,
        name: &str,
        destination: &str,
        options: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<SamReply> {
        let mut cmd = SamCommand::new("SESSION CREATE")
            .arg("STYLE", style)
            .arg("ID", name)
            .arg("DESTINATION", destination);

        for opt in options {
            let opt = opt.as_ref();
            let (k, v) = opt
                .split_once('=')
                .ok_or_else(|| anyhow!("SAM session option must be key=value (got '{opt}')"))?;
            cmd = cmd.arg(k, v);
        }

        let reply = self.send_cmd(cmd, "SESSION").await?;
        reply.require_ok()?;
        Ok(reply)
    }

    pub async fn session_create_datagram_forward(
        &mut self,
        session_id: &str,
        destination_privkey: &str,
        forward_port: u16,
        forward_host: IpAddr,
        options: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<SamReply> {
        let cmd = build_session_create_datagram_forward(
            session_id,
            destination_privkey,
            forward_port,
            forward_host,
            options,
        )?;
        let reply = self.send_cmd(cmd, "SESSION").await?;
        reply.require_ok()?;
        Ok(reply)
    }

    pub async fn naming_register(&mut self, name: &str, destination: &str) -> Result<SamReply> {
        let cmd = SamCommand::new("NAMING REGISTER")
            .arg("NAME", name)
            .arg("DESTINATION", destination);
        let reply = self.send_cmd(cmd, "NAMING").await?;
        reply.require_ok()?;
        Ok(reply)
    }

    pub async fn naming_lookup(&mut self, name: &str) -> anyhow::Result<String> {
        let cmd = SamCommand::new("NAMING LOOKUP").arg("NAME", name);
        let reply = self.send_cmd(cmd, "NAMING").await?;
        reply.require_ok()?;

        let value = reply
            .kv
            .get("VALUE")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("NAMING LOOKUP missing VALUE: {}", reply.raw))?;

        Ok(value)
    }

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
        })
    }

    pub async fn session_create_idempotent(&mut self, name: &str, priv_key: &str) -> Result<()> {
        let create_cmd = SamCommand::new("SESSION CREATE")
            .arg("STYLE", "STREAM")
            .arg("ID", name)
            .arg("DESTINATION", priv_key)
            .arg("i2cp.messageReliability", "BestEffort")
            .arg("i2cp.leaseSetEncType", "4");

        let reply: SamReply = self.send_cmd(create_cmd.clone(), "SESSION").await?;
        match (reply.result(), reply.message()) {
            (Some("OK"), _) => Ok(()),

            (Some("I2P_ERROR"), Some(msg))
                if msg.contains("Session already exists")
                    || msg.contains("Duplicate")
                    || msg.contains("DUPLICATE")
                    || msg.contains("DUPLICATED") =>
            {
                tracing::warn!(session=%name, "SAM session already exists; destroying and retrying");

                // Try destroy, but be tolerant if it's already gone / races.
                let destroy_reply = self
                    .send_cmd(
                        SamCommand::new("SESSION DESTROY").arg("ID", name),
                        "SESSION",
                    )
                    .await?;

                match destroy_reply.result() {
                    Some("OK") => {}
                    // Sometimes SAM returns I2P_ERROR if it doesn't exist; don't treat as fatal here.
                    Some("I2P_ERROR") => {
                        tracing::warn!(
                            session=%name,
                            message=%destroy_reply.message().unwrap_or(""),
                            "SESSION DESTROY returned I2P_ERROR; continuing"
                        );
                    }
                    _ => {
                        tracing::warn!(session=%name, raw=%destroy_reply.raw, "Unexpected DESTROY reply");
                    }
                }

                let reply2 = self.send_cmd(create_cmd, "SESSION").await?;
                reply2.require_ok()
            }

            _ => reply.require_ok(),
        }
    }

    pub fn with_timeout(mut self, d: Duration) -> Self {
        self.io_timeout = d;
        self
    }

    // --- Public API ---

    pub async fn hello(&mut self, min: &str, max: &str) -> Result<SamReply> {
        let cmd = SamCommand::new("HELLO VERSION")
            .arg("MIN", min)
            .arg("MAX", max);
        let reply = self.send_cmd(cmd, "HELLO").await?;
        reply.require_ok()?;
        self.hello_done = true;
        Ok(reply)
    }

    pub async fn ping(&mut self) -> Result<SamReply> {
        let reply = self.send_cmd(SamCommand::new("PING"), "PING").await?;
        reply.require_ok()?;
        Ok(reply)
    }

    pub async fn dest_generate(&mut self) -> Result<(String, String)> {
        let reply: SamReply = self
            .send_cmd(SamCommand::new("DEST GENERATE"), "DEST")
            .await?;
        reply.require_ok()?;

        let priv_key: String = reply
            .kv
            .get("PRIV")
            .cloned()
            .ok_or_else(|| anyhow!("DEST GENERATE missing PRIV"))?;

        let pub_key: String = reply
            .kv
            .get("PUB")
            .cloned()
            .ok_or_else(|| anyhow!("DEST GENERATE missing PUB"))?;

        Ok((priv_key, pub_key))
    }

    pub async fn http_get_over_i2p(
        mut stream: impl AsyncRead + AsyncWrite + Unpin,
        host: &str,
    ) -> anyhow::Result<String> {
        let req = format!(
            "GET / HTTP/1.1\r\nHost: {}\r\nUser-Agent: rust-mule/0.1\r\nConnection: close\r\n\r\n",
            host
        );

        timeout(Duration::from_secs(5), async {
            stream.write_all(req.as_bytes()).await
        })
        .await
        .map_err(|_| anyhow::anyhow!("write_all timed out"))??;

        tracing::info!("HTTP request bytes written={}", req.len());

        let mut buf = Vec::new();
        stream.read_to_end(&mut buf).await?;

        Ok(String::from_utf8_lossy(&buf).to_string())
    }

    // --- Line IO pipeline (control channel) ---

    async fn send_cmd(&mut self, cmd: SamCommand, expected_verb: &str) -> Result<SamReply> {
        // SAM requires HELLO as the first command on each TCP connection.
        if !self.hello_done && expected_verb != "HELLO" {
            bail!("SAM protocol error: HELLO must be the first command on a new connection");
        }

        let line = cmd.to_line();
        let line_dbg = cmd.to_line_redacted();
        tracing::debug!(expected_verb, cmd = %line_dbg, "SAM ->");
        self.send_line_crlf(&line).await?;
        let reply_line: String = self.read_line_timeout_for(&line_dbg).await?;
        let reply: SamReply = SamReply::parse(&reply_line)
            .with_context(|| format!("Bad SAM reply to: {line_dbg} (raw={})", reply_line.trim()))?;
        tracing::debug!(raw = %reply.raw, "SAM <-");

        if reply.verb != expected_verb {
            bail!(
                "Unexpected SAM reply verb. expected={} got={} raw={}",
                expected_verb,
                reply.verb,
                reply.raw
            );
        }

        Ok(reply)
    }

    async fn send_line_crlf(&mut self, line: &str) -> Result<()> {
        let payload: String = format!("{line}\r\n");

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

    async fn read_line_timeout_for(&mut self, waiting_for: &str) -> Result<String> {
        timeout(self.io_timeout, async {
            let mut buf = String::new();
            let n = self.reader.read_line(&mut buf).await?;
            if n == 0 {
                bail!("SAM closed the connection");
            }
            Ok::<String, anyhow::Error>(buf.trim_end_matches(['\r', '\n']).to_string())
        })
        .await
        .with_context(|| format!("SAM read timed out waiting for reply to: {waiting_for}"))?
        .context("SAM read failed")
    }
}

/// A SAM STREAM socket after a successful `STREAM CONNECT` or `STREAM ACCEPT`.
///
/// This type preserves any bytes already buffered while reading the SAM status line,
/// which is why we don't simply return a raw `TcpStream`.
pub struct SamStream {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
}

impl SamStream {
    /// Connect a SAM STREAM socket (new TCP connection to the SAM bridge).
    pub async fn connect(host: &str, port: u16, session_name: &str, dest: &str) -> Result<Self> {
        let (mut reader, mut writer) = connect_split(host, port).await?;
        hello_on_socket(&mut reader, &mut writer).await?;

        let cmd = SamCommand::new("STREAM CONNECT")
            .arg("ID", session_name)
            .arg("DESTINATION", dest)
            .to_line();

        send_line_crlf(&mut writer, &cmd).await?;
        let line = read_line(&mut reader).await?;
        let reply =
            SamReply::parse(&line).with_context(|| format!("Bad SAM reply: {}", line.trim()))?;
        if reply.verb != "STREAM" {
            bail!(
                "Unexpected SAM reply verb for STREAM CONNECT: {}",
                reply.raw
            );
        }
        reply.require_ok()?;

        Ok(Self { reader, writer })
    }

    /// Accept an inbound SAM STREAM socket (new TCP connection to the SAM bridge).
    ///
    /// On success, SAM may include `DESTINATION=<peer>` in the STATUS line.
    pub async fn accept(
        host: &str,
        port: u16,
        session_name: &str,
    ) -> Result<(Self, Option<String>)> {
        let (mut reader, mut writer) = connect_split(host, port).await?;
        hello_on_socket(&mut reader, &mut writer).await?;

        let cmd = SamCommand::new("STREAM ACCEPT")
            .arg("ID", session_name)
            .to_line();

        send_line_crlf(&mut writer, &cmd).await?;
        let line = read_line(&mut reader).await?;
        let reply =
            SamReply::parse(&line).with_context(|| format!("Bad SAM reply: {}", line.trim()))?;
        if reply.verb != "STREAM" {
            bail!("Unexpected SAM reply verb for STREAM ACCEPT: {}", reply.raw);
        }
        reply.require_ok()?;

        let peer = reply.kv.get("DESTINATION").cloned();
        Ok((Self { reader, writer }, peer))
    }
}

impl AsyncRead for SamStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.reader).poll_read(cx, buf)
    }
}

impl AsyncWrite for SamStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.writer).poll_write(cx, data)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.writer).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.writer).poll_shutdown(cx)
    }
}

async fn connect_split(
    host: &str,
    port: u16,
) -> Result<(
    BufReader<tokio::net::tcp::OwnedReadHalf>,
    tokio::net::tcp::OwnedWriteHalf,
)> {
    let s: TcpStream = TcpStream::connect((host, port))
        .await
        .with_context(|| format!("SAM connect failed: {host}:{port}"))?;
    let (read_half, write_half) = s.into_split();
    Ok((BufReader::new(read_half), write_half))
}

async fn hello_on_socket(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
) -> Result<()> {
    // SAM requires HELLO as the first command on each TCP connection.
    let cmd = SamCommand::new("HELLO VERSION")
        .arg("MIN", "3.0")
        .arg("MAX", "3.3")
        .to_line();
    send_line_crlf(writer, &cmd).await?;
    let line = read_line(reader).await?;
    let reply =
        SamReply::parse(&line).with_context(|| format!("Bad SAM reply: {}", line.trim()))?;
    if reply.verb != "HELLO" {
        bail!("Unexpected SAM reply verb for HELLO: {}", reply.raw);
    }
    reply.require_ok()
}

async fn send_line_crlf(writer: &mut tokio::net::tcp::OwnedWriteHalf, line: &str) -> Result<()> {
    let payload = format!("{line}\r\n");
    writer
        .write_all(payload.as_bytes())
        .await
        .with_context(|| format!("Failed to write SAM line: {line}"))?;
    writer.flush().await.context("Failed to flush SAM line")?;
    Ok(())
}

async fn read_line(reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> Result<String> {
    let mut buf = String::new();
    let n = reader
        .read_line(&mut buf)
        .await
        .context("Failed to read SAM line")?;
    if n == 0 {
        bail!("SAM closed the connection");
    }
    Ok(buf.trim_end_matches(['\r', '\n']).to_string())
}
