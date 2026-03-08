//! Minimal MD4 implementation (RFC 1320) used for iMule-compatible keyword hashes.
//!
//! We keep this local (no extra crates) to avoid dependency/network churn in this project.

use std::io::Read;

#[inline]
fn f(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | (!x & z)
}

#[inline]
fn g(x: u32, y: u32, z: u32) -> u32 {
    (x & y) | (x & z) | (y & z)
}

#[inline]
fn h(x: u32, y: u32, z: u32) -> u32 {
    x ^ y ^ z
}

#[inline]
fn rotl(x: u32, s: u32) -> u32 {
    x.rotate_left(s)
}

#[inline]
fn ff(a: u32, b: u32, c: u32, d: u32, x: u32, s: u32) -> u32 {
    rotl(a.wrapping_add(f(b, c, d)).wrapping_add(x), s)
}

#[inline]
fn gg(a: u32, b: u32, c: u32, d: u32, x: u32, s: u32) -> u32 {
    rotl(
        a.wrapping_add(g(b, c, d))
            .wrapping_add(x)
            .wrapping_add(0x5A82_7999),
        s,
    )
}

#[inline]
fn hh(a: u32, b: u32, c: u32, d: u32, x: u32, s: u32) -> u32 {
    rotl(
        a.wrapping_add(h(b, c, d))
            .wrapping_add(x)
            .wrapping_add(0x6ED9_EBA1),
        s,
    )
}

#[derive(Debug, Clone)]
pub struct Md4 {
    state: [u32; 4],
    buffer: Vec<u8>,
    total_len: u64,
}

impl Default for Md4 {
    fn default() -> Self {
        Self::new()
    }
}

impl Md4 {
    pub fn new() -> Self {
        Self {
            state: [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476],
            buffer: Vec::with_capacity(64),
            total_len: 0,
        }
    }

    pub fn update(&mut self, input: &[u8]) {
        self.total_len = self.total_len.saturating_add(input.len() as u64);
        self.buffer.extend_from_slice(input);

        let full_blocks_len = self.buffer.len() & !63;
        for block in self.buffer[..full_blocks_len].chunks_exact(64) {
            process_block(&mut self.state, block);
        }
        if full_blocks_len > 0 {
            self.buffer.drain(..full_blocks_len);
        }
    }

    pub fn finalize(mut self) -> [u8; 16] {
        let bit_len = self.total_len.saturating_mul(8);
        self.buffer.push(0x80);
        while (self.buffer.len() % 64) != 56 {
            self.buffer.push(0);
        }
        self.buffer.extend_from_slice(&bit_len.to_le_bytes());
        for block in self.buffer.chunks_exact(64) {
            process_block(&mut self.state, block);
        }
        encode_state(self.state)
    }
}

/// Compute an MD4 digest.
///
/// Output matches the standard MD4 digest layout: little-endian A, B, C, D words.
pub fn digest(input: &[u8]) -> [u8; 16] {
    let mut md4 = Md4::new();
    md4.update(input);
    md4.finalize()
}

pub fn digest_reader<R: Read>(reader: &mut R) -> std::io::Result<[u8; 16]> {
    let mut md4 = Md4::new();
    let mut buf = [0u8; 8192];
    loop {
        let read = reader.read(&mut buf)?;
        if read == 0 {
            break;
        }
        md4.update(&buf[..read]);
    }
    Ok(md4.finalize())
}

fn encode_state(state: [u32; 4]) -> [u8; 16] {
    let [a, b, c, d] = state;
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a.to_le_bytes());
    out[4..8].copy_from_slice(&b.to_le_bytes());
    out[8..12].copy_from_slice(&c.to_le_bytes());
    out[12..16].copy_from_slice(&d.to_le_bytes());
    out
}

fn process_block(state: &mut [u32; 4], block: &[u8]) {
    let mut x = [0u32; 16];
    for (i, word) in x.iter_mut().enumerate() {
        let j = i * 4;
        *word = u32::from_le_bytes(block[j..j + 4].try_into().unwrap());
    }

    let [mut a, mut b, mut c, mut d] = *state;
    let (aa, bb, cc, dd) = (a, b, c, d);

    // Round 1.
    a = ff(a, b, c, d, x[0], 3);
    d = ff(d, a, b, c, x[1], 7);
    c = ff(c, d, a, b, x[2], 11);
    b = ff(b, c, d, a, x[3], 19);
    a = ff(a, b, c, d, x[4], 3);
    d = ff(d, a, b, c, x[5], 7);
    c = ff(c, d, a, b, x[6], 11);
    b = ff(b, c, d, a, x[7], 19);
    a = ff(a, b, c, d, x[8], 3);
    d = ff(d, a, b, c, x[9], 7);
    c = ff(c, d, a, b, x[10], 11);
    b = ff(b, c, d, a, x[11], 19);
    a = ff(a, b, c, d, x[12], 3);
    d = ff(d, a, b, c, x[13], 7);
    c = ff(c, d, a, b, x[14], 11);
    b = ff(b, c, d, a, x[15], 19);

    // Round 2.
    a = gg(a, b, c, d, x[0], 3);
    d = gg(d, a, b, c, x[4], 5);
    c = gg(c, d, a, b, x[8], 9);
    b = gg(b, c, d, a, x[12], 13);
    a = gg(a, b, c, d, x[1], 3);
    d = gg(d, a, b, c, x[5], 5);
    c = gg(c, d, a, b, x[9], 9);
    b = gg(b, c, d, a, x[13], 13);
    a = gg(a, b, c, d, x[2], 3);
    d = gg(d, a, b, c, x[6], 5);
    c = gg(c, d, a, b, x[10], 9);
    b = gg(b, c, d, a, x[14], 13);
    a = gg(a, b, c, d, x[3], 3);
    d = gg(d, a, b, c, x[7], 5);
    c = gg(c, d, a, b, x[11], 9);
    b = gg(b, c, d, a, x[15], 13);

    // Round 3.
    a = hh(a, b, c, d, x[0], 3);
    d = hh(d, a, b, c, x[8], 9);
    c = hh(c, d, a, b, x[4], 11);
    b = hh(b, c, d, a, x[12], 15);
    a = hh(a, b, c, d, x[2], 3);
    d = hh(d, a, b, c, x[10], 9);
    c = hh(c, d, a, b, x[6], 11);
    b = hh(b, c, d, a, x[14], 15);
    a = hh(a, b, c, d, x[1], 3);
    d = hh(d, a, b, c, x[9], 9);
    c = hh(c, d, a, b, x[5], 11);
    b = hh(b, c, d, a, x[13], 15);
    a = hh(a, b, c, d, x[3], 3);
    d = hh(d, a, b, c, x[11], 9);
    c = hh(c, d, a, b, x[7], 11);
    b = hh(b, c, d, a, x[15], 15);

    state[0] = a.wrapping_add(aa);
    state[1] = b.wrapping_add(bb);
    state[2] = c.wrapping_add(cc);
    state[3] = d.wrapping_add(dd);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        let mut s = String::new();
        for b in bytes {
            use std::fmt::Write as _;
            let _ = write!(&mut s, "{:02x}", b);
        }
        s
    }

    #[test]
    fn md4_test_vectors() {
        // RFC 1320 test vectors.
        assert_eq!(hex(&digest(b"")), "31d6cfe0d16ae931b73c59d7e0c089c0");
        assert_eq!(hex(&digest(b"a")), "bde52cb31de33e46245e05fbdbd6fb24");
        assert_eq!(hex(&digest(b"abc")), "a448017aaf21d8525fc10ae87aa6729d");
        assert_eq!(
            hex(&digest(b"message digest")),
            "d9130a8164549fe818874806e1c7014b"
        );
        assert_eq!(
            hex(&digest(b"abcdefghijklmnopqrstuvwxyz")),
            "d79e1c308aa5bbcdeea8ed63df412da9"
        );
        assert_eq!(
            hex(&digest(
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
            )),
            "043f8582f241db351ce627e153e7f0e4"
        );
        assert_eq!(
            hex(&digest(
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"
            )),
            "e33b4ddc9c38f2199c3e7b164fcc0536"
        );
    }

    #[test]
    fn md4_digest_reader_matches_digest() {
        let input = b"streamed md4 input".repeat(4096);
        let mut reader = std::io::Cursor::new(input.clone());
        assert_eq!(digest(&input), digest_reader(&mut reader).unwrap());
    }
}
