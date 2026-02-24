use crate::i2p::sam::SamError;
use crate::i2p::sam::datagram::build_session_create_datagram_forward;
use crate::i2p::sam::protocol::{SamCommand, SamReply};
use std::net::IpAddr;
use std::{
    pin::Pin,
    task::{Context as TaskContext, Poll},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, ReadBuf},
    net::TcpStream,
    time::{Duration, timeout},
};

type Result<T> = std::result::Result<T, SamError>;

/// A SAM v3 control-channel client (line-based request/reply protocol).
pub struct SamClient {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
    io_timeout: Duration,
    hello_done: bool,
}

const MAX_SAM_CONTROL_LINE_LEN: usize = 8 * 1024;

impl SamClient {
    pub async fn connect_hello(host: &str, port: u16, min: &str, max: &str) -> Result<Self> {
        let mut c = Self::connect(host, port).await?;
        c.hello(min, max).await?;
        Ok(c)
    }

    pub async fn session_destroy(&mut self, name: &str) -> Result<SamReply> {
        self.session_destroy_hint_style(name, None).await
    }

    async fn session_destroy_hint_style(
        &mut self,
        name: &str,
        style_hint: Option<&'static str>,
    ) -> Result<SamReply> {
        // SAM versions differ here:
        // - Some accept: `SESSION DESTROY ID=...`
        // - Some require: `SESSION DESTROY STYLE=... ID=...` (router replies `I2P_ERROR` otherwise)
        //
        // So we try the simplest form first and fall back to style-specific destroy.

        if let Some(style) = style_hint {
            let reply = self
                .send_cmd(
                    SamCommand::new("SESSION DESTROY")
                        .arg("STYLE", style)
                        .arg("ID", name),
                    "SESSION",
                )
                .await?;
            if reply.is_ok() {
                return Ok(reply);
            }
            // Fall through to the compatibility path below.
        }

        let reply = self
            .send_cmd(
                SamCommand::new("SESSION DESTROY").arg("ID", name),
                "SESSION",
            )
            .await?;
        if reply.is_ok() {
            return Ok(reply);
        }

        if matches!(reply.message(), Some(msg) if msg.contains("No SESSION STYLE specified")) {
            for style in ["DATAGRAM", "STREAM"] {
                let r = self
                    .send_cmd(
                        SamCommand::new("SESSION DESTROY")
                            .arg("STYLE", style)
                            .arg("ID", name),
                        "SESSION",
                    )
                    .await?;
                if r.is_ok() {
                    return Ok(r);
                }
            }
        }

        reply.require_ok()?;
        unreachable!("require_ok above always returns Err for non-OK replies")
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
            let (k, v) = opt.split_once('=').ok_or_else(|| {
                SamError::protocol(format!(
                    "SAM session option must be key=value (got '{opt}')"
                ))
            })?;
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

    pub async fn naming_lookup(&mut self, name: &str) -> Result<String> {
        let cmd = SamCommand::new("NAMING LOOKUP").arg("NAME", name);
        let reply = self.send_cmd(cmd, "NAMING").await?;
        reply.require_ok()?;

        let value = reply
            .kv
            .get("VALUE")
            .cloned()
            .ok_or_else(|| SamError::BadFrame {
                what: "NAMING LOOKUP missing VALUE",
                raw: reply.raw_redacted(),
            })?;

        Ok(value)
    }

    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        let stream: TcpStream = TcpStream::connect((host, port))
            .await
            .map_err(|e| SamError::io(format!("connect {host}:{port}"), e))?;

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
            .arg("i2cp.messageReliability", "BestEffort");

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
                if let Err(err) = self.session_destroy_hint_style(name, Some("STREAM")).await {
                    tracing::warn!(
                        session=%name,
                        error=%err,
                        "SESSION DESTROY failed; continuing"
                    );
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

        let priv_key: String = reply
            .kv
            .get("PRIV")
            .cloned()
            .ok_or_else(|| SamError::BadFrame {
                what: "DEST GENERATE missing PRIV",
                raw: reply.raw_redacted(),
            })?;

        let pub_key: String = reply
            .kv
            .get("PUB")
            .cloned()
            .ok_or_else(|| SamError::BadFrame {
                what: "DEST GENERATE missing PUB",
                raw: reply.raw_redacted(),
            })?;

        Ok((priv_key, pub_key))
    }

    // --- Line IO pipeline (control channel) ---

    async fn send_cmd(&mut self, cmd: SamCommand, expected_verb: &str) -> Result<SamReply> {
        // SAM requires HELLO as the first command on each TCP connection.
        if !self.hello_done && expected_verb != "HELLO" {
            return Err(SamError::protocol(
                "HELLO must be the first command on a new connection",
            ));
        }

        let line = cmd.to_line();
        let line_dbg = cmd.to_line_redacted();
        tracing::debug!(expected_verb, cmd = %line_dbg, "SAM ->");
        self.send_line_crlf(&line, &line_dbg).await?;
        let reply_line: String = self.read_line_timeout_for(&line_dbg).await?;
        let reply: SamReply = SamReply::parse(&reply_line)?;
        tracing::debug!(raw = %reply.raw_redacted(), "SAM <-");

        if reply.verb != expected_verb {
            return Err(SamError::BadFrame {
                what: "unexpected reply verb",
                raw: reply.raw_redacted(),
            });
        }

        Ok(reply)
    }

    async fn send_line_crlf(&mut self, line: &str, line_dbg: &str) -> Result<()> {
        let payload: String = format!("{line}\r\n");

        match timeout(self.io_timeout, async {
            self.writer
                .write_all(payload.as_bytes())
                .await
                .map_err(|e| SamError::io(format!("write line: {line_dbg}"), e))?;
            self.writer
                .flush()
                .await
                .map_err(|e| SamError::io(format!("flush line: {line_dbg}"), e))?;
            Ok::<(), SamError>(())
        })
        .await
        {
            Ok(r) => r?,
            Err(_) => {
                return Err(SamError::timeout(
                    format!("write line: {line_dbg}"),
                    self.io_timeout,
                ));
            }
        }

        Ok(())
    }

    async fn read_line_timeout_for(&mut self, waiting_for: &str) -> Result<String> {
        match timeout(self.io_timeout, async {
            read_line_capped(&mut self.reader, &format!("waiting_for={waiting_for}")).await
        })
        .await
        {
            Ok(r) => r,
            Err(_) => Err(SamError::timeout(
                format!("read reply waiting_for={waiting_for}"),
                self.io_timeout,
            )),
        }
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
        let reply = SamReply::parse(&line)?;
        if reply.verb != "STREAM" {
            return Err(SamError::BadFrame {
                what: "unexpected reply verb for STREAM CONNECT",
                raw: reply.raw_redacted(),
            });
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
        let reply = SamReply::parse(&line)?;
        if reply.verb != "STREAM" {
            return Err(SamError::BadFrame {
                what: "unexpected reply verb for STREAM ACCEPT",
                raw: reply.raw_redacted(),
            });
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
        .map_err(|e| SamError::io(format!("connect {host}:{port}"), e))?;
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
    let reply = SamReply::parse(&line)?;
    if reply.verb != "HELLO" {
        return Err(SamError::BadFrame {
            what: "unexpected reply verb for HELLO",
            raw: reply.raw_redacted(),
        });
    }
    reply.require_ok()
}

async fn send_line_crlf(writer: &mut tokio::net::tcp::OwnedWriteHalf, line: &str) -> Result<()> {
    let payload = format!("{line}\r\n");
    writer
        .write_all(payload.as_bytes())
        .await
        .map_err(|e| SamError::io("write SAM line", e))?;
    writer
        .flush()
        .await
        .map_err(|e| SamError::io("flush SAM line", e))?;
    Ok(())
}

async fn read_line(reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> Result<String> {
    read_line_capped(reader, "stream line").await
}

async fn read_line_capped(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    details: &str,
) -> Result<String> {
    let mut buf = Vec::new();
    let n = reader
        .read_until(b'\n', &mut buf)
        .await
        .map_err(|e| SamError::io("read SAM line", e))?;
    if n == 0 {
        return Err(SamError::Closed);
    }
    decode_line_bytes(buf, details)
}

fn decode_line_bytes(mut bytes: Vec<u8>, details: &str) -> Result<String> {
    if bytes.len() > MAX_SAM_CONTROL_LINE_LEN {
        return Err(SamError::FramingDesync {
            what: "SAM control line too long",
            details: format!("{details} len={}", bytes.len()),
        });
    }
    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes.pop();
    }

    String::from_utf8(bytes).map_err(|err| SamError::FramingDesync {
        what: "control reply invalid UTF-8",
        details: format!("{details} err={err}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_line_bytes_rejects_oversized_lines() {
        let bytes = vec![b'a'; MAX_SAM_CONTROL_LINE_LEN + 1];
        let err = decode_line_bytes(bytes, "unit").unwrap_err();
        assert!(matches!(err, SamError::FramingDesync { .. }));
    }

    #[test]
    fn decode_line_bytes_rejects_invalid_utf8() {
        let err = decode_line_bytes(vec![0xff, b'\n'], "unit").unwrap_err();
        assert!(matches!(err, SamError::FramingDesync { .. }));
    }

    #[test]
    fn decode_line_bytes_trims_crlf() {
        let line = decode_line_bytes(b"HELLO REPLY RESULT=OK\r\n".to_vec(), "unit").unwrap();
        assert_eq!(line, "HELLO REPLY RESULT=OK");
    }
}
