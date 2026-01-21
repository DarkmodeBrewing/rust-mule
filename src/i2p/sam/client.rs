use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

pub struct SamClient {
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
}

impl SamClient {
    pub async fn connect(host: &str, port: u16) -> anyhow::Result<Self> {
        let addr = format!("{host}:{port}");
        let stream = TcpStream::connect(&addr)
            .await
            .with_context(|| format!("Failed to connect to SAM at {addr}"))?;

        let (read_half, write_half) = stream.into_split();
        Ok(Self {
            reader: BufReader::new(read_half),
            writer: write_half,
        })
    }

    async fn send_line(&mut self, line: &str) -> anyhow::Result<()> {
        // SAM requires CRLF.
        self.writer
            .write_all(format!("{line}\r\n").as_bytes())
            .await
            .with_context(|| format!("Failed to write SAM line: {line}"))?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn read_line(&mut self) -> anyhow::Result<String> {
        let mut buf: String = String::new();
        let n = self.reader.read_line(&mut buf).await?;
        if n == 0 {
            bail!("SAM closed the connection");
        }
        // BufRead keeps the newline; trim it.
        Ok(buf.trim().to_string())
    }

    async fn request(&mut self, line: &str) -> anyhow::Result<SamReply> {
        self.send_line(line).await?;
        let reply: String = self.read_line().await?;
        SamReply::parse(&reply).with_context(|| format!("Bad SAM reply to: {line}"))
    }

    pub async fn hello(&mut self) -> anyhow::Result<()> {
        // Your earlier nc test returned "Timeout waiting for HELLO VERSION"
        // That usually means the client connected but didn’t speak SAM correctly
        // (wrong line ending, wrong command, or mixed channels).
        let reply = self.request("HELLO VERSION MIN=3.0 MAX=3.1").await?;
        let result = reply
            .kv
            .get("RESULT")
            .map(|s: &String| s.as_str())
            .unwrap_or("UNKNOWN");

        if result != "OK" {
            bail!("HELLO failed: {:?}", reply);
        } else {
            tracing::info!("Receieved HELLO from SAM")
        }
        Ok(())
    }

    async fn ping(&mut self) -> anyhow::Result<()> {
        // Not all SAM versions implement PING; if yours doesn’t,
        // we’ll remove this and do DEST GENERATE instead.
        let reply = self.request("PING").await?;
        let result = reply
            .kv
            .get("RESULT")
            .map(|s: &String| s.as_str())
            .unwrap_or("");
        if result.is_empty() {
            // Some SAMs just reply "PONG" without k=v pairs
            // In that case parse() will treat it as verb-only.
            if reply.verb == "PONG" {
                return Ok(());
            }
            bail!("Unexpected PING reply: {:?}", reply);
        }
        if result != "OK" {
            bail!("PING failed: {:?}", reply);
        }
        Ok(())
    }

    async fn stream_session_create(
        &mut self,
        id: &str,
        dest: &str,
        options: &str,
    ) -> anyhow::Result<()> {
        // Options must come last; SAM expects it as "OPTS=key=val key=val"
        // Some implementations accept no OPTS=.
        let line = if options.trim().is_empty() {
            format!("SESSION CREATE STYLE=STREAM ID={id} DESTINATION={dest}")
        } else {
            format!("SESSION CREATE STYLE=STREAM ID={id} DESTINATION={dest} OPTS={options}")
        };

        let reply = self.request(&line).await?;
        let result = reply
            .kv
            .get("RESULT")
            .map(|s: &String| s.as_str())
            .unwrap_or("UNKNOWN");

        if result != "OK" {
            bail!("SESSION CREATE failed: {:?}", reply);
        }
        Ok(())
    }

    async fn dest_generate(&mut self, sig_type: Option<&str>) -> anyhow::Result<(String, String)> {
        // Some SAMs support SIGNATURE_TYPE=...
        // If omitted, default signature type is used.
        let line = match sig_type {
            Some(t) => format!("DEST GENERATE SIGNATURE_TYPE={t}"),
            None => "DEST GENERATE".to_string(),
        };

        let reply = self.request(&line).await?;
        let result = reply
            .kv
            .get("RESULT")
            .map(|s| s.as_str())
            .unwrap_or("UNKNOWN");
        if result != "OK" {
            bail!("DEST GENERATE failed: {:?}", reply);
        }

        let pub_key = reply
            .kv
            .get("PUB")
            .cloned()
            .ok_or_else(|| anyhow!("DEST GENERATE missing PUB: {:?}", reply))?;
        let priv_key = reply
            .kv
            .get("PRIV")
            .cloned()
            .ok_or_else(|| anyhow!("DEST GENERATE missing PRIV: {:?}", reply))?;

        Ok((pub_key, priv_key))
    }
}

#[derive(Debug)]
struct SamReply {
    verb: String,
    // key/value pairs parsed from the reply
    kv: HashMap<String, String>,
    // raw reply line (useful for debugging)
    raw: String,
}

impl SamReply {
    fn parse(line: &str) -> anyhow::Result<Self> {
        let raw = line.to_string();
        let mut parts = line.split_whitespace();

        let verb = parts
            .next()
            .ok_or_else(|| anyhow!("Empty reply"))?
            .to_string();

        let mut kv = HashMap::new();
        for p in parts {
            if let Some((k, v)) = p.split_once('=') {
                kv.insert(k.to_string(), v.trim_matches('"').to_string());
            }
        }

        Ok(Self { verb, kv, raw })
    }
}
