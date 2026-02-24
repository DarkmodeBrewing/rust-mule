use std::collections::HashMap;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{Duration, timeout};

pub type Result<T> = std::result::Result<T, HttpError>;

#[derive(Debug)]
pub enum HttpError {
    WriteTimedOut,
    FlushTimedOut,
    ReadTimedOut,
    Io(std::io::Error),
    InvalidHeaderUtf8(std::str::Utf8Error),
    InvalidResponse(String),
    InvalidStatusCode {
        status_line: String,
        source: std::num::ParseIntError,
    },
    BadChunkSizeLineUtf8(std::str::Utf8Error),
    BadChunkSize {
        value: String,
        source: std::num::ParseIntError,
    },
    ResponseTooLarge {
        max_bytes: usize,
    },
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WriteTimedOut => write!(f, "HTTP write timed out"),
            Self::FlushTimedOut => write!(f, "HTTP flush timed out"),
            Self::ReadTimedOut => write!(f, "HTTP read timed out"),
            Self::Io(_) => write!(f, "HTTP I/O failed"),
            Self::InvalidHeaderUtf8(_) => write!(f, "HTTP headers were not valid UTF-8"),
            Self::InvalidResponse(msg) => write!(f, "{msg}"),
            Self::InvalidStatusCode { status_line, .. } => {
                write!(f, "bad HTTP status code in: {status_line}")
            }
            Self::BadChunkSizeLineUtf8(_) => write!(f, "bad chunk size line utf8"),
            Self::BadChunkSize { value, .. } => write!(f, "bad chunk size: {value}"),
            Self::ResponseTooLarge { max_bytes } => {
                write!(f, "HTTP response exceeded max size of {max_bytes} bytes")
            }
        }
    }
}

impl std::error::Error for HttpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            Self::InvalidHeaderUtf8(source) => Some(source),
            Self::InvalidStatusCode { source, .. } => Some(source),
            Self::BadChunkSizeLineUtf8(source) => Some(source),
            Self::BadChunkSize { source, .. } => Some(source),
            Self::ResponseTooLarge { .. } => None,
            Self::WriteTimedOut
            | Self::FlushTimedOut
            | Self::ReadTimedOut
            | Self::InvalidResponse(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

const MAX_HTTP_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

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
        .map_err(|_| HttpError::WriteTimedOut)?
        .map_err(HttpError::Io)?;

    timeout(io_timeout, async { stream.flush().await })
        .await
        .map_err(|_| HttpError::FlushTimedOut)?
        .map_err(HttpError::Io)?;

    let buf = read_to_end_bounded(&mut stream, io_timeout, MAX_HTTP_RESPONSE_BYTES).await?;

    parse_http_response(&buf)
}

async fn read_to_end_bounded(
    stream: &mut (impl AsyncRead + Unpin),
    io_timeout: Duration,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = timeout(io_timeout, stream.read(&mut chunk))
            .await
            .map_err(|_| HttpError::ReadTimedOut)?
            .map_err(HttpError::Io)?;
        if n == 0 {
            break;
        }
        if out.len().saturating_add(n) > max_bytes {
            return Err(HttpError::ResponseTooLarge { max_bytes });
        }
        out.extend_from_slice(&chunk[..n]);
    }
    Ok(out)
}

fn parse_http_response(raw: &[u8]) -> Result<HttpResponse> {
    let (head, body) = split_http(raw)?;
    let head_str = std::str::from_utf8(head).map_err(HttpError::InvalidHeaderUtf8)?;
    let mut lines = head_str.split("\r\n");

    let status_line = lines
        .next()
        .ok_or_else(|| HttpError::InvalidResponse("missing HTTP status line".to_string()))?;
    let mut parts = status_line.split_whitespace();
    let _http = parts.next().ok_or_else(|| {
        HttpError::InvalidResponse(format!("bad HTTP status line: {status_line}"))
    })?;
    let status: u16 = parts
        .next()
        .ok_or_else(|| HttpError::InvalidResponse(format!("bad HTTP status line: {status_line}")))?
        .parse()
        .map_err(|source| HttpError::InvalidStatusCode {
            status_line: status_line.to_string(),
            source,
        })?;

    let mut headers = HashMap::<String, String>::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (k, v) = line
            .split_once(':')
            .ok_or_else(|| HttpError::InvalidResponse(format!("bad HTTP header line: {line}")))?;
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
    Err(HttpError::InvalidResponse(
        "HTTP response missing header delimiter".to_string(),
    ))
}

fn decode_chunked(mut b: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let line_end = twoway_find(b, b"\r\n").ok_or_else(|| {
            HttpError::InvalidResponse("bad chunked encoding: missing CRLF after size".to_string())
        })?;
        let line = &b[..line_end];
        let line_str = std::str::from_utf8(line).map_err(HttpError::BadChunkSizeLineUtf8)?;
        let size_str = line_str.split(';').next().unwrap_or(line_str).trim();
        let size =
            usize::from_str_radix(size_str, 16).map_err(|source| HttpError::BadChunkSize {
                value: size_str.to_string(),
                source,
            })?;
        b = &b[line_end + 2..];
        if size == 0 {
            // Consume trailer headers until the final CRLF.
            loop {
                let trailer_end = twoway_find(b, b"\r\n").ok_or_else(|| {
                    HttpError::InvalidResponse(
                        "bad chunked encoding: missing CRLF in trailers".to_string(),
                    )
                })?;
                let trailer_line = &b[..trailer_end];
                b = &b[trailer_end + 2..];
                if trailer_line.is_empty() {
                    break;
                }
            }
            break;
        }
        if b.len() < size + 2 {
            return Err(HttpError::InvalidResponse(
                "bad chunked encoding: truncated chunk".to_string(),
            ));
        }
        if &b[size..size + 2] != b"\r\n" {
            return Err(HttpError::InvalidResponse(
                "bad chunked encoding: missing CRLF after chunk payload".to_string(),
            ));
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
    use tokio::io::repeat;

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

    #[test]
    fn rejects_chunked_when_payload_crlf_is_missing() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhelloXX0\r\n\r\n";
        let err = parse_http_response(raw).unwrap_err();
        assert!(matches!(err, HttpError::InvalidResponse(_)));
    }

    #[test]
    fn rejects_chunked_when_final_trailer_crlf_is_missing() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n";
        let err = parse_http_response(raw).unwrap_err();
        assert!(matches!(err, HttpError::InvalidResponse(_)));
    }

    #[tokio::test]
    async fn read_to_end_bounded_rejects_unbounded_stream() {
        let mut stream = repeat(0u8);
        let err = read_to_end_bounded(&mut stream, Duration::from_secs(1), 1024).await;
        assert!(matches!(err, Err(HttpError::ResponseTooLarge { .. })));
    }
}
