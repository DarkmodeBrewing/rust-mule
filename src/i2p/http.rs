use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{Duration, timeout};

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

pub async fn http_get_bytes(
    mut stream: impl AsyncRead + AsyncWrite + Unpin,
    host: &str,
    path: &str,
    io_timeout: Duration,
) -> Result<HttpResponse> {
    let path = if path.is_empty() { "/" } else { path };
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: rust-mule/0.1\r\nConnection: close\r\nAccept: */*\r\n\r\n",
        path, host
    );

    timeout(io_timeout, async { stream.write_all(req.as_bytes()).await })
        .await
        .context("HTTP write timed out")??;

    timeout(io_timeout, async { stream.flush().await })
        .await
        .context("HTTP flush timed out")??;

    let mut buf = Vec::new();
    timeout(io_timeout, async { stream.read_to_end(&mut buf).await })
        .await
        .context("HTTP read timed out")??;

    parse_http_response(&buf)
}

fn parse_http_response(raw: &[u8]) -> Result<HttpResponse> {
    let (head, body) = split_http(raw)?;
    let head_str = std::str::from_utf8(head).context("HTTP headers were not valid UTF-8")?;
    let mut lines = head_str.split("\r\n");

    let status_line = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing HTTP status line"))?;
    let mut parts = status_line.split_whitespace();
    let _http = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("bad HTTP status line: {status_line}"))?;
    let status: u16 = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("bad HTTP status line: {status_line}"))?
        .parse()
        .with_context(|| format!("bad HTTP status code in: {status_line}"))?;

    let mut headers = HashMap::<String, String>::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (k, v) = line
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("bad HTTP header line: {line}"))?;
        headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
    }

    let body = if headers
        .get("transfer-encoding")
        .map(|v| v.to_ascii_lowercase())
        .as_deref()
        == Some("chunked")
    {
        decode_chunked(body)?
    } else {
        body.to_vec()
    };

    Ok(HttpResponse {
        status,
        headers,
        body,
    })
}

fn split_http(raw: &[u8]) -> Result<(&[u8], &[u8])> {
    // Prefer RFC-compliant delimiter.
    if let Some(i) = twoway_find(raw, b"\r\n\r\n") {
        return Ok((&raw[..i], &raw[i + 4..]));
    }
    // Be tolerant of bare-LF responses.
    if let Some(i) = twoway_find(raw, b"\n\n") {
        return Ok((&raw[..i], &raw[i + 2..]));
    }
    bail!("HTTP response missing header delimiter");
}

fn decode_chunked(mut b: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = twoway_find(b, b"\r\n")
            .ok_or_else(|| anyhow::anyhow!("bad chunked encoding: missing CRLF after size"))?;
        let line = &b[..line_end];
        let line_str = std::str::from_utf8(line).context("bad chunk size line utf8")?;
        let size_str = line_str.split(';').next().unwrap_or(line_str).trim();
        let size = usize::from_str_radix(size_str, 16)
            .with_context(|| format!("bad chunk size: {size_str}"))?;
        b = &b[line_end + 2..];
        if size == 0 {
            // Consume trailer headers until CRLF.
            if let Some(trailer_end) = twoway_find(b, b"\r\n\r\n") {
                let _ = trailer_end;
            }
            break;
        }
        if b.len() < size + 2 {
            bail!("bad chunked encoding: truncated chunk");
        }
        out.extend_from_slice(&b[..size]);
        b = &b[size + 2..]; // skip chunk + CRLF
    }
    Ok(out)
}

fn twoway_find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_http_response() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
        let r = parse_http_response(raw).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body, b"hello");
        assert_eq!(r.headers.get("content-length").unwrap(), "5");
    }

    #[test]
    fn parses_chunked_http_response() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n";
        let r = parse_http_response(raw).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body, b"hello");
    }
}
