/// I2P base64 codec.
///
/// I2P uses the same bit-packing as standard base64, but with a different alphabet:
/// `A-Z a-z 0-9 - ~` instead of `+ /`.
/// This matches iMule's `b64codec.h`.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-~";

pub fn encode(input: &[u8]) -> String {
    if input.is_empty() {
        return String::new();
    }

    // 4 chars per 3 bytes, rounded up.
    let out_len = ((input.len() + 2) / 3) * 4;
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
}
