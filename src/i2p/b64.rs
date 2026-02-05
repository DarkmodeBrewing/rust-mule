/// I2P base64 codec.
///
/// I2P uses the same bit-packing as standard base64, but with a different alphabet:
/// `A-Z a-z 0-9 - ~` instead of `+ /`.
/// This matches iMule's `b64codec.h`.
use std::fmt;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-~";

/// Short, human-friendly formatter for very long I2P base64 destination strings.
///
/// This is meant for logs: it keeps the beginning/end (useful for correlation) without
/// dumping ~500 bytes of destination on every line.
pub fn short(s: &str) -> ShortB64<'_> {
    ShortB64 { s }
}

pub struct ShortB64<'a> {
    s: &'a str,
}

impl fmt::Display for ShortB64<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.s.trim();
        const HEAD: usize = 16;
        const TAIL: usize = 12;

        if s.len() <= HEAD + TAIL + 3 {
            return f.write_str(s);
        }

        // Safe because I2P base64 destinations are ASCII.
        write!(f, "{}...{}", &s[..HEAD], &s[s.len() - TAIL..])
    }
}

pub fn encode(input: &[u8]) -> String {
    if input.is_empty() {
        return String::new();
    }

    // 4 chars per 3 bytes, rounded up.
    let out_len = input.len().div_ceil(3) * 4;
    let mut out = String::with_capacity(out_len);

    let mut i = 0usize;
    while i + 3 <= input.len() {
        let b0 = input[i];
        let b1 = input[i + 1];
        let b2 = input[i + 2];
        i += 3;

        let s0 = (b0 >> 2) & 0x3F;
        let s1 = ((b0 & 0x03) << 4) | (b1 >> 4);
        let s2 = ((b1 & 0x0F) << 2) | (b2 >> 6);
        let s3 = b2 & 0x3F;

        out.push(ALPHABET[s0 as usize] as char);
        out.push(ALPHABET[s1 as usize] as char);
        out.push(ALPHABET[s2 as usize] as char);
        out.push(ALPHABET[s3 as usize] as char);
    }

    let rem = input.len() - i;
    if rem == 1 {
        let b0 = input[i];
        let s0 = (b0 >> 2) & 0x3F;
        let s1 = (b0 & 0x03) << 4;
        out.push(ALPHABET[s0 as usize] as char);
        out.push(ALPHABET[s1 as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let b0 = input[i];
        let b1 = input[i + 1];
        let s0 = (b0 >> 2) & 0x3F;
        let s1 = ((b0 & 0x03) << 4) | (b1 >> 4);
        let s2 = (b1 & 0x0F) << 2;
        out.push(ALPHABET[s0 as usize] as char);
        out.push(ALPHABET[s1 as usize] as char);
        out.push(ALPHABET[s2 as usize] as char);
        out.push('=');
    }

    out
}

pub fn decode(input: &str) -> anyhow::Result<Vec<u8>> {
    let mut rev = [0u8; 256];
    rev.fill(0xFF);
    for (i, &b) in ALPHABET.iter().enumerate() {
        rev[b as usize] = i as u8;
    }

    let mut cleaned = Vec::with_capacity(input.len());
    for b in input.bytes() {
        if b.is_ascii_whitespace() {
            continue;
        }
        cleaned.push(b);
    }

    if cleaned.len() % 4 != 0 {
        anyhow::bail!("invalid base64 length: {}", cleaned.len());
    }

    let mut out = Vec::with_capacity((cleaned.len() / 4) * 3);
    let mut i = 0usize;
    while i < cleaned.len() {
        let c0 = cleaned[i];
        let c1 = cleaned[i + 1];
        let c2 = cleaned[i + 2];
        let c3 = cleaned[i + 3];
        i += 4;

        let v0 = decode_char(&rev, c0)?;
        let v1 = decode_char(&rev, c1)?;

        if c2 == b'=' && c3 == b'=' {
            out.push((v0 << 2) | (v1 >> 4));
            break;
        }

        let v2 = decode_char(&rev, c2)?;
        if c3 == b'=' {
            out.push((v0 << 2) | (v1 >> 4));
            out.push((v1 << 4) | (v2 >> 2));
            break;
        }

        let v3 = decode_char(&rev, c3)?;
        out.push((v0 << 2) | (v1 >> 4));
        out.push((v1 << 4) | (v2 >> 2));
        out.push((v2 << 6) | v3);
    }

    Ok(out)
}

fn decode_char(rev: &[u8; 256], b: u8) -> anyhow::Result<u8> {
    if b == b'=' {
        return Ok(0);
    }
    let v = rev[b as usize];
    if v == 0xFF {
        anyhow::bail!("invalid base64 character: 0x{b:02x}");
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_empty() {
        assert_eq!(encode(&[]), "");
    }

    #[test]
    fn matches_known_mapping_for_plus_slash() {
        // 0xFB, 0xEF, 0xFF encodes to "++//" in standard base64.
        // In I2P base64, '+' -> '-' and '/' -> '~', so we expect "--~~".
        assert_eq!(encode(&[0xFB, 0xEF, 0xFF]), "--~~");
    }

    #[test]
    fn encodes_with_padding() {
        assert_eq!(encode(&[0x00]), "AA==");
        assert_eq!(encode(&[0x00, 0x00]), "AAA=");
    }

    #[test]
    fn decodes_round_trip() {
        let data = b"hello world";
        let enc = encode(data);
        let dec = decode(&enc).unwrap();
        assert_eq!(dec, data);
    }

    #[test]
    fn decodes_known_mapping_for_plus_slash() {
        // "--~~" in I2P base64 == "++//" in standard base64 == bytes [0xFB, 0xEF, 0xFF].
        assert_eq!(decode("--~~").unwrap(), vec![0xFB, 0xEF, 0xFF]);
    }
}
