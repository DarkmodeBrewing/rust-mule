use anyhow::{Context, Result, anyhow, bail};
use std::collections::HashMap;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::{Duration, timeout},
};

pub struct SamClient {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
    io_timeout: Duration,
}

impl SamClient {
    pub async fn stream_connect(
        host: &str,
        port: u16,
        session_name: &str,
        dest: &str,
    ) -> anyhow::Result<SamStream> {
        let mut s: TcpStream = TcpStream::connect((host, port)).await?;

        // HELLO must be first on this connection too.
        s.write_all(b"HELLO VERSION MIN=3.0 MAX=3.3\r\n").await?;
        // Read one line reply (you can reuse your parser later; keep it minimal for now)
        let mut buf: Vec<u8> = vec![0u8; 256];
        let n = s.read(&mut buf).await?;
        let reply = String::from_utf8_lossy(&buf[..n]);
        if !reply.contains("RESULT=OK") {
            anyhow::bail!("SAM HELLO failed on stream socket: {}", reply.trim());
        }

        // Now CONNECT; after OK, the socket becomes the data stream
        let cmd = format!("STREAM CONNECT ID={} DESTINATION={}", session_name, dest);
        tracing::info!("SAM Command: {}", cmd);
        tracing::info!(dest_len = dest.len(), "Using destination");
        s.write_all(cmd.as_bytes()).await?;

        // Read reply
        let n = s.read(&mut buf).await?;
        let reply: std::borrow::Cow<'_, str> = String::from_utf8_lossy(&buf[..n]);
        if !reply.contains("RESULT=OK") {
            anyhow::bail!("STREAM CONNECT failed: {}", reply.trim());
        }

        Ok(SamStream { stream: s })
    }

    pub async fn naming_lookup(&mut self, name: &str) -> anyhow::Result<String> {
        let cmd = format!("NAMING LOOKUP NAME={}", name);
        let reply = self.send_cmd_expect_raw(&cmd, "NAMING").await?;

        if reply.kv.get("RESULT").map(String::as_str) != Some("OK") {
            anyhow::bail!("NAMING LOOKUP failed: {}", reply.raw);
        }

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
        })
    }

    pub async fn session_create_idempotent(&mut self, name: &str, priv_key: &str) -> Result<()> {
        let create_cmd: String = format!(
            "SESSION CREATE STYLE=STREAM ID={} DESTINATION={} OPTION=i2cp.leaseSetEncType=4",
            name, priv_key
        );

        let reply: SamReply = self.send_cmd_expect_raw(&create_cmd, "SESSION").await?;
        match (reply.result(), reply.message()) {
            (Some("OK"), _) => return Ok(()),

            (Some("I2P_ERROR"), Some("Session already exists")) => {
                tracing::warn!(session=%name, "SAM session already exists; destroying and retrying");

                // Try destroy, but be tolerant if it's already gone / races.
                let destroy_cmd = format!("SESSION DESTROY ID={}", name);
                let destroy_reply = self.send_cmd_expect_raw(&destroy_cmd, "SESSION").await?;

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

                let reply2 = self.send_cmd_expect_raw(&create_cmd, "SESSION").await?;
                if reply2.result() == Some("OK") {
                    return Ok(());
                }

                bail!(
                    "SESSION CREATE failed after retry: RESULT={:?} MESSAGE={:?} RAW={}",
                    reply2.result(),
                    reply2.message(),
                    reply2.raw
                );
            }

            _ => {
                bail!(
                    "SESSION CREATE failed: RESULT={:?} MESSAGE={:?} RAW={}",
                    reply.result(),
                    reply.message(),
                    reply.raw
                );
            }
        }
    }

    pub fn with_timeout(mut self, d: Duration) -> Self {
        self.io_timeout = d;
        self
    }

    // --- Public API ---

    pub async fn hello(&mut self, min: &str, max: &str) -> Result<SamReply> {
        // More typical than "HELLO VERSION 3.3"
        let cmd = format!("HELLO VERSION MIN={} MAX={}", min, max);
        self.send_cmd_expect_ok(&cmd, "HELLO").await
    }

    pub async fn dest_generate(&mut self) -> Result<(String, String)> {
        let reply: SamReply = self.send_cmd_expect_ok("DEST GENERATE", "DEST").await?;

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
        mut stream: tokio::net::TcpStream,
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

    // --- The one true IO pipeline ---

    pub async fn stream_accept(&mut self, session: &str) -> Result<()> {
        let cmd: String = format!("STREAM ACCEPT ID={}", session);
        let reply: SamReply = self.send_cmd_expect_raw(&cmd, "STREAM").await?;
        if reply.kv.get("RESULT").map(String::as_str) == Some("OK") {
            Ok(())
        } else {
            anyhow::bail!("STREAM ACCEPT failed: {}", reply.raw);
        }
    }

    async fn send_cmd_expect_ok(&mut self, cmd: &str, expected_verb: &str) -> Result<SamReply> {
        let reply: SamReply = self.send_cmd_expect_raw(cmd, expected_verb).await?;

        if let Some(result) = reply.kv.get("RESULT") {
            if result != "OK" {
                let msg = reply.kv.get("MESSAGE").cloned().unwrap_or_default();
                bail!("SAM error: {} {}", result, msg);
            }
        }

        Ok(reply)
    }

    async fn send_cmd_expect_raw(&mut self, cmd: &str, expected_verb: &str) -> Result<SamReply> {
        self.send_line_crlf(cmd).await?;
        let line: String = self.read_line_timeout().await?;
        let reply: SamReply = SamReply::parse(&line)
            .with_context(|| format!("Bad SAM reply to: {cmd} (raw={})", line.trim()))?;

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

#[derive(Debug, Clone)]
pub struct SamReply {
    pub verb: String,                // "HELLO", "SESSION", "DEST"
    pub kind: String,                // "REPLY" / "STATUS"
    pub kv: HashMap<String, String>, // RESULT, MESSAGE, PRIV, PUB, etc.
    pub raw: String,
}

impl SamReply {
    pub fn parse(line: &str) -> Result<Self> {
        let raw = line.to_string();
        let mut parts = raw.split_whitespace();

        let verb = parts
            .next()
            .ok_or_else(|| anyhow!("Empty SAM reply"))?
            .to_string();
        let kind = parts
            .next()
            .ok_or_else(|| anyhow!("SAM reply missing kind"))?
            .to_string();

        let rest = raw.splitn(3, ' ').nth(2).unwrap_or("").trim();
        let kv = parse_kv_pairs(rest)?;

        Ok(Self {
            verb,
            kind,
            kv,
            raw,
        })
    }

    pub fn result(&self) -> Option<&str> {
        self.kv.get("RESULT").map(|s: &String| s.as_str())
    }

    pub fn message(&self) -> Option<&str> {
        self.kv.get("MESSAGE").map(|s: &String| s.as_str())
    }
}

fn parse_kv_pairs(input: &str) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let mut i = 0;
    let bytes = input.as_bytes();

    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' {
            i += 1;
        }
        if i >= bytes.len() {
            return Err(anyhow!("Malformed KV (no '=') in: {}", input));
        }
        let key = &input[key_start..i];
        i += 1; // '='

        let value;
        if i < bytes.len() && bytes[i] == b'"' {
            i += 1;
            let val_start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            if i >= bytes.len() {
                return Err(anyhow!("Unterminated quote in: {}", input));
            }
            value = input[val_start..i].to_string();
            i += 1; // closing quote
        } else {
            let val_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            value = input[val_start..i].to_string();
        }

        map.insert(key.to_string(), value);
    }

    Ok(map)
}

pub struct SamStream {
    pub stream: TcpStream,
}
