use anyhow::{Result, bail};

pub fn inflate_zlib(src: &[u8], max_out: usize) -> Result<Vec<u8>> {
    if src.len() < 2 + 4 {
        bail!("zlib stream too small: {} bytes", src.len());
    }

    let cmf = src[0];
    let flg = src[1];
    let cm = cmf & 0x0F;
    if cm != 8 {
        bail!("unsupported zlib compression method CM={cm}, expected 8 (deflate)");
    }
    let check = (u16::from(cmf) << 8) | u16::from(flg);
    if check % 31 != 0 {
        bail!("bad zlib header check bits");
    }
    if (flg & 0x20) != 0 {
        bail!("zlib preset dictionary (FDICT) is not supported");
    }

    let mut br = BitReader::new(&src[2..]);
    let mut out = Vec::<u8>::new();

    loop {
        let bfinal = br.read_bits(1)? as u8;
        let btype = br.read_bits(2)? as u8;

        match btype {
            0 => decode_stored_block(&mut br, &mut out, max_out)?,
            1 => {
                let litlen = FIXED_LITLEN.get_or_init(fixed_litlen);
                let dist = FIXED_DIST.get_or_init(fixed_dist);
                decode_compressed_block(&mut br, &mut out, litlen, dist, max_out)?
            }
            2 => {
                let (litlen, dist) = decode_dynamic_tables(&mut br)?;
                decode_compressed_block(&mut br, &mut out, &litlen, &dist, max_out)?;
            }
            _ => bail!("invalid deflate block type"),
        }

        if bfinal == 1 {
            break;
        }
    }

    // After the deflate stream, zlib carries Adler-32 (big-endian).
    br.align_to_byte();
    let a0 = br.read_bits(8)? as u8;
    let a1 = br.read_bits(8)? as u8;
    let a2 = br.read_bits(8)? as u8;
    let a3 = br.read_bits(8)? as u8;
    let expected = u32::from_be_bytes([a0, a1, a2, a3]);
    let got = adler32(&out);
    if expected != got {
        bail!("adler32 mismatch: expected={expected:08x} got={got:08x}");
    }

    Ok(out)
}

fn decode_stored_block(br: &mut BitReader<'_>, out: &mut Vec<u8>, max_out: usize) -> Result<()> {
    br.align_to_byte();
    let len = br.read_bits(16)? as u16;
    let nlen = br.read_bits(16)? as u16;
    if len ^ nlen != 0xFFFF {
        bail!("stored block LEN/NLEN mismatch");
    }

    let len = len as usize;
    if out.len().saturating_add(len) > max_out {
        bail!("inflate output exceeds max_out");
    }

    for _ in 0..len {
        out.push(br.read_bits(8)? as u8);
    }
    Ok(())
}

fn decode_compressed_block(
    br: &mut BitReader<'_>,
    out: &mut Vec<u8>,
    litlen: &HuffmanTable,
    dist: &HuffmanTable,
    max_out: usize,
) -> Result<()> {
    loop {
        let sym = litlen.decode(br)? as u16;
        match sym {
            0..=255 => {
                if out.len() + 1 > max_out {
                    bail!("inflate output exceeds max_out");
                }
                out.push(sym as u8);
            }
            256 => break,
            257..=285 => {
                let (len_base, len_extra) = length_code(sym)?;
                let extra = if len_extra == 0 {
                    0
                } else {
                    br.read_bits(len_extra)? as usize
                };
                let length = len_base + extra;

                let dist_sym = dist.decode(br)? as u16;
                let (dist_base, dist_extra) = distance_code(dist_sym)?;
                let dist_extra_val = if dist_extra == 0 {
                    0
                } else {
                    br.read_bits(dist_extra)? as usize
                };
                let distance = dist_base + dist_extra_val;

                copy_from_history(out, distance, length, max_out)?;
            }
            _ => bail!("invalid literal/length symbol {sym}"),
        }
    }
    Ok(())
}

fn decode_dynamic_tables(br: &mut BitReader<'_>) -> Result<(HuffmanTable, HuffmanTable)> {
    let hlit = br.read_bits(5)? as usize + 257;
    let hdist = br.read_bits(5)? as usize + 1;
    let hclen = br.read_bits(4)? as usize + 4;

    // Order in which code-length code lengths are stored.
    const ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];

    let mut clen = [0u8; 19];
    for i in 0..hclen {
        clen[ORDER[i]] = br.read_bits(3)? as u8;
    }
    let clen_tab = HuffmanTable::build(&clen)?;

    let total = hlit + hdist;
    let mut lengths = Vec::<u8>::with_capacity(total);

    while lengths.len() < total {
        let sym = clen_tab.decode(br)? as u16;
        match sym {
            0..=15 => lengths.push(sym as u8),
            16 => {
                if lengths.is_empty() {
                    bail!("repeat previous length with no previous");
                }
                let repeat = br.read_bits(2)? as usize + 3;
                let prev = *lengths.last().unwrap();
                for _ in 0..repeat {
                    lengths.push(prev);
                }
            }
            17 => {
                let repeat = br.read_bits(3)? as usize + 3;
                lengths.extend(std::iter::repeat_n(0, repeat));
            }
            18 => {
                let repeat = br.read_bits(7)? as usize + 11;
                lengths.extend(std::iter::repeat_n(0, repeat));
            }
            _ => bail!("invalid code-length symbol {sym}"),
        }
    }
    lengths.truncate(total);

    let litlen = HuffmanTable::build(&lengths[..hlit])?;
    let dist = HuffmanTable::build(&lengths[hlit..])?;
    Ok((litlen, dist))
}

fn copy_from_history(
    out: &mut Vec<u8>,
    distance: usize,
    length: usize,
    max_out: usize,
) -> Result<()> {
    if distance == 0 {
        bail!("distance cannot be zero");
    }
    if distance > out.len() {
        bail!(
            "distance beyond output history (distance={}, out={})",
            distance,
            out.len()
        );
    }
    if out.len().saturating_add(length) > max_out {
        bail!("inflate output exceeds max_out");
    }

    let start = out.len() - distance;
    for i in 0..length {
        let b = out[start + (i % distance)];
        out.push(b);
    }
    Ok(())
}

fn length_code(sym: u16) -> Result<(usize, u32)> {
    // 257..285
    const BASE: [usize; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const EXTRA: [u32; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];

    if !(257..=285).contains(&sym) {
        bail!("invalid length symbol {sym}");
    }
    let i = (sym - 257) as usize;
    Ok((BASE[i], EXTRA[i]))
}

fn distance_code(sym: u16) -> Result<(usize, u32)> {
    const BASE: [usize; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const EXTRA: [u32; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];

    let i = sym as usize;
    if i >= BASE.len() {
        bail!("invalid distance symbol {sym}");
    }
    Ok((BASE[i], EXTRA[i]))
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;
    for &b in data {
        s1 = (s1 + u32::from(b)) % MOD;
        s2 = (s2 + s1) % MOD;
    }
    (s2 << 16) | s1
}

struct BitReader<'a> {
    b: &'a [u8],
    pos: usize,
    bitbuf: u64,
    bitcnt: u32,
}

impl<'a> BitReader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self {
            b,
            pos: 0,
            bitbuf: 0,
            bitcnt: 0,
        }
    }

    fn fill(&mut self, n: u32) -> Result<()> {
        while self.bitcnt < n {
            let byte = *self
                .b
                .get(self.pos)
                .ok_or_else(|| anyhow::anyhow!("unexpected EOF"))?;
            self.pos += 1;
            self.bitbuf |= (byte as u64) << self.bitcnt;
            self.bitcnt += 8;
        }
        Ok(())
    }

    fn read_bits(&mut self, n: u32) -> Result<u32> {
        self.fill(n)?;
        let v = (self.bitbuf & ((1u64 << n) - 1)) as u32;
        self.bitbuf >>= n;
        self.bitcnt -= n;
        Ok(v)
    }

    fn peek_bits(&mut self, n: u32) -> Result<u32> {
        self.fill(n)?;
        Ok((self.bitbuf & ((1u64 << n) - 1)) as u32)
    }

    fn drop_bits(&mut self, n: u32) -> Result<()> {
        self.fill(n)?;
        self.bitbuf >>= n;
        self.bitcnt -= n;
        Ok(())
    }

    fn align_to_byte(&mut self) {
        let rem = self.bitcnt % 8;
        if rem != 0 {
            // Safe to ignore errors: align is only used when the stream is well-formed.
            let _ = self.drop_bits(rem);
        }
    }
}

#[derive(Clone)]
struct HuffmanTable {
    max_bits: u32,
    entries: Vec<HuffEntry>,
}

#[derive(Clone, Copy)]
struct HuffEntry {
    sym: u16,
    bits: u8,
}

impl HuffmanTable {
    fn build(code_lengths: &[u8]) -> Result<Self> {
        let mut max_len = 0u32;
        for &l in code_lengths {
            max_len = max_len.max(l as u32);
        }
        if max_len == 0 {
            // Degenerate; build a dummy table that will always fail decode.
            return Ok(Self {
                max_bits: 1,
                entries: vec![HuffEntry { sym: 0, bits: 0 }; 2],
            });
        }
        if max_len > 15 {
            bail!("huffman code length too large: {max_len}");
        }

        let max_bits = max_len;
        let size = 1usize << max_bits;
        let mut entries = vec![HuffEntry { sym: 0, bits: 0 }; size];

        let mut bl_count = [0u16; 16];
        for &l in code_lengths {
            if l != 0 {
                bl_count[l as usize] += 1;
            }
        }

        // Canonical codes.
        let mut next_code = [0u16; 16];
        let mut code: u16 = 0;
        for bits in 1..=15 {
            code = code.wrapping_add(bl_count[bits - 1]);
            code <<= 1;
            next_code[bits] = code;
        }
        if (u32::from(next_code[15]) + u32::from(bl_count[15])) > (1u32 << 15) {
            bail!("oversubscribed huffman tree");
        }

        for (sym, &len) in code_lengths.iter().enumerate() {
            let len = len as usize;
            if len == 0 {
                continue;
            }
            let c = next_code[len];
            next_code[len] = next_code[len].wrapping_add(1);

            let r = reverse_bits(c, len) as usize;
            let step = 1usize << len;
            for idx in (r..size).step_by(step) {
                entries[idx] = HuffEntry {
                    sym: sym as u16,
                    bits: len as u8,
                };
            }
        }

        Ok(Self { max_bits, entries })
    }

    fn decode(&self, br: &mut BitReader<'_>) -> Result<u32> {
        let idx = br.peek_bits(self.max_bits)? as usize;
        let e = self.entries[idx];
        if e.bits == 0 {
            bail!("invalid huffman code");
        }
        br.drop_bits(e.bits as u32)?;
        Ok(e.sym as u32)
    }
}

fn reverse_bits(code: u16, len: usize) -> u16 {
    let mut x = code;
    let mut out = 0u16;
    for _ in 0..len {
        out = (out << 1) | (x & 1);
        x >>= 1;
    }
    out
}

fn fixed_litlen() -> HuffmanTable {
    let mut lens = [0u8; 288];
    lens[..144].fill(8);
    lens[144..256].fill(9);
    lens[256..280].fill(7);
    lens[280..288].fill(8);
    HuffmanTable::build(&lens).expect("fixed litlen table must build")
}

fn fixed_dist() -> HuffmanTable {
    let lens = [5u8; 32];
    HuffmanTable::build(&lens).expect("fixed dist table must build")
}

static FIXED_LITLEN: std::sync::OnceLock<HuffmanTable> = std::sync::OnceLock::new();
static FIXED_DIST: std::sync::OnceLock<HuffmanTable> = std::sync::OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inflates_known_zlib_stream_hello() {
        // Generated with: python3 - <<'PY'
        // import zlib, binascii
        // print(binascii.hexlify(zlib.compress(b"hello")).decode())
        // PY
        let hex = "789ccb48cdc9c90700062c0215";
        let src = hex_to_bytes(hex);
        let out = inflate_zlib(&src, 1024).unwrap();
        assert_eq!(out, b"hello");
    }

    #[test]
    fn rejects_bad_adler() {
        let mut src = hex_to_bytes("789ccb48cdc9c90700062c0215");
        *src.last_mut().unwrap() ^= 0xFF;
        let err = inflate_zlib(&src, 1024).unwrap_err();
        assert!(err.to_string().to_lowercase().contains("adler32"));
    }

    fn hex_to_bytes(s: &str) -> Vec<u8> {
        assert!(s.len().is_multiple_of(2));
        let mut out = Vec::with_capacity(s.len() / 2);
        for i in (0..s.len()).step_by(2) {
            out.push(u8::from_str_radix(&s[i..i + 2], 16).unwrap());
        }
        out
    }
}
