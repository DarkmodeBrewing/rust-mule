#[cfg(test)]
mod tests;

use anyhow::{Context, Result, anyhow};
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamHelloRequest {
    pub min: String,
    pub max: String,
}

impl Default for SamHelloRequest {
    fn default() -> Self {
        // Common SAM v3 range. Adjust later if you want stricter.
        Self {
            min: "3.1".to_string(),
            max: "3.3".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SamHelloResponse {
    pub result: String,          // e.g. "OK" or "NOVERSION"
    pub version: Option<String>, // e.g. "3.3"
    pub message: Option<String>, // optional extra text
    pub kv: HashMap<String, String>,
}

pub async fn hello_version<S>(stream: &mut S, req: SamHelloRequest) -> Result<SamHelloResponse>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    // SAM is line-oriented. Commands end with '\n'.
    // Typical:
    // HELLO VERSION MIN=3.1 MAX=3.3
    // Reply:
    // HELLO REPLY RESULT=OK VERSION=3.3
    let cmd = format!("HELLO VERSION MIN={} MAX={}\n", req.min, req.max);

    stream
        .write_all(cmd.as_bytes())
        .await
        .context("failed to write SAM HELLO VERSION")?;

    stream.flush().await.ok(); // not strictly required, but fine

    // Read one line response
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let bytes = reader
        .read_line(&mut line)
        .await
        .context("failed to read SAM HELLO reply line")?;

    if bytes == 0 {
        return Err(anyhow!("SAM HELLO reply: EOF"));
    }

    let line = line.trim_end_matches(['\r', '\n']).to_string();
    parse_hello_reply(&line)
}

pub fn parse_hello_reply(line: &str) -> Result<SamHelloResponse> {
    // Expect something like:
    // "HELLO REPLY RESULT=OK VERSION=3.3"
    // But we’ll parse defensively.
    let mut parts = line.split_whitespace();

    let p0 = parts.next().ok_or_else(|| anyhow!("empty SAM reply"))?;
    let p1 = parts.next().ok_or_else(|| anyhow!("short SAM reply"))?;

    if p0 != "HELLO" || p1 != "REPLY" {
        return Err(anyhow!("unexpected SAM reply prefix: {line}"));
    }

    let mut kv = HashMap::new();
    for token in parts {
        if let Some((k, v)) = token.split_once('=') {
            kv.insert(k.to_string(), v.to_string());
        }
    }

    let result = kv
        .get("RESULT")
        .cloned()
        .ok_or_else(|| anyhow!("SAM HELLO reply missing RESULT: {line}"))?;

    Ok(SamHelloResponse {
        version: kv.get("VERSION").cloned(),
        message: kv.get("MESSAGE").cloned(),
        kv,
        result,
    })
}
