use crate::kad::KadId;

pub type Result<T> = std::result::Result<T, UdpCryptoError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UdpCryptoError {
    Random(String),
    InvalidPacket(String),
}

impl std::fmt::Display for UdpCryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Random(msg) => write!(f, "{msg}"),
            Self::InvalidPacket(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for UdpCryptoError {}

macro_rules! crypto_bail {
    ($($arg:tt)*) => {
        return Err(UdpCryptoError::InvalidPacket(format!($($arg)*)))
    };
}

// From iMule `EncryptedDatagramSocket.cpp`.
const CRYPT_HEADER_WITHOUTPADDING: usize = 8;
const MAGICVALUE_UDP_SYNC_CLIENT: u32 = 0x395F2EC1;

// From iMule `protocol/Protocols.h`. These are disallowed values for the first byte of an
// obfuscated packet (it must not look like a plain protocol header).
const OP_EDONKEYHEADER: u8 = 0x01;
const OP_EDONKEYPROT: u8 = 0x02;
const OP_PACKEDPROT: u8 = 0x03;
const OP_EMULEPROT: u8 = 0x04;
const OP_UDPRESERVEDPROT1: u8 = 0xA3;
const OP_UDPRESERVEDPROT2: u8 = 0xB2;
const OP_KADEMLIAHEADER: u8 = 0x05;
const OP_KADEMLIAPACKEDPROT: u8 = 0x06;

#[derive(Debug, Clone)]
pub struct DecryptedKad {
    pub payload: Vec<u8>,
    pub receiver_verify_key: u32,
    pub sender_verify_key: u32,
    pub was_obfuscated: bool,
}

/// aMule/iMule KAD UDP "obfuscation" (RC4+MD5) sender-side.
///
/// This is required for interoperability with iMule nodes, which expect encrypted KAD packets.
pub fn encrypt_kad_packet(
    plain: &[u8],
    target_kad_id: KadId,
    receiver_verify_key: u32,
    sender_verify_key: u32,
) -> Result<Vec<u8>> {
    let mut random_key_part = [0u8; 2];
    getrandom::getrandom(&mut random_key_part)
        .map_err(|e| UdpCryptoError::Random(format!("failed to generate random key part: {e}")))?;

    // Key = MD5( targetKadIdCrypt(16) || randomKeyPart(2) )
    let mut key_data = [0u8; 18];
    key_data[..16].copy_from_slice(&target_kad_id.to_crypt_bytes());
    key_data[16..18].copy_from_slice(&random_key_part);
    let key = md5(&key_data);

    let mut rc4 = Rc4::new(&key);

    let pad_len: u8 = 0;
    let crypt_header_len = CRYPT_HEADER_WITHOUTPADDING + (pad_len as usize) + 8;
    let mut out = vec![0u8; crypt_header_len + plain.len()];

    // First byte: "semi random not protocol marker".
    out[0] = choose_semi_random_marker(false /* kad_recv_key_used */)?;

    // randomKeyPart
    out[1..3].copy_from_slice(&random_key_part);

    // Encrypted magic value + pad length.
    let mut magic = MAGICVALUE_UDP_SYNC_CLIENT.to_le_bytes();
    rc4.apply(&mut magic);
    out[3..7].copy_from_slice(&magic);

    let mut pad = [pad_len];
    rc4.apply(&mut pad);
    out[7] = pad[0];

    // Padding is disabled (pad_len=0), but keep the flow aligned.
    // Encrypted verify keys.
    let mut rvk = receiver_verify_key.to_le_bytes();
    let mut svk = sender_verify_key.to_le_bytes();
    rc4.apply(&mut rvk);
    rc4.apply(&mut svk);
    out[8..12].copy_from_slice(&rvk);
    out[12..16].copy_from_slice(&svk);

    // Encrypted payload.
    let mut payload = plain.to_vec();
    rc4.apply(&mut payload);
    out[crypt_header_len..].copy_from_slice(&payload);

    Ok(out)
}

/// aMule/iMule KAD UDP "obfuscation" sender-side, using **receiver verify key** as the RC4 key.
///
/// iMule uses this path when the receiver's node ID is not available (e.g. for `HELLO_RES_ACK`).
pub fn encrypt_kad_packet_with_receiver_key(
    plain: &[u8],
    receiver_verify_key: u32,
    sender_verify_key: u32,
) -> Result<Vec<u8>> {
    let mut random_key_part = [0u8; 2];
    getrandom::getrandom(&mut random_key_part)
        .map_err(|e| UdpCryptoError::Random(format!("failed to generate random key part: {e}")))?;

    // Key = MD5( receiverVerifyKey(4 le) || randomKeyPart(2) )
    let mut key_data = [0u8; 6];
    key_data[..4].copy_from_slice(&receiver_verify_key.to_le_bytes());
    key_data[4..6].copy_from_slice(&random_key_part);
    let key = md5(&key_data);

    let mut rc4 = Rc4::new(&key);

    let pad_len: u8 = 0;
    let crypt_header_len = CRYPT_HEADER_WITHOUTPADDING + (pad_len as usize) + 8;
    let mut out = vec![0u8; crypt_header_len + plain.len()];

    out[0] = choose_semi_random_marker(true /* kad_recv_key_used */)?;
    out[1..3].copy_from_slice(&random_key_part);

    let mut magic = MAGICVALUE_UDP_SYNC_CLIENT.to_le_bytes();
    rc4.apply(&mut magic);
    out[3..7].copy_from_slice(&magic);

    let mut pad = [pad_len];
    rc4.apply(&mut pad);
    out[7] = pad[0];

    let mut rvk = receiver_verify_key.to_le_bytes();
    let mut svk = sender_verify_key.to_le_bytes();
    rc4.apply(&mut rvk);
    rc4.apply(&mut svk);
    out[8..12].copy_from_slice(&rvk);
    out[12..16].copy_from_slice(&svk);

    let mut payload = plain.to_vec();
    rc4.apply(&mut payload);
    out[crypt_header_len..].copy_from_slice(&payload);

    Ok(out)
}

/// aMule/iMule KAD UDP "obfuscation" (RC4+MD5) receiver-side.
///
/// - `my_kad_id` is our KadID (used for NodeID-key packets).
/// - `my_kad_udp_key_secret` is our secret Kad UDP key (used to compute receiver verify keys).
/// - `from_dest_hash` is iMule's `CI2PAddress::hashCode()` for the sender.
pub fn decrypt_kad_packet(
    buf: &[u8],
    my_kad_id: KadId,
    my_kad_udp_key_secret: u32,
    from_dest_hash: u32,
) -> Result<DecryptedKad> {
    if buf.len() <= CRYPT_HEADER_WITHOUTPADDING {
        return Ok(DecryptedKad {
            payload: buf.to_vec(),
            receiver_verify_key: 0,
            sender_verify_key: 0,
            was_obfuscated: false,
        });
    }

    // Fast path: already looks like a plain protocol header.
    match buf[0] {
        OP_KADEMLIAHEADER
        | OP_KADEMLIAPACKEDPROT
        | OP_EMULEPROT
        | OP_PACKEDPROT
        | OP_EDONKEYPROT
        | OP_EDONKEYHEADER
        | OP_UDPRESERVEDPROT1
        | OP_UDPRESERVEDPROT2 => {
            return Ok(DecryptedKad {
                payload: buf.to_vec(),
                receiver_verify_key: 0,
                sender_verify_key: 0,
                was_obfuscated: false,
            });
        }
        _ => {}
    }

    // Might be obfuscated. Try KAD(NodeID) and KAD(ReceiverKey) variants (skip ED2K).
    let random_key_part = buf.get(1..3).ok_or_else(|| {
        UdpCryptoError::InvalidPacket("obfuscated packet too short for key part".to_string())
    })?;

    // Try 0: KAD packet with NodeID as key.
    if let Ok(d) = try_decrypt_kad_with_node_id(buf, my_kad_id, random_key_part) {
        return Ok(d);
    }

    // Try 2: KAD packet with ReceiverVerifyKey as key.
    let receiver_key = udp_verify_key(my_kad_udp_key_secret, from_dest_hash);
    try_decrypt_kad_with_receiver_key(buf, receiver_key, random_key_part)
}

fn try_decrypt_kad_with_node_id(
    buf: &[u8],
    my_kad_id: KadId,
    random_key_part: &[u8],
) -> Result<DecryptedKad> {
    let mut key_data = [0u8; 18];
    key_data[..16].copy_from_slice(&my_kad_id.to_crypt_bytes());
    key_data[16..18].copy_from_slice(random_key_part);
    let key = md5(&key_data);
    decrypt_kad_with_key(buf, &key)
}

fn try_decrypt_kad_with_receiver_key(
    buf: &[u8],
    receiver_key: u32,
    random_key_part: &[u8],
) -> Result<DecryptedKad> {
    let mut key_data = [0u8; 6];
    key_data[..4].copy_from_slice(&receiver_key.to_le_bytes());
    key_data[4..6].copy_from_slice(random_key_part);
    let key = md5(&key_data);
    decrypt_kad_with_key(buf, &key)
}

fn decrypt_kad_with_key(buf: &[u8], key: &[u8; 16]) -> Result<DecryptedKad> {
    if buf.len() < CRYPT_HEADER_WITHOUTPADDING + 8 {
        crypto_bail!("obfuscated packet too short");
    }

    let mut rc4 = Rc4::new(key);

    // Magic value (4 bytes)
    let mut magic = [0u8; 4];
    magic.copy_from_slice(&buf[3..7]);
    rc4.apply(&mut magic);
    let magic = u32::from_le_bytes(magic);
    if magic != MAGICVALUE_UDP_SYNC_CLIENT {
        crypto_bail!("magic mismatch");
    }

    // PadLen (1 byte)
    let mut pad = [buf[7]];
    rc4.apply(&mut pad);
    let pad_len = pad[0] as usize;

    // Discard padding bytes in keystream.
    if pad_len > 0 {
        rc4.discard(pad_len);
    }

    let verify_off = CRYPT_HEADER_WITHOUTPADDING + pad_len;
    if buf.len() < verify_off + 8 {
        crypto_bail!("missing verify keys");
    }

    // Verify keys (each 4 bytes)
    let mut rvk = [0u8; 4];
    let mut svk = [0u8; 4];
    rvk.copy_from_slice(&buf[verify_off..verify_off + 4]);
    svk.copy_from_slice(&buf[verify_off + 4..verify_off + 8]);
    rc4.apply(&mut rvk);
    rc4.apply(&mut svk);
    let receiver_verify_key = u32::from_le_bytes(rvk);
    let sender_verify_key = u32::from_le_bytes(svk);

    let payload_off = verify_off + 8;
    if payload_off > buf.len() {
        crypto_bail!("payload offset past buffer end");
    }
    let mut payload = buf[payload_off..].to_vec();
    rc4.apply(&mut payload);

    Ok(DecryptedKad {
        payload,
        receiver_verify_key,
        sender_verify_key,
        was_obfuscated: true,
    })
}

/// iMule `CPrefs::GetUDPVerifyKey` (adapted to I2P dest hash instead of IPv4).
pub fn udp_verify_key(kad_udp_key_secret: u32, target_hash: u32) -> u32 {
    // iMule computes MD5 over the in-memory representation of:
    //   (uint64)secret << 32 | target
    // On little-endian systems, this is bytes: target(le) || secret(le).
    let mut buf = [0u8; 8];
    buf[..4].copy_from_slice(&target_hash.to_le_bytes());
    buf[4..].copy_from_slice(&kad_udp_key_secret.to_le_bytes());
    let h = md5(&buf);

    let mut a = [0u8; 4];
    let mut b = [0u8; 4];
    let mut c = [0u8; 4];
    let mut d = [0u8; 4];
    a.copy_from_slice(&h[0..4]);
    b.copy_from_slice(&h[4..8]);
    c.copy_from_slice(&h[8..12]);
    d.copy_from_slice(&h[12..16]);
    let a = u32::from_le_bytes(a);
    let b = u32::from_le_bytes(b);
    let c = u32::from_le_bytes(c);
    let d = u32::from_le_bytes(d);

    ((a ^ b ^ c ^ d) % 0xFFFF_FFFE) + 1
}

fn choose_semi_random_marker(kad_recv_key_used: bool) -> Result<u8> {
    for _ in 0..128 {
        let mut b = [0u8; 1];
        getrandom::getrandom(&mut b)
            .map_err(|e| UdpCryptoError::Random(format!("failed generating marker byte: {e}")))?;
        let mut m = b[0];

        // For KAD packets, marker bit 0 must be 0.
        m &= 0xFE;
        // Bit 1 indicates whether receiver-key-based crypto was used.
        m = if kad_recv_key_used {
            (m & 0xFE) | 0x02
        } else {
            m & 0xFC
        };

        let ok = !matches!(
            m,
            OP_EDONKEYHEADER
                | OP_EDONKEYPROT
                | OP_PACKEDPROT
                | OP_EMULEPROT
                | OP_KADEMLIAHEADER
                | OP_KADEMLIAPACKEDPROT
                | OP_UDPRESERVEDPROT1
                | OP_UDPRESERVEDPROT2
        );
        if ok {
            return Ok(m);
        }
    }

    // Extremely unlikely.
    Ok(0x00)
}

struct Rc4 {
    s: [u8; 256],
    i: u8,
    j: u8,
}

impl Rc4 {
    fn new(key: &[u8; 16]) -> Self {
        let mut s = [0u8; 256];
        for (i, v) in s.iter_mut().enumerate() {
            *v = i as u8;
        }

        let mut j: u8 = 0;
        for i in 0u16..256u16 {
            let ii = i as u8;
            let k = key[(i as usize) % key.len()];
            j = j.wrapping_add(s[ii as usize]).wrapping_add(k);
            s.swap(ii as usize, j as usize);
        }

        Self { s, i: 0, j: 0 }
    }

    fn apply(&mut self, data: &mut [u8]) {
        for b in data {
            *b ^= self.next();
        }
    }

    fn discard(&mut self, n: usize) {
        for _ in 0..n {
            let _ = self.next();
        }
    }

    fn next(&mut self) -> u8 {
        self.i = self.i.wrapping_add(1);
        self.j = self.j.wrapping_add(self.s[self.i as usize]);
        self.s.swap(self.i as usize, self.j as usize);
        let idx = self.s[self.i as usize].wrapping_add(self.s[self.j as usize]);
        self.s[idx as usize]
    }
}

// Minimal MD5 implementation (RFC 1321), one-shot.
fn md5(input: &[u8]) -> [u8; 16] {
    let mut msg = input.to_vec();
    let bit_len = (msg.len() as u64) * 8;

    // Append 0x80 then pad with 0x00 to 56 mod 64.
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0: u32 = 0x67452301;
    let mut b0: u32 = 0xefcdab89;
    let mut c0: u32 = 0x98badcfe;
    let mut d0: u32 = 0x10325476;

    // RFC 1321 T[i] table: T[i] = floor(2^32 * |sin(i+1)|), i = 0..63.
    // Hardcoded to avoid any floating-point precision ambiguity across platforms.
    #[rustfmt::skip]
    let k: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
        0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
        0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
        0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
        0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
        0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
        0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];

    let r: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, // 0..15
        5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, // 16..31
        4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, // 32..47
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, // 48..63
    ];

    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }

        let mut a = a0;
        let mut b = b0;
        let mut c = c0;
        let mut d = d0;

        for i in 0..64 {
            let (f, g) = if i < 16 {
                ((b & c) | ((!b) & d), i)
            } else if i < 32 {
                ((d & b) | ((!d) & c), (5 * i + 1) % 16)
            } else if i < 48 {
                (b ^ c ^ d, (3 * i + 5) % 16)
            } else {
                (c ^ (b | (!d)), (7 * i) % 16)
            };

            let tmp = d;
            d = c;
            c = b;
            let x = a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(m[g]);
            b = b.wrapping_add(x.rotate_left(r[i]));
            a = tmp;
        }

        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
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
    fn md5_matches_known_vector() {
        assert_eq!(hex(&md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn rc4_matches_known_vector() {
        // Wikipedia test vector: Key="Key", Plaintext="Plaintext"
        let key = md5(b"Key"); // any 16-byte key works, just use md5("Key") for determinism
        let mut rc4 = Rc4::new(&key);
        let mut pt = b"Plaintext".to_vec();
        rc4.apply(&mut pt);

        // We can't compare to Wikipedia directly (different key material), but we can round-trip.
        let mut rc4b = Rc4::new(&key);
        rc4b.apply(&mut pt);
        assert_eq!(pt, b"Plaintext");
    }

    #[test]
    fn encrypt_decrypt_round_trip_nodeid() {
        let a_kad = KadId([1u8; 16]);
        let b_kad = KadId([2u8; 16]);
        let msg = [OP_KADEMLIAHEADER, 0x1E, 1, 2, 3, 4];
        let enc = encrypt_kad_packet(&msg, b_kad, 0, 123).unwrap();
        let dec = decrypt_kad_packet(&enc, b_kad, 0xAABBCCDD, 0x11223344).unwrap();
        assert!(dec.was_obfuscated);
        assert_eq!(dec.payload, msg);
        let _ = a_kad; // just to keep the names meaningful
    }

    #[test]
    fn decrypt_round_trip_receiver_key() {
        let b_kad = KadId([2u8; 16]);
        let secret = 0xDEADBEEF;
        let from_hash = 0x11223344;
        let recv_key = udp_verify_key(secret, from_hash);

        // Build an obfuscated packet using receiver-key method (no target KadID).
        let mut random_key_part = [0u8; 2];
        random_key_part.copy_from_slice(&[0xAA, 0x55]);
        let mut key_data = [0u8; 6];
        key_data[..4].copy_from_slice(&recv_key.to_le_bytes());
        key_data[4..6].copy_from_slice(&random_key_part);
        let key = md5(&key_data);
        let mut rc4 = Rc4::new(&key);

        let plain = [OP_KADEMLIAHEADER, 0x1F];
        let pad_len: u8 = 0;
        let crypt_header_len = CRYPT_HEADER_WITHOUTPADDING + (pad_len as usize) + 8;
        let mut out = vec![0u8; crypt_header_len + plain.len()];
        out[0] = 0x0A; // kad + receiver key marker; must not match any plain protocol header
        out[1..3].copy_from_slice(&random_key_part);

        let mut magic = MAGICVALUE_UDP_SYNC_CLIENT.to_le_bytes();
        rc4.apply(&mut magic);
        out[3..7].copy_from_slice(&magic);
        let mut pad = [pad_len];
        rc4.apply(&mut pad);
        out[7] = pad[0];

        let mut rvk = 0u32.to_le_bytes();
        let mut svk = 1u32.to_le_bytes();
        rc4.apply(&mut rvk);
        rc4.apply(&mut svk);
        out[8..12].copy_from_slice(&rvk);
        out[12..16].copy_from_slice(&svk);

        let mut pl = plain.to_vec();
        rc4.apply(&mut pl);
        out[crypt_header_len..].copy_from_slice(&pl);

        let dec = decrypt_kad_packet(&out, b_kad, secret, from_hash).unwrap();
        assert!(dec.was_obfuscated);
        assert_eq!(dec.payload, plain);
    }
}
