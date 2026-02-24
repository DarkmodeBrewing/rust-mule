use crate::i2p::sam::protocol::{SamCommand, SamReply};
use crate::i2p::sam::{SamDatagramRecv, SamError};
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
const MAX_SAM_LINE_LEN: usize = 8 * 1024;

impl SamDatagramTcp {
    pub async fn connect(host: &str, port: u16) -> Result<Self, SamError> {
        let stream: TcpStream = TcpStream::connect((host, port))
            .await
            .map_err(|e| SamError::io(format!("connect {host}:{port}"), e))?;
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

    pub async fn hello(&mut self, min: &str, max: &str) -> Result<SamReply, SamError> {
        let cmd = SamCommand::new("HELLO VERSION")
            .arg("MIN", min)
            .arg("MAX", max);
        let reply = self.send_cmd(cmd, "HELLO").await?;
        reply.require_ok()?;
        self.hello_done = true;
        Ok(reply)
    }

    pub async fn session_destroy(&mut self, name: &str) -> Result<SamReply, SamError> {
        // See `SamClient::session_destroy_hint_style` for the rationale: some SAM routers require
        // STYLE=... on DESTROY.
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
            let r = self
                .send_cmd(
                    SamCommand::new("SESSION DESTROY")
                        .arg("STYLE", "DATAGRAM")
                        .arg("ID", name),
                    "SESSION",
                )
                .await?;
            if r.is_ok() {
                return Ok(r);
            }
            r.require_ok()?;
            unreachable!("require_ok above always returns Err for non-OK replies");
        }

        reply.require_ok()?;
        unreachable!("require_ok above always returns Err for non-OK replies")
    }

    pub async fn session_create_datagram(
        &mut self,
        session_id: &str,
        destination_privkey: &str,
        options: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<SamReply, SamError> {
        let mut cmd = SamCommand::new("SESSION CREATE")
            .arg("STYLE", "DATAGRAM")
            .arg("ID", session_id)
            .arg("DESTINATION", destination_privkey);

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
        self.session_id = session_id.to_string();
        Ok(reply)
    }

    /// Send a datagram to a remote destination over the SAM TCP connection.
    pub async fn send_to(&mut self, destination: &str, payload: &[u8]) -> Result<(), SamError> {
        if !self.hello_done {
            return Err(SamError::protocol(
                "HELLO must be the first command on a new connection",
            ));
        }
        if self.session_id.is_empty() {
            return Err(SamError::protocol(
                "must create a DATAGRAM session before sending",
            ));
        }
        ensure_datagram_payload_size(payload.len())?;

        let header = format!(
            "DATAGRAM SEND DESTINATION={} SIZE={}\n",
            destination,
            payload.len()
        );

        match timeout(self.io_timeout, async {
            self.writer
                .write_all(header.as_bytes())
                .await
                .map_err(|e| SamError::io("write DATAGRAM SEND header", e))?;
            self.writer
                .write_all(payload)
                .await
                .map_err(|e| SamError::io("write DATAGRAM SEND payload", e))?;
            self.writer
                .flush()
                .await
                .map_err(|e| SamError::io("flush DATAGRAM SEND", e))?;
            Ok::<(), SamError>(())
        })
        .await
        {
            Ok(r) => r?,
            Err(_) => return Err(SamError::timeout("write DATAGRAM SEND", self.io_timeout)),
        }

        Ok(())
    }

    /// Receive an incoming datagram from the SAM TCP connection.
    pub async fn recv(&mut self) -> Result<SamDatagramRecv, SamError> {
        loop {
            // Intentionally do not apply `io_timeout` here.
            //
            // Higher-level code (e.g. bootstrap loops) should apply its own timeout/deadline
            // around `recv()`. This avoids turning "no traffic yet" into a hard error.
            let line_bytes = self.read_line_blocking_bytes().await?;
            let line = match bytes_to_utf8_line(&line_bytes) {
                Some(s) => s,
                None => {
                    // Non-UTF8 here means the DATAGRAM stream framing is no longer trustworthy
                    // (payload bytes consumed as line data). Force reconnect so we don't spin
                    // dropping frames indefinitely on a desynced stream.
                    return Err(SamError::FramingDesync {
                        what: "invalid UTF-8 line",
                        details: format!(
                            "len={} head_hex={}",
                            line_bytes.len(),
                            hex_head(&line_bytes, 24)
                        ),
                    });
                }
            };

            if let Some(pong_line) = build_pong_line_for_ping(&line) {
                tracing::debug!(raw = %line.trim(), "received SAM PING; sending PONG");
                self.send_line_lf(&pong_line).await?;
                continue;
            }

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
                    return Err(SamError::FramingDesync {
                        what: "DATAGRAM RECEIVED missing DESTINATION",
                        details: reply.raw_redacted(),
                    });
                };

                let Some(size_raw) = reply.kv.get("SIZE") else {
                    return Err(SamError::FramingDesync {
                        what: "DATAGRAM RECEIVED missing SIZE",
                        details: reply.raw_redacted(),
                    });
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
                            .map_err(|e| SamError::io("discard oversized DATAGRAM payload", e))?;
                        remaining -= n;
                    }
                    continue;
                }

                let mut payload = vec![0u8; size];
                self.reader
                    .read_exact(&mut payload)
                    .await
                    .map_err(|e| SamError::io("read DATAGRAM payload", e))?;

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

    async fn send_cmd(
        &mut self,
        cmd: SamCommand,
        expected_verb: &str,
    ) -> Result<SamReply, SamError> {
        // SAM requires HELLO as the first command on each TCP connection.
        if !self.hello_done && expected_verb != "HELLO" {
            return Err(SamError::protocol(
                "HELLO must be the first command on a new connection",
            ));
        }

        let line = cmd.to_line();
        let line_dbg = cmd.to_line_redacted();
        tracing::debug!(expected_verb, cmd = %line_dbg, "SAM(TCP-DGRAM) ->");

        // iMule uses `\n` here; SAM implementations accept both. Avoid `\r` to keep framing strict.
        self.send_line_lf(&line).await?;

        // Read frames until we see the reply we were waiting for. If a datagram shows up, buffer
        // it by logging and dropping it (session creation happens before bootstrap traffic anyway).
        loop {
            let reply_line_bytes = self.read_line_timeout_bytes().await?;
            let Some(reply_line) = bytes_to_utf8_line(&reply_line_bytes) else {
                return Err(SamError::FramingDesync {
                    what: "control reply invalid UTF-8",
                    details: format!(
                        "expected={expected_verb} len={} head_hex={}",
                        reply_line_bytes.len(),
                        hex_head(&reply_line_bytes, 24)
                    ),
                });
            };

            if let Some(pong_line) = build_pong_line_for_ping(&reply_line) {
                tracing::debug!(raw = %reply_line.trim(), "received SAM PING; sending PONG");
                self.send_line_lf(&pong_line).await?;
                continue;
            }

            let reply: SamReply = SamReply::parse(&reply_line).map_err(|e| SamError::BadFrame {
                what: "control reply parse failed",
                raw: format!("to={line_dbg} err={e}"),
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

    async fn send_line_lf(&mut self, line: &str) -> Result<(), SamError> {
        let payload = format!("{line}\n");
        match timeout(self.io_timeout, async {
            self.writer
                .write_all(payload.as_bytes())
                .await
                .map_err(|e| SamError::io("write SAM line", e))?;
            self.writer
                .flush()
                .await
                .map_err(|e| SamError::io("flush SAM line", e))?;
            Ok::<(), SamError>(())
        })
        .await
        {
            Ok(r) => r?,
            Err(_) => return Err(SamError::timeout("write SAM line", self.io_timeout)),
        }
        Ok(())
    }

    async fn read_line_timeout_bytes(&mut self) -> Result<Vec<u8>, SamError> {
        match timeout(self.io_timeout, async {
            let mut buf = Vec::new();
            let n = self
                .reader
                .read_until(b'\n', &mut buf)
                .await
                .map_err(|e| SamError::io("read SAM line", e))?;
            if n == 0 {
                return Err(SamError::Closed);
            }
            if buf.len() > MAX_SAM_LINE_LEN {
                return Err(SamError::FramingDesync {
                    what: "SAM line too long",
                    details: format!("{} bytes", buf.len()),
                });
            }
            Ok::<Vec<u8>, SamError>(trim_crlf_bytes(buf))
        })
        .await
        {
            Ok(r) => r,
            Err(_) => Err(SamError::timeout("read SAM line", self.io_timeout)),
        }
    }

    async fn read_line_blocking_bytes(&mut self) -> Result<Vec<u8>, SamError> {
        let mut buf = Vec::new();
        let n = self
            .reader
            .read_until(b'\n', &mut buf)
            .await
            .map_err(|e| SamError::io("read SAM line", e))?;
        if n == 0 {
            return Err(SamError::Closed);
        }
        if buf.len() > MAX_SAM_LINE_LEN {
            return Err(SamError::FramingDesync {
                what: "SAM line too long",
                details: format!("{} bytes", buf.len()),
            });
        }
        Ok(trim_crlf_bytes(buf))
    }
}

fn trim_crlf_bytes(mut b: Vec<u8>) -> Vec<u8> {
    while matches!(b.last(), Some(b'\n' | b'\r')) {
        b.pop();
    }
    b
}

fn bytes_to_utf8_line(b: &[u8]) -> Option<String> {
    std::str::from_utf8(b).ok().map(|s| s.to_string())
}

fn build_pong_line_for_ping(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("PING") {
        return None;
    }

    let rest = trimmed.strip_prefix("PING").unwrap_or("").trim_start();
    if rest.is_empty() {
        Some("PONG".to_string())
    } else {
        Some(format!("PONG {rest}"))
    }
}

fn ensure_datagram_payload_size(payload_len: usize) -> Result<(), SamError> {
    if payload_len > MAX_DGRAM_TCP_SIZE {
        return Err(SamError::protocol(format!(
            "DATAGRAM payload too large: {payload_len} > {MAX_DGRAM_TCP_SIZE}"
        )));
    }
    Ok(())
}

fn hex_head(b: &[u8], max: usize) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for (i, v) in b.iter().take(max).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(&mut out, "{v:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{build_pong_line_for_ping, ensure_datagram_payload_size};
    use crate::i2p::sam::SamError;

    #[test]
    fn ping_without_payload_maps_to_plain_pong() {
        assert_eq!(build_pong_line_for_ping("PING"), Some("PONG".to_string()));
    }

    #[test]
    fn ping_with_suffix_preserves_suffix() {
        assert_eq!(
            build_pong_line_for_ping("PING 1234 nonce=abcd"),
            Some("PONG 1234 nonce=abcd".to_string())
        );
    }

    #[test]
    fn non_ping_line_returns_none() {
        assert_eq!(build_pong_line_for_ping("SESSION STATUS RESULT=OK"), None);
    }

    #[test]
    fn rejects_oversized_outbound_payload() {
        let err = ensure_datagram_payload_size((64 * 1024) + 1).unwrap_err();
        assert!(matches!(err, SamError::Protocol { .. }));
    }
}
