use crate::download::service::BlockRange;

pub const OP_SENDINGPART: u8 = 0x2A;
pub const OP_REQUESTPARTS: u8 = 0x2B;
pub const OP_COMPRESSEDPART: u8 = 0x46;
pub const MAX_PART_PAYLOAD: usize = 4 * 1024 * 1024;
pub const MAX_COMPRESSED_PAYLOAD: usize = 4 * 1024 * 1024;
pub const MAX_BLOCK_LEN: u64 = MAX_PART_PAYLOAD as u64;

pub type Ed2kFileHash = [u8; 16];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestPartsPayload {
    pub file_hash: Ed2kFileHash,
    pub blocks: Vec<BlockRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendingPartPayload {
    pub file_hash: Ed2kFileHash,
    pub start: u64,
    pub end_exclusive: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompressedPartPayload {
    pub file_hash: Ed2kFileHash,
    pub start: u64,
    pub unpacked_len: u32,
    pub compressed_data: Vec<u8>,
}

#[derive(Debug)]
pub enum ProtocolError {
    Truncated {
        needed: usize,
        actual: usize,
    },
    InvalidRange {
        start: u64,
        end_exclusive: u64,
    },
    InvalidLength {
        expected: usize,
        actual: usize,
    },
    TooManyBlocks(usize),
    PayloadTooLarge {
        kind: &'static str,
        limit: usize,
        actual: usize,
    },
    BlockTooLarge {
        limit: u64,
        actual: u64,
    },
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated { needed, actual } => {
                write!(f, "truncated payload: needed {needed} bytes, got {actual}")
            }
            Self::InvalidRange {
                start,
                end_exclusive,
            } => {
                write!(
                    f,
                    "invalid range: start={start}, end_exclusive={end_exclusive}"
                )
            }
            Self::InvalidLength { expected, actual } => {
                write!(
                    f,
                    "invalid payload length: expected {expected}, got {actual}"
                )
            }
            Self::TooManyBlocks(n) => write!(f, "too many requested blocks: {n}"),
            Self::PayloadTooLarge {
                kind,
                limit,
                actual,
            } => write!(f, "{kind} payload too large: {actual} > {limit}"),
            Self::BlockTooLarge { limit, actual } => {
                write!(f, "block too large: {actual} > {limit}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

pub type Result<T> = std::result::Result<T, ProtocolError>;

pub fn encode_requestparts_payload(
    file_hash: Ed2kFileHash,
    blocks: &[BlockRange],
) -> Result<Vec<u8>> {
    if blocks.len() > 3 {
        return Err(ProtocolError::TooManyBlocks(blocks.len()));
    }
    let mut out = Vec::with_capacity(16 + 8 * 3 * 2);
    out.extend_from_slice(&file_hash);
    for i in 0..3 {
        let start = blocks.get(i).map_or(0, |b| b.start);
        out.extend_from_slice(&start.to_le_bytes());
    }
    for i in 0..3 {
        let end_exclusive = blocks.get(i).map_or(0, |b| b.end.saturating_add(1));
        out.extend_from_slice(&end_exclusive.to_le_bytes());
    }
    Ok(out)
}

pub fn decode_requestparts_payload(payload: &[u8]) -> Result<RequestPartsPayload> {
    const EXPECTED: usize = 16 + 8 * 3 * 2;
    if payload.len() != EXPECTED {
        return Err(ProtocolError::InvalidLength {
            expected: EXPECTED,
            actual: payload.len(),
        });
    }
    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[..16]);

    let mut starts = [0u64; 3];
    let mut ends = [0u64; 3];
    let mut idx = 16;
    for s in &mut starts {
        *s = read_u64_le(payload, idx)?;
        idx += 8;
    }
    for e in &mut ends {
        *e = read_u64_le(payload, idx)?;
        idx += 8;
    }
    let mut blocks = Vec::new();
    for i in 0..3 {
        let start = starts[i];
        let end_exclusive = ends[i];
        if start == 0 && end_exclusive == 0 {
            continue;
        }
        if end_exclusive <= start {
            return Err(ProtocolError::InvalidRange {
                start,
                end_exclusive,
            });
        }
        blocks.push(BlockRange {
            start,
            end: end_exclusive - 1,
        });
    }

    Ok(RequestPartsPayload { file_hash, blocks })
}

pub fn decode_sendingpart_payload(payload: &[u8]) -> Result<SendingPartPayload> {
    if payload.len() < 32 {
        return Err(ProtocolError::Truncated {
            needed: 32,
            actual: payload.len(),
        });
    }
    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[..16]);
    let start = read_u64_le(payload, 16)?;
    let end_exclusive = read_u64_le(payload, 24)?;
    if end_exclusive <= start {
        return Err(ProtocolError::InvalidRange {
            start,
            end_exclusive,
        });
    }
    let block_len = end_exclusive - start;
    if block_len > MAX_BLOCK_LEN {
        return Err(ProtocolError::BlockTooLarge {
            limit: MAX_BLOCK_LEN,
            actual: block_len,
        });
    }
    let expected_data_len = block_len as usize;
    if expected_data_len > MAX_PART_PAYLOAD {
        return Err(ProtocolError::PayloadTooLarge {
            kind: "sendingpart",
            limit: MAX_PART_PAYLOAD,
            actual: expected_data_len,
        });
    }
    let actual_data_len = payload.len() - 32;
    if actual_data_len > MAX_PART_PAYLOAD {
        return Err(ProtocolError::PayloadTooLarge {
            kind: "sendingpart",
            limit: MAX_PART_PAYLOAD,
            actual: actual_data_len,
        });
    }
    if expected_data_len != actual_data_len {
        return Err(ProtocolError::InvalidLength {
            expected: expected_data_len + 32,
            actual: payload.len(),
        });
    }

    Ok(SendingPartPayload {
        file_hash,
        start,
        end_exclusive,
        data: payload[32..].to_vec(),
    })
}

pub fn decode_compressedpart_payload(payload: &[u8]) -> Result<CompressedPartPayload> {
    if payload.len() < 28 {
        return Err(ProtocolError::Truncated {
            needed: 28,
            actual: payload.len(),
        });
    }
    let mut file_hash = [0u8; 16];
    file_hash.copy_from_slice(&payload[..16]);
    let start = read_u64_le(payload, 16)?;
    let unpacked_len = read_u32_le(payload, 24)?;
    if u64::from(unpacked_len) > MAX_BLOCK_LEN {
        return Err(ProtocolError::BlockTooLarge {
            limit: MAX_BLOCK_LEN,
            actual: u64::from(unpacked_len),
        });
    }
    let compressed_len = payload.len() - 28;
    if compressed_len > MAX_COMPRESSED_PAYLOAD {
        return Err(ProtocolError::PayloadTooLarge {
            kind: "compressedpart",
            limit: MAX_COMPRESSED_PAYLOAD,
            actual: compressed_len,
        });
    }
    Ok(CompressedPartPayload {
        file_hash,
        start,
        unpacked_len,
        compressed_data: payload[28..].to_vec(),
    })
}

fn read_u64_le(payload: &[u8], offset: usize) -> Result<u64> {
    let slice = payload
        .get(offset..offset + 8)
        .ok_or(ProtocolError::Truncated {
            needed: offset + 8,
            actual: payload.len(),
        })?;
    let arr: [u8; 8] = slice.try_into().map_err(|_| ProtocolError::Truncated {
        needed: offset + 8,
        actual: payload.len(),
    })?;
    Ok(u64::from_le_bytes(arr))
}

fn read_u32_le(payload: &[u8], offset: usize) -> Result<u32> {
    let slice = payload
        .get(offset..offset + 4)
        .ok_or(ProtocolError::Truncated {
            needed: offset + 4,
            actual: payload.len(),
        })?;
    let arr: [u8; 4] = slice.try_into().map_err(|_| ProtocolError::Truncated {
        needed: offset + 4,
        actual: payload.len(),
    })?;
    Ok(u32::from_le_bytes(arr))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requestparts_roundtrip() {
        let hash = [0x11; 16];
        let blocks = vec![
            BlockRange { start: 10, end: 20 },
            BlockRange { start: 30, end: 39 },
        ];
        let encoded = encode_requestparts_payload(hash, &blocks).expect("encode");
        let decoded = decode_requestparts_payload(&encoded).expect("decode");
        assert_eq!(decoded.file_hash, hash);
        assert_eq!(decoded.blocks, blocks);
    }

    #[test]
    fn decode_sendingpart_validates_len_and_range() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0x22; 16]);
        payload.extend_from_slice(&100u64.to_le_bytes());
        payload.extend_from_slice(&104u64.to_le_bytes());
        payload.extend_from_slice(&[1, 2, 3, 4]);
        let decoded = decode_sendingpart_payload(&payload).expect("decode");
        assert_eq!(decoded.start, 100);
        assert_eq!(decoded.end_exclusive, 104);
        assert_eq!(decoded.data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn decode_sendingpart_rejects_block_too_large() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0x22; 16]);
        payload.extend_from_slice(&0u64.to_le_bytes());
        payload.extend_from_slice(&(MAX_BLOCK_LEN + 1).to_le_bytes());
        let err = decode_sendingpart_payload(&payload).expect_err("block too large");
        assert!(matches!(err, ProtocolError::BlockTooLarge { .. }));
    }

    #[test]
    fn decode_compressedpart_rejects_compressed_payload_too_large() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0x22; 16]);
        payload.extend_from_slice(&0u64.to_le_bytes());
        payload.extend_from_slice(&1024u32.to_le_bytes());
        payload.extend(vec![0xAA; MAX_COMPRESSED_PAYLOAD + 1]);
        let err = decode_compressedpart_payload(&payload).expect_err("payload too large");
        assert!(matches!(err, ProtocolError::PayloadTooLarge { .. }));
    }

    #[test]
    fn decode_compressedpart_rejects_unpacked_too_large() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0x22; 16]);
        payload.extend_from_slice(&0u64.to_le_bytes());
        payload.extend_from_slice(&((MAX_BLOCK_LEN + 1) as u32).to_le_bytes());
        let err = decode_compressedpart_payload(&payload).expect_err("block too large");
        assert!(matches!(err, ProtocolError::BlockTooLarge { .. }));
    }
}
