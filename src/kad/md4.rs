//! Minimal MD4 implementation (RFC 1320) used for iMule-compatible keyword hashes.
//!
//! We keep this local (no extra crates) to avoid dependency/network churn in this project.

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

/// Compute an MD4 digest.
///
/// Output matches the standard MD4 digest layout: little-endian A, B, C, D words.
pub fn digest(input: &[u8]) -> [u8; 16] {
    let mut a: u32 = 0x6745_2301;
    let mut b: u32 = 0xEFCD_AB89;
    let mut c: u32 = 0x98BA_DCFE;
    let mut d: u32 = 0x1032_5476;

    // MD4 padding: 0x80, then zeros, then 64-bit length (little-endian) in bits.
    let bit_len = (input.len() as u64) * 8;
    let mut msg = Vec::<u8>::with_capacity((input.len() + 9).div_ceil(64) * 64);
    msg.extend_from_slice(input);
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    for block in msg.chunks_exact(64) {
        let mut x = [0u32; 16];
        for (i, word) in x.iter_mut().enumerate() {
            let j = i * 4;
            *word = u32::from_le_bytes(block[j..j + 4].try_into().unwrap());
        }

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

        a = a.wrapping_add(aa);
        b = b.wrapping_add(bb);
        c = c.wrapping_add(cc);
        d = d.wrapping_add(dd);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a.to_le_bytes());
    out[4..8].copy_from_slice(&b.to_le_bytes());
    out[8..12].copy_from_slice(&c.to_le_bytes());
    out[12..16].copy_from_slice(&d.to_le_bytes());
    out
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
}
