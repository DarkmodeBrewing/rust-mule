use crate::download::service::BlockRange;

pub const OP_SENDINGPART: u8 = 0x2A;
pub const OP_REQUESTPARTS: u8 = 0x2B;
pub const OP_COMPRESSEDPART: u8 = 0x46;

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
    Truncated { needed: usize, actual: usize },
    InvalidRange { start: u64, end_exclusive: u64 },
    InvalidLength { expected: usize, actual: usize },
    TooManyBlocks(usize),
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
        *s = u64::from_le_bytes(payload[idx..idx + 8].try_into().unwrap());
        idx += 8;
    }
    for e in &mut ends {
        *e = u64::from_le_bytes(payload[idx..idx + 8].try_into().unwrap());
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
    let start = u64::from_le_bytes(payload[16..24].try_into().unwrap());
    let end_exclusive = u64::from_le_bytes(payload[24..32].try_into().unwrap());
    if end_exclusive <= start {
        return Err(ProtocolError::InvalidRange {
            start,
            end_exclusive,
        });
    }
    let expected_data_len = (end_exclusive - start) as usize;
    let actual_data_len = payload.len() - 32;
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
    let start = u64::from_le_bytes(payload[16..24].try_into().unwrap());
    let unpacked_len = u32::from_le_bytes(payload[24..28].try_into().unwrap());
    Ok(CompressedPartPayload {
        file_hash,
        start,
        unpacked_len,
        compressed_data: payload[28..].to_vec(),
    })
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
}
