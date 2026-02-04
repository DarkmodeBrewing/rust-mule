use anyhow::{Result, anyhow, bail};
use std::collections::HashMap;

/// A parsed SAM reply line, e.g.:
/// `HELLO REPLY RESULT=OK VERSION=3.3`
/// `SESSION STATUS RESULT=I2P_ERROR MESSAGE="Session already exists"`
#[derive(Debug, Clone)]
pub struct SamReply {
    pub verb: String, // "HELLO", "SESSION", "DEST", "NAMING", "STREAM", ...
    pub kind: String, // "REPLY" / "STATUS"
    pub kv: HashMap<String, String>, // RESULT, MESSAGE, PRIV, PUB, VALUE, etc.
    pub raw: String,
}

impl SamReply {
    pub fn parse(line: &str) -> Result<Self> {
        let raw = line.trim_end_matches(['\r', '\n']).to_string();
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

    pub fn is_ok(&self) -> bool {
        self.result() == Some("OK")
    }

    pub fn require_ok(&self) -> Result<()> {
        if self.is_ok() {
            return Ok(());
        }
        bail!(
            "SAM error: verb={} kind={} result={:?} message={:?} raw={}",
            self.verb,
            self.kind,
            self.result(),
            self.message(),
            self.raw
        );
    }
}

/// A SAM command builder for the line-based control protocol.
///
/// We keep this as a tiny helper (instead of hardcoding format strings everywhere)
/// so quoting/escaping rules stay consistent.
#[derive(Debug, Clone)]
pub struct SamCommand {
    pub verb: &'static str,
    /// Ordered arguments. SAM allows repeated keys (notably `OPTION=...`), so we
    /// intentionally do not use a map here.
    pub args: Vec<(String, String)>,
}

impl SamCommand {
    pub fn new(verb: &'static str) -> Self {
        Self {
            verb,
            args: Vec::new(),
        }
    }

    pub fn arg(mut self, k: impl Into<String>, v: impl Into<String>) -> Self {
        self.args.push((k.into(), v.into()));
        self
    }

    /// Render to a single SAM command line (without trailing CRLF).
    pub fn to_line(&self) -> String {
        let mut s = String::new();
        s.push_str(self.verb);

        for (k, v) in &self.args {
            s.push(' ');
            s.push_str(k);
            s.push('=');
            s.push_str(&encode_value(v));
        }

        s
    }
}

fn encode_value(v: &str) -> String {
    // Most SAM values are unquoted tokens (no whitespace). MESSAGE commonly needs quoting.
    let needs_quotes =
        v.bytes().any(|b| b.is_ascii_whitespace()) || v.contains('"') || v.contains('\\');

    if !needs_quotes {
        return v.to_string();
    }

    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for ch in v.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn parse_kv_pairs(input: &str) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let mut i = 0usize;
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

        let value: String;
        if i < bytes.len() && bytes[i] == b'"' {
            i += 1; // opening quote
            let mut out = String::new();
            let mut closed = false;
            while i < bytes.len() {
                match bytes[i] {
                    b'"' => {
                        i += 1; // closing quote
                        closed = true;
                        break;
                    }
                    b'\\' => {
                        i += 1;
                        if i >= bytes.len() {
                            return Err(anyhow!("Unterminated escape in: {}", input));
                        }
                        match bytes[i] {
                            b'"' => out.push('"'),
                            b'\\' => out.push('\\'),
                            other => out.push(other as char), // be permissive
                        }
                        i += 1;
                    }
                    other => {
                        out.push(other as char);
                        i += 1;
                    }
                }
            }
            if !closed {
                return Err(anyhow!("Unterminated quote in: {}", input));
            }
            value = out;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_reply() {
        let r = SamReply::parse("HELLO REPLY RESULT=OK VERSION=3.3").unwrap();
        assert_eq!(r.verb, "HELLO");
        assert_eq!(r.kind, "REPLY");
        assert_eq!(r.result(), Some("OK"));
        assert_eq!(r.kv.get("VERSION").map(String::as_str), Some("3.3"));
    }

    #[test]
    fn parses_quoted_message() {
        let r =
            SamReply::parse("SESSION STATUS RESULT=I2P_ERROR MESSAGE=\"Session already exists\"")
                .unwrap();
        assert_eq!(r.verb, "SESSION");
        assert_eq!(r.kind, "STATUS");
        assert_eq!(r.result(), Some("I2P_ERROR"));
        assert_eq!(r.message(), Some("Session already exists"));
    }

    #[test]
    fn parses_escaped_quotes_in_message() {
        let r =
            SamReply::parse("X STATUS RESULT=I2P_ERROR MESSAGE=\"a \\\"quote\\\" here\"").unwrap();
        assert_eq!(r.message(), Some("a \"quote\" here"));
    }

    #[test]
    fn renders_command_with_quoting() {
        let cmd = SamCommand::new("HELLO VERSION")
            .arg("MIN", "3.0")
            .arg("MAX", "3.3");
        assert_eq!(cmd.to_line(), "HELLO VERSION MIN=3.0 MAX=3.3");

        let cmd2 = SamCommand::new("X").arg("MESSAGE", "hello world");
        assert_eq!(cmd2.to_line(), "X MESSAGE=\"hello world\"");
    }

    #[test]
    fn allows_repeated_keys() {
        let cmd = SamCommand::new("SESSION CREATE")
            .arg("OPTION", "a=b")
            .arg("OPTION", "c=d");
        assert_eq!(cmd.to_line(), "SESSION CREATE OPTION=a=b OPTION=c=d");
    }

    #[test]
    fn rejects_unterminated_quotes() {
        let err = SamReply::parse("X STATUS RESULT=I2P_ERROR MESSAGE=\"oops").unwrap_err();
        assert!(err.to_string().to_lowercase().contains("unterminated"));
    }
}
