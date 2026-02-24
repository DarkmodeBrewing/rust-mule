use crate::kad::{KadId, packed};
use std::collections::BTreeMap;

pub type Result<T> = std::result::Result<T, WireError>;

#[derive(Debug)]
pub enum WireError {
    Inflate(packed::InflateError),
    UnexpectedEof { offset: usize },
    InvalidFormat(String),
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inflate(source) => write!(f, "{source}"),
            Self::UnexpectedEof { offset } => write!(f, "unexpected EOF at {offset}"),
            Self::InvalidFormat(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for WireError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inflate(source) => Some(source),
            Self::UnexpectedEof { .. } | Self::InvalidFormat(_) => None,
        }
    }
}

impl From<packed::InflateError> for WireError {
    fn from(value: packed::InflateError) -> Self {
        Self::Inflate(value)
    }
}

macro_rules! wire_bail {
    ($($arg:tt)*) => {
        return Err(WireError::InvalidFormat(format!($($arg)*)))
    };
}

pub const OP_KADEMLIAHEADER: u8 = 0x05;
pub const OP_KADEMLIAPACKEDPROT: u8 = 0x06;

// Kademlia2Opcodes from iMule/aMule headers.
pub const KADEMLIA2_BOOTSTRAP_REQ: u8 = 0x0D;
pub const KADEMLIA2_BOOTSTRAP_RES: u8 = 0x0E;
pub const KADEMLIA2_HELLO_REQ: u8 = 0x0F;
pub const KADEMLIA2_HELLO_RES: u8 = 0x10;
pub const KADEMLIA2_REQ: u8 = 0x11;
pub const KADEMLIA2_HELLO_RES_ACK: u8 = 0x12;
pub const KADEMLIA2_RES: u8 = 0x13;
pub const KADEMLIA2_SEARCH_KEY_REQ: u8 = 0x14;
pub const KADEMLIA2_SEARCH_SOURCE_REQ: u8 = 0x15;
pub const KADEMLIA2_SEARCH_NOTES_REQ: u8 = 0x16;
pub const KADEMLIA2_SEARCH_RES: u8 = 0x17;
pub const KADEMLIA2_PUBLISH_KEY_REQ: u8 = 0x18;
pub const KADEMLIA2_PUBLISH_SOURCE_REQ: u8 = 0x19;
pub const KADEMLIA2_PUBLISH_NOTES_REQ: u8 = 0x1A;
pub const KADEMLIA2_PUBLISH_RES: u8 = 0x1B;
pub const KADEMLIA2_PUBLISH_RES_ACK: u8 = 0x1C;
pub const KADEMLIA2_PING: u8 = 0x1E;
pub const KADEMLIA2_PONG: u8 = 0x1F;

// Kademlia v1 (deprecated) opcodes. Still seen in the wild (and in iMule codepaths).
pub const KADEMLIA_HELLO_REQ_DEPRECATED: u8 = 0x03;
pub const KADEMLIA_HELLO_RES_DEPRECATED: u8 = 0x04;
pub const KADEMLIA_REQ_DEPRECATED: u8 = 0x05;
pub const KADEMLIA_RES_DEPRECATED: u8 = 0x06;
pub const KADEMLIA_SEARCH_REQ_DEPRECATED: u8 = 0x07;
pub const KADEMLIA_SEARCH_RES_DEPRECATED: u8 = 0x08;
pub const KADEMLIA_SEARCH_NOTES_REQ_DEPRECATED: u8 = 0x09;
pub const KADEMLIA_PUBLISH_REQ_DEPRECATED: u8 = 0x0A;
pub const KADEMLIA_PUBLISH_RES_DEPRECATED: u8 = 0x0B;
pub const KADEMLIA_PUBLISH_NOTES_REQ_DEPRECATED: u8 = 0x0C;

pub const I2P_DEST_LEN: usize = 387;

// FileTags.h (iMule/aMule). Used in Kad2 HELLO taglists.
pub const TAG_FILENAME: u8 = 35; // 0x23 <string>
pub const TAG_KADMISCOPTIONS: u8 = 88; // 0x58
pub const TAG_FILESIZE: u8 = 36; // 0x24
pub const TAG_FILETYPE: u8 = 37; // 0x25 <string>
pub const TAG_SOURCES: u8 = 53; // 0x35 <uint32>
pub const TAG_COMPLETE_SOURCES: u8 = 66; // 0x42 <uint32>
pub const TAG_SERVERDEST: u8 = 81; // 0x51
pub const TAG_SOURCEUDEST: u8 = 82; // 0x52
pub const TAG_SOURCEDEST: u8 = 83; // 0x53
pub const TAG_SOURCETYPE: u8 = 84; // 0x54
pub const TAG_PUBLISHINFO: u8 = 85; // 0x55 <uint32> (search results only)

// rust-mule private extension tag (Kad2 HELLO TagList).
//
// iMule ignores unknown tags in the HELLO taglist, so we can use this for vendor detection.
pub const TAG_RUST_MULE_AGENT: u8 = 0xFE; // <string>

const TAGTYPE_UINT8: u8 = 0x09;
const TAGTYPE_UINT16: u8 = 0x08;
const TAGTYPE_UINT32: u8 = 0x03;
const TAGTYPE_UINT64: u8 = 0x29;
const TAGTYPE_STRING: u8 = 0x02;
const TAGTYPE_FLOAT32: u8 = 0x04;
const TAGTYPE_BOOL: u8 = 0x05;
const TAGTYPE_BOOLARRAY: u8 = 0x06;
const TAGTYPE_BLOB: u8 = 0x07;
const TAGTYPE_BSOB: u8 = 0x0A;
const TAGTYPE_ADDRESS: u8 = 0x27;
const TAGTYPE_STR1: u8 = 0x11;
const TAGTYPE_STR16: u8 = 0x20;

// Defensive bound: avoid allocating absurdly large strings from untrusted network packets.
const MAX_TAG_STRING_LEN: usize = 4096;

#[derive(Debug, Clone)]
pub struct KadPacket {
    pub protocol: u8,
    pub opcode: u8,
    pub payload: Vec<u8>,
}

impl KadPacket {
    pub fn encode(opcode: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(2 + payload.len());
        out.push(OP_KADEMLIAHEADER);
        out.push(opcode);
        out.extend_from_slice(payload);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 2 {
            wire_bail!("kademlia packet too short: {} bytes", bytes.len());
        }
        let protocol = bytes[0];
        let opcode = bytes[1];

        match protocol {
            OP_KADEMLIAHEADER => Ok(Self {
                protocol,
                opcode,
                payload: bytes[2..].to_vec(),
            }),
            OP_KADEMLIAPACKEDPROT => {
                let decompressed = packed::inflate_zlib(&bytes[2..], 512 * 1024)?;
                Ok(Self {
                    protocol: OP_KADEMLIAHEADER,
                    opcode,
                    payload: decompressed,
                })
            }
            other => wire_bail!("unknown kademlia protocol byte: 0x{other:02x}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Kad2BootstrapRes {
    pub sender_id: KadId,
    pub sender_kad_version: u8,
    pub sender_tcp_dest: [u8; I2P_DEST_LEN],
    pub contacts: Vec<Kad2Contact>,
}

#[derive(Debug, Clone)]
pub struct Kad2Contact {
    pub kad_version: u8,
    pub node_id: KadId,
    pub udp_dest: [u8; I2P_DEST_LEN],
}

#[derive(Debug, Clone, Copy)]
pub struct Kad2SearchSourceReq {
    pub target: KadId,
    pub start_position: u16,
    pub file_size: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct Kad2SearchKeyReq {
    pub target: KadId,
    pub start_position: u16,
    pub restrictive: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Kad2PublishSourceReq {
    pub file: KadId,
    pub source: KadId,
}

#[derive(Debug, Clone)]
pub struct Kad2PublishKeyReq {
    pub keyword: KadId,
    pub entries: Vec<Kad2PublishKeyEntry>,
}

#[derive(Debug, Clone)]
pub struct Kad2PublishKeyEntry {
    pub file: KadId,
    pub filename: Option<String>,
    pub file_size: Option<u64>,
    pub file_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Kad2SearchResSources {
    pub sender_id: KadId,
    pub key: KadId,
    pub results: Vec<Kad2SearchResSourceResult>,
}

#[derive(Debug, Clone)]
pub struct Kad2SearchResSourceResult {
    pub source_id: KadId,
    /// Best-effort: KAD source results may carry either SOURCEDEST or SOURCEUDEST (or both).
    pub udp_dest: Option<[u8; I2P_DEST_LEN]>,
    pub source_type: Option<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct Kad2PublishRes {
    pub file: KadId,
    pub source_count: u32,
    pub complete_count: u32,
    pub load: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct Kad2PublishResKey {
    pub key: KadId,
    pub load: u8,
}

#[derive(Debug, Clone)]
pub struct Kad2Hello {
    pub kad_version: u8,
    pub node_id: KadId,
    pub udp_dest: [u8; I2P_DEST_LEN],
    /// Parsed taglist, limited to integer tags we care about.
    pub tags: BTreeMap<u8, u64>,
    /// Optional peer agent string (rust-mule private extension).
    pub agent: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct Kad2Req {
    /// Requested number of contacts to return (low 5 bits, 1..=31).
    ///
    /// iMule names this `type` but uses it as a count (see
    /// `KademliaUDPListener.cpp::ProcessKademlia2Request`).
    pub requested_contacts: u8,
    pub target: KadId,
    pub check: KadId,
    /// Optional sender KadID.
    ///
    /// iMule includes this field so the receiver can request details if needed.
    pub sender_id: Option<KadId>,
}

#[derive(Debug, Clone)]
pub struct Kad2Res {
    pub target: KadId,
    pub contacts: Vec<Kad2Contact>,
}

#[derive(Debug, Clone, Copy)]
pub struct Kad1Req {
    pub kind: u8,
    pub target: KadId,
    pub check: KadId,
    pub sender_id: KadId,
}

const KAD2_CONTACT_MIN_WIRE_BYTES: usize = 1 + 16 + I2P_DEST_LEN;
const KAD2_SEARCH_RESULT_MIN_WIRE_BYTES: usize = 16 + 1; // answer + empty taglist
const KAD2_PUBLISH_KEY_ENTRY_MIN_WIRE_BYTES: usize = 16 + 1; // file + empty taglist

fn clamp_allocation_count_by_remaining(
    declared_count: usize,
    remaining_bytes: usize,
    min_entry_wire_bytes: usize,
) -> usize {
    if min_entry_wire_bytes == 0 {
        return declared_count;
    }
    declared_count.min(remaining_bytes / min_entry_wire_bytes)
}

pub fn decode_kad2_bootstrap_res(payload: &[u8]) -> Result<Kad2BootstrapRes> {
    let mut r = Reader::new(payload);

    let sender_id = r.read_uint128_emule()?;
    let sender_kad_version = r.read_u8()?;
    let sender_tcp_dest = r.read_i2p_dest()?;

    let count = r.read_u16_le()? as usize;
    let alloc_count = clamp_allocation_count_by_remaining(
        count,
        payload.len().saturating_sub(r.i),
        KAD2_CONTACT_MIN_WIRE_BYTES,
    );
    let mut contacts = Vec::with_capacity(alloc_count);
    for _ in 0..count {
        let kad_version = r.read_u8()?;
        let node_id = r.read_uint128_emule()?;
        let udp_dest = r.read_i2p_dest()?;
        contacts.push(Kad2Contact {
            kad_version,
            node_id,
            udp_dest,
        });
    }

    Ok(Kad2BootstrapRes {
        sender_id,
        sender_kad_version,
        sender_tcp_dest,
        contacts,
    })
}

pub fn encode_kad2_bootstrap_res(
    sender_id: KadId,
    sender_kad_version: u8,
    sender_tcp_dest: &[u8; I2P_DEST_LEN],
    contacts: &[Kad2Contact],
) -> Vec<u8> {
    // iMule Kad2 bootstrap response:
    // <senderId u128><senderKadVersion u8><senderTcpDest 387><count u16><contacts...>
    let mut out =
        Vec::with_capacity(16 + 1 + I2P_DEST_LEN + 2 + contacts.len() * (1 + 16 + I2P_DEST_LEN));
    out.extend_from_slice(&sender_id.to_crypt_bytes());
    out.push(sender_kad_version);
    out.extend_from_slice(sender_tcp_dest);
    let count = (contacts.len().min(u16::MAX as usize)) as u16;
    out.extend_from_slice(&count.to_le_bytes());
    for c in contacts.iter().take(count as usize) {
        out.push(c.kad_version);
        out.extend_from_slice(&c.node_id.to_crypt_bytes());
        out.extend_from_slice(&c.udp_dest);
    }
    out
}

pub fn encode_kad2_hello(
    my_kad_version: u8,
    my_id: KadId,
    my_udp_dest: &[u8; I2P_DEST_LEN],
) -> Vec<u8> {
    // iMule Kad2 HELLO: <kadVersion u8><nodeId u128><udpDest 387><TagList>
    //
    // We always include a private vendor tag so other rust-mule peers can be identified.
    let agent = format!("rust-mule/{}", env!("CARGO_PKG_VERSION"));
    let mut out = Vec::with_capacity(1 + 16 + I2P_DEST_LEN + 1 + 2 + 2 + agent.len());
    out.push(my_kad_version);
    out.extend_from_slice(&my_id.to_crypt_bytes());
    out.extend_from_slice(my_udp_dest);
    out.push(1); // TagList count
    write_tag_string(&mut out, TAG_RUST_MULE_AGENT, agent.as_str());
    out
}

pub fn encode_kad2_hello_req(
    my_kad_version: u8,
    my_id: KadId,
    my_udp_dest: &[u8; I2P_DEST_LEN],
) -> Vec<u8> {
    // iMule HELLO_REQ: <kadVersion u8><nodeId u128><udpDest 387><TagList>
    //
    // For HELLO_REQ, iMule sends an empty TagList (count=0) and uses kadVersion=1.
    let mut out = Vec::with_capacity(1 + 16 + I2P_DEST_LEN + 1);
    out.push(my_kad_version);
    out.extend_from_slice(&my_id.to_crypt_bytes());
    out.extend_from_slice(my_udp_dest);
    out.push(0); // TagList count
    out
}

pub fn decode_kad2_hello(payload: &[u8]) -> Result<Kad2Hello> {
    let mut r = Reader::new(payload);
    let kad_version = r.read_u8()?;
    let node_id = r.read_uint128_emule()?;
    let udp_dest = r.read_i2p_dest()?;
    let (tags, agent) = r.read_taglist_hello_info()?;
    Ok(Kad2Hello {
        kad_version,
        node_id,
        udp_dest,
        tags,
        agent,
    })
}

pub fn decode_kad2_req(payload: &[u8]) -> Result<Kad2Req> {
    let mut r = Reader::new(payload);
    let requested_contacts = r.read_u8()? & 0x1F;
    if requested_contacts == 0 {
        wire_bail!("kademlia2 req requested_contacts=0");
    }
    let target = r.read_uint128_emule()?;
    let check = r.read_uint128_emule()?;
    let sender_id = if payload.len().saturating_sub(r.i) >= 16 {
        Some(r.read_uint128_emule()?)
    } else {
        None
    };
    Ok(Kad2Req {
        requested_contacts,
        target,
        check,
        sender_id,
    })
}

pub fn encode_kad2_req(
    requested_contacts: u8,
    target: KadId,
    check: KadId,
    sender_id: KadId,
) -> Vec<u8> {
    // iMule format:
    // <contactCount u8><target u128><check(receiver) u128><sender(my) u128>
    let mut out = Vec::with_capacity(1 + 16 + 16 + 16);
    // iMule masks this field with `0x1F` on decode, so only 1..=31 is representable.
    // Use a safe clamp so config mistakes (e.g. 32) don't silently become 1.
    let c = requested_contacts.clamp(1, 31) & 0x1F;
    out.push(c); // must be non-zero
    out.extend_from_slice(&target.to_crypt_bytes());
    out.extend_from_slice(&check.to_crypt_bytes());
    out.extend_from_slice(&sender_id.to_crypt_bytes());
    out
}

pub fn encode_kad2_res(target: KadId, contacts: &[Kad2Contact]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + 1 + contacts.len() * (1 + 16 + I2P_DEST_LEN));
    out.extend_from_slice(&target.to_crypt_bytes());
    out.push(contacts.len().min(255) as u8);
    for c in contacts.iter().take(255) {
        out.push(c.kad_version);
        out.extend_from_slice(&c.node_id.to_crypt_bytes());
        out.extend_from_slice(&c.udp_dest);
    }
    out
}

pub fn decode_kad2_res(payload: &[u8]) -> Result<Kad2Res> {
    let mut r = Reader::new(payload);
    let target = r.read_uint128_emule()?;
    let count = r.read_u8()? as usize;
    let alloc_count = clamp_allocation_count_by_remaining(
        count,
        payload.len().saturating_sub(r.i),
        KAD2_CONTACT_MIN_WIRE_BYTES,
    );
    let mut contacts = Vec::with_capacity(alloc_count);
    for _ in 0..count {
        let kad_version = r.read_u8()?;
        let node_id = r.read_uint128_emule()?;
        let udp_dest = r.read_i2p_dest()?;
        contacts.push(Kad2Contact {
            kad_version,
            node_id,
            udp_dest,
        });
    }

    Ok(Kad2Res { target, contacts })
}

pub fn decode_kad2_search_source_req(payload: &[u8]) -> Result<Kad2SearchSourceReq> {
    let mut r = Reader::new(payload);
    let target = r.read_uint128_emule()?;
    // startPosition is u16; MSB is used for options (restrictive), so mask it off.
    let start_position = r.read_u16_le()? & 0x7FFF;
    let lo = r.read_u32_le()? as u64;
    let hi = r.read_u32_le()? as u64;
    let file_size = (hi << 32) | lo;
    Ok(Kad2SearchSourceReq {
        target,
        start_position,
        file_size,
    })
}

pub fn encode_kad2_search_source_req(
    target: KadId,
    start_position: u16,
    file_size: u64,
) -> Vec<u8> {
    // iMule (Kad2): <fileID u128><startPos u16><fileSize u64>
    let mut out = Vec::with_capacity(16 + 2 + 8);
    out.extend_from_slice(&target.to_crypt_bytes());
    out.extend_from_slice(&(start_position & 0x7FFF).to_le_bytes());
    out.extend_from_slice(&file_size.to_le_bytes());
    out
}

pub fn encode_kad2_search_key_req(target: KadId, start_position: u16) -> Vec<u8> {
    // iMule (Kad2): <keyword u128><startPos u16>
    //
    // The MSB of startPos (0x8000) indicates a "restrictive" search with an attached expression
    // tree. We don't implement expressions yet, so we always clear that bit.
    let mut out = Vec::with_capacity(16 + 2);
    out.extend_from_slice(&target.to_crypt_bytes());
    out.extend_from_slice(&(start_position & 0x7FFF).to_le_bytes());
    out
}

pub fn decode_kad2_search_key_req(payload: &[u8]) -> Result<Kad2SearchKeyReq> {
    let mut r = Reader::new(payload);
    let target = r.read_uint128_emule()?;
    let start_raw = r.read_u16_le()?;
    let restrictive = (start_raw & 0x8000) != 0;
    let start_position = start_raw & 0x7FFF;
    Ok(Kad2SearchKeyReq {
        target,
        start_position,
        restrictive,
    })
}

pub fn decode_kad2_publish_source_req_min(payload: &[u8]) -> Result<Kad2PublishSourceReq> {
    let mut r = Reader::new(payload);
    let file = r.read_uint128_emule()?;
    let source = r.read_uint128_emule()?;
    Ok(Kad2PublishSourceReq { file, source })
}

pub fn decode_kad2_publish_key_req(payload: &[u8]) -> Result<Kad2PublishKeyReq> {
    // iMule `Process2PublishKeyRequest`:
    // <keyword u128><count u16><entry>*count
    // entry = <file u128><taglist>
    let mut r = Reader::new(payload);
    let keyword = r.read_uint128_emule()?;
    let count = r.read_u16_le()? as usize;
    let alloc_count = clamp_allocation_count_by_remaining(
        count,
        payload.len().saturating_sub(r.i),
        KAD2_PUBLISH_KEY_ENTRY_MIN_WIRE_BYTES,
    );
    let mut entries = Vec::with_capacity(alloc_count);
    for _ in 0..count {
        let file = r.read_uint128_emule()?;
        let tags = r.read_taglist_search_info()?;
        entries.push(Kad2PublishKeyEntry {
            file,
            filename: tags.filename,
            file_size: tags.file_size,
            file_type: tags.file_type,
        });
    }
    Ok(Kad2PublishKeyReq { keyword, entries })
}

/// Lenient decoder for `KADEMLIA2_PUBLISH_KEY_REQ`.
///
/// In the wild we sometimes receive truncated payloads (or payloads we don't fully understand yet).
/// We still want to:
/// - extract the keyword hash (so we can send `PUBLISH_RES` and stop peer retries),
/// - store whatever well-formed entries we can parse (best-effort).
#[derive(Debug, Clone)]
pub struct Kad2PublishKeyReqLenient {
    pub keyword: KadId,
    pub declared_count: u16,
    pub complete: bool,
    pub entries: Vec<Kad2PublishKeyEntry>,
}

/// Extract only the `<keyword u128>` prefix from a Kad2 publish-key request.
pub fn decode_kad2_publish_key_keyword_prefix(payload: &[u8]) -> Result<KadId> {
    // Same weird eMule/aMule encoding used everywhere: 4 x u32 le, chunk order preserved.
    if payload.len() < 16 {
        wire_bail!(
            "publish-key payload too short for keyword prefix: {} bytes",
            payload.len()
        );
    }
    let mut i = 0usize;
    let mut out = [0u8; 16];
    for w in 0..4 {
        let s = payload
            .get(i..i + 4)
            .ok_or(WireError::UnexpectedEof { offset: i })?;
        i += 4;
        let le = u32::from_le_bytes(s.try_into().unwrap());
        out[w * 4..w * 4 + 4].copy_from_slice(&le.to_be_bytes());
    }
    Ok(KadId(out))
}

pub fn decode_kad2_publish_key_req_lenient(payload: &[u8]) -> Result<Kad2PublishKeyReqLenient> {
    struct R<'a> {
        b: &'a [u8],
        i: usize,
    }

    impl<'a> R<'a> {
        fn new(b: &'a [u8]) -> Self {
            Self { b, i: 0 }
        }

        fn read_u8(&mut self) -> Option<u8> {
            let v = *self.b.get(self.i)?;
            self.i += 1;
            Some(v)
        }

        fn read_u16_le(&mut self) -> Option<u16> {
            let s = self.b.get(self.i..self.i + 2)?;
            self.i += 2;
            Some(u16::from_le_bytes(s.try_into().ok()?))
        }

        fn read_u32_le(&mut self) -> Option<u32> {
            let s = self.b.get(self.i..self.i + 4)?;
            self.i += 4;
            Some(u32::from_le_bytes(s.try_into().ok()?))
        }

        fn read_u64_le(&mut self) -> Option<u64> {
            let s = self.b.get(self.i..self.i + 8)?;
            self.i += 8;
            Some(u64::from_le_bytes(s.try_into().ok()?))
        }

        fn read_uint128_emule(&mut self) -> Option<KadId> {
            let mut out = [0u8; 16];
            for w in 0..4 {
                let le = self.read_u32_le()?;
                out[w * 4..w * 4 + 4].copy_from_slice(&le.to_be_bytes());
            }
            Some(KadId(out))
        }

        fn skip(&mut self, len: usize) -> Option<()> {
            self.b.get(self.i..self.i + len)?;
            self.i += len;
            Some(())
        }

        fn read_i2p_dest(&mut self) -> Option<[u8; I2P_DEST_LEN]> {
            let s = self.b.get(self.i..self.i + I2P_DEST_LEN)?;
            self.i += I2P_DEST_LEN;
            s.try_into().ok()
        }

        fn read_taglist_search_info_lenient(&mut self) -> Option<TaglistSearchInfo> {
            // Some iMule debug builds include an extra u32 tag serial after each tag header.
            // Try normal first; if it fails, retry with the extra u32 enabled.
            let start = self.i;
            self.read_taglist_search_info_lenient_impl(false)
                .or_else(|| {
                    self.i = start;
                    self.read_taglist_search_info_lenient_impl(true)
                })
        }

        fn read_taglist_search_info_lenient_impl(
            &mut self,
            debug_serial: bool,
        ) -> Option<TaglistSearchInfo> {
            let count = self.read_u8()? as usize;
            let mut out = TaglistSearchInfo::default();

            for _ in 0..count {
                let type_raw = self.read_u8()?;
                let (tag_type, id) = if (type_raw & 0x80) != 0 {
                    let tag_type = type_raw & 0x7F;
                    let id = self.read_u8()?;
                    (tag_type, Some(id))
                } else {
                    // Old format: <type u8><nameLen u16><name bytes...>
                    let name_len = self.read_u16_le()? as usize;
                    if name_len == 1 {
                        let id = self.read_u8()?;
                        (type_raw, Some(id))
                    } else {
                        self.skip(name_len)?;
                        (type_raw, None)
                    }
                };

                if debug_serial {
                    let _ = self.read_u32_le()?;
                }

                match tag_type {
                    TAGTYPE_UINT8 => {
                        let v = self.read_u8()? as u64;
                        if id == Some(TAG_FILESIZE) {
                            out.file_size = Some(v);
                        }
                        if id == Some(TAG_PUBLISHINFO) {
                            out.publish_info = Some(v as u32);
                        }
                        if id == Some(TAG_SOURCETYPE) {
                            out.source_type = Some(v as u8);
                        }
                    }
                    TAGTYPE_UINT16 => {
                        let v = self.read_u16_le()? as u64;
                        if id == Some(TAG_FILESIZE) {
                            out.file_size = Some(v);
                        }
                    }
                    TAGTYPE_UINT32 => {
                        let v = self.read_u32_le()? as u64;
                        match id {
                            Some(TAG_FILESIZE) => out.file_size = Some(v),
                            Some(TAG_PUBLISHINFO) => out.publish_info = Some(v as u32),
                            _ => {}
                        }
                    }
                    TAGTYPE_UINT64 => {
                        let v = self.read_u64_le()?;
                        match id {
                            Some(TAG_FILESIZE) => out.file_size = Some(v),
                            Some(TAG_PUBLISHINFO) => out.publish_info = Some(v as u32),
                            _ => {}
                        }
                    }
                    TAGTYPE_ADDRESS => {
                        let v = self.read_i2p_dest()?;
                        match id {
                            Some(TAG_SOURCEDEST) => out.source_dest = Some(v),
                            Some(TAG_SOURCEUDEST) => out.source_udest = Some(v),
                            _ => out.fallback_udpdest = Some(v),
                        }
                    }
                    TAGTYPE_STRING => {
                        let len = self.read_u16_le()? as usize;
                        if len > MAX_TAG_STRING_LEN {
                            self.skip(len)?;
                            continue;
                        }
                        let s = self.b.get(self.i..self.i + len)?;
                        self.i += len;
                        let s = String::from_utf8_lossy(s).into_owned();
                        match id {
                            Some(TAG_FILENAME) => out.filename = Some(s),
                            Some(TAG_FILETYPE) => out.file_type = Some(s),
                            _ => {}
                        }
                    }
                    0x01 => {
                        // TAGTYPE_HASH16
                        self.skip(16)?;
                    }
                    TAGTYPE_FLOAT32 => {
                        self.skip(4)?;
                    }
                    TAGTYPE_BOOL => {
                        self.skip(1)?;
                    }
                    TAGTYPE_BOOLARRAY => {
                        let bits = self.read_u16_le()? as usize;
                        self.skip(bits.div_ceil(8))?;
                    }
                    TAGTYPE_BLOB => {
                        let len = self.read_u32_le()? as usize;
                        self.skip(len)?;
                    }
                    TAGTYPE_BSOB => {
                        let len = self.read_u8()? as usize;
                        self.skip(len)?;
                    }
                    t if (TAGTYPE_STR1..=TAGTYPE_STR16).contains(&t) => {
                        let len = (t - TAGTYPE_STR1 + 1) as usize;
                        let s = self.b.get(self.i..self.i + len)?;
                        self.i += len;
                        let s = String::from_utf8_lossy(s).into_owned();
                        match id {
                            Some(TAG_FILENAME) => out.filename = Some(s),
                            Some(TAG_FILETYPE) => out.file_type = Some(s),
                            _ => {}
                        }
                    }
                    _ => {
                        // Unknown tag type. We can't safely resync, so abort parsing this taglist.
                        return None;
                    }
                }
            }

            Some(out)
        }
    }

    let mut r = R::new(payload);
    let keyword = r.read_uint128_emule().ok_or_else(|| {
        WireError::InvalidFormat("publish-key payload too short for keyword".to_string())
    })?;
    let declared_count = r.read_u16_le().ok_or_else(|| {
        WireError::InvalidFormat("publish-key payload too short for count".to_string())
    })?;

    let mut entries = Vec::new();
    let mut complete = true;
    for _ in 0..declared_count {
        let Some(file) = r.read_uint128_emule() else {
            complete = false;
            break;
        };
        let Some(tags) = r.read_taglist_search_info_lenient() else {
            complete = false;
            break;
        };
        entries.push(Kad2PublishKeyEntry {
            file,
            filename: tags.filename,
            file_size: tags.file_size,
            file_type: tags.file_type,
        });
    }

    Ok(Kad2PublishKeyReqLenient {
        keyword,
        declared_count,
        complete,
        entries,
    })
}

pub fn encode_kad2_publish_source_req(
    file: KadId,
    source: KadId,
    source_udp_dest: &[u8; I2P_DEST_LEN],
    file_size: Option<u64>,
) -> Vec<u8> {
    // iMule (Kad2): <fileID u128><sourceID u128><taglist>
    let mut out = Vec::new();
    out.extend_from_slice(&file.to_crypt_bytes());
    out.extend_from_slice(&source.to_crypt_bytes());
    write_publish_source_taglist(&mut out, source_udp_dest, file_size);
    out
}

pub fn encode_kad2_publish_key_req(
    keyword: KadId,
    entries: &[(KadId, &str, u64, Option<&str>)],
) -> Vec<u8> {
    // iMule publish keyword:
    // <keyword u128><count u16><file u128><taglist>...>
    //
    // We implement a minimal tagset required for search results: filename + filesize (+ optional filetype).
    let mut out = Vec::new();
    out.extend_from_slice(&keyword.to_crypt_bytes());
    out.extend_from_slice(&(entries.len().min(u16::MAX as usize) as u16).to_le_bytes());
    for (file, name, size, file_type) in entries.iter().take(u16::MAX as usize) {
        out.extend_from_slice(&file.to_crypt_bytes());
        write_keyword_taglist(&mut out, name, *size, *file_type);
    }
    out
}

pub fn encode_kad2_publish_res_for_source(
    file: KadId,
    source_count: u32,
    complete_count: u32,
    load: u8,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + 4 + 4 + 1);
    out.extend_from_slice(&file.to_crypt_bytes());
    out.extend_from_slice(&source_count.to_le_bytes());
    out.extend_from_slice(&complete_count.to_le_bytes());
    out.push(load);
    out
}

pub fn encode_kad2_publish_res_for_key(key: KadId, load: u8) -> Vec<u8> {
    // iMule `Process2PublishKeyRequest` reply:
    // <key u128><load u8>
    let mut out = Vec::with_capacity(16 + 1);
    out.extend_from_slice(&key.to_crypt_bytes());
    out.push(load);
    out
}

pub fn decode_kad2_publish_res(payload: &[u8]) -> Result<Kad2PublishRes> {
    let mut r = Reader::new(payload);
    let file = r.read_uint128_emule()?;
    let source_count = r.read_u32_le()?;
    let complete_count = r.read_u32_le()?;
    let load = r.read_u8()?;
    Ok(Kad2PublishRes {
        file,
        source_count,
        complete_count,
        load,
    })
}

pub fn decode_kad2_publish_res_key(payload: &[u8]) -> Result<Kad2PublishResKey> {
    let mut r = Reader::new(payload);
    let key = r.read_uint128_emule()?;
    let load = r.read_u8()?;
    Ok(Kad2PublishResKey { key, load })
}

pub fn encode_kad2_search_res_sources(
    my_id: KadId,
    key: KadId,
    results: &[(KadId, [u8; I2P_DEST_LEN])],
) -> Vec<u8> {
    // iMule `CIndexed::SendResults` (kad2):
    // <sender KadID u128><key u128><count u16><result>*count
    // where result = <answer u128><taglist>
    let mut out = Vec::new();
    out.extend_from_slice(&my_id.to_crypt_bytes());
    out.extend_from_slice(&key.to_crypt_bytes());
    out.extend_from_slice(&(results.len().min(u16::MAX as usize) as u16).to_le_bytes());
    for (source_id, udp_dest) in results.iter().take(u16::MAX as usize) {
        out.extend_from_slice(&source_id.to_crypt_bytes());
        write_source_taglist(&mut out, udp_dest);
    }
    out
}

pub fn encode_kad2_search_res_keyword(
    my_id: KadId,
    keyword: KadId,
    results: &[(KadId, String, u64, Option<String>)],
) -> Vec<u8> {
    // Same Kad2 results container as sources:
    // <sender u128><key u128><count u16><answer u128><taglist>
    let mut out = Vec::new();
    out.extend_from_slice(&my_id.to_crypt_bytes());
    out.extend_from_slice(&keyword.to_crypt_bytes());
    out.extend_from_slice(&(results.len().min(u16::MAX as usize) as u16).to_le_bytes());
    for (file_id, filename, file_size, file_type) in results.iter().take(u16::MAX as usize) {
        out.extend_from_slice(&file_id.to_crypt_bytes());
        write_keyword_taglist(
            &mut out,
            filename.as_str(),
            *file_size,
            file_type.as_deref(),
        );
    }
    out
}

fn write_source_taglist(out: &mut Vec<u8>, udp_dest: &[u8; I2P_DEST_LEN]) {
    // Minimum tagset required by iMule/aMule to treat a result as usable.
    // See iMule `Search.cpp::ProcessResultFile`.
    out.push(3); // tag count

    write_tag_uint8(out, TAG_SOURCETYPE, 1);
    write_tag_address(out, TAG_SOURCEDEST, udp_dest);
    write_tag_address(out, TAG_SOURCEUDEST, udp_dest);
}

fn write_publish_source_taglist(
    out: &mut Vec<u8>,
    udp_dest: &[u8; I2P_DEST_LEN],
    file_size: Option<u64>,
) {
    // iMule will accept a publish source request as long as it contains at least TAG_SOURCETYPE.
    // Including the address tags makes interop more robust (and matches our search results).
    //
    // Layout: <tagCount u8><tag>...
    let mut count = 3u8;
    if file_size.is_some() {
        count += 1;
    }
    out.push(count);

    write_tag_uint8(out, TAG_SOURCETYPE, 1);
    write_tag_address(out, TAG_SOURCEDEST, udp_dest);
    write_tag_address(out, TAG_SOURCEUDEST, udp_dest);

    if let Some(sz) = file_size {
        write_tag_uint64(out, TAG_FILESIZE, sz);
    }
}

fn write_keyword_taglist(
    out: &mut Vec<u8>,
    filename: &str,
    file_size: u64,
    file_type: Option<&str>,
) {
    // iMule Kad2 keyword publishing requires at least one additional tag besides filename+size;
    // otherwise `CIndexed::AddKeyword` rejects the entry (`GetTagCount() == 0`) because those
    // two fields are stored out-of-band and do not contribute to the internal tag list.
    let mut count = 4u8;
    if file_type.is_some() {
        count += 1;
    }
    out.push(count);

    write_tag_string(out, TAG_FILENAME, filename);
    write_tag_uint64(out, TAG_FILESIZE, file_size);
    write_tag_uint32(out, TAG_COMPLETE_SOURCES, 1);
    write_tag_uint32(out, TAG_SOURCES, 1);
    if let Some(t) = file_type {
        write_tag_string(out, TAG_FILETYPE, t);
    }
}

fn write_tag_uint8(out: &mut Vec<u8>, id: u8, val: u8) {
    out.push(TAGTYPE_UINT8 | 0x80);
    out.push(id);
    out.push(val);
}

fn write_tag_uint32(out: &mut Vec<u8>, id: u8, val: u32) {
    out.push(TAGTYPE_UINT32 | 0x80);
    out.push(id);
    out.extend_from_slice(&val.to_le_bytes());
}

fn write_tag_uint64(out: &mut Vec<u8>, id: u8, val: u64) {
    out.push(TAGTYPE_UINT64 | 0x80);
    out.push(id);
    out.extend_from_slice(&val.to_le_bytes());
}

fn write_tag_string(out: &mut Vec<u8>, id: u8, s: &str) {
    // NOTE: Tag strings in iMule are UTF-8 for Kad packets.
    out.push(TAGTYPE_STRING | 0x80);
    out.push(id);
    let b = s.as_bytes();
    let len = b.len().min(u16::MAX as usize) as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&b[..len as usize]);
}

fn write_tag_address(out: &mut Vec<u8>, id: u8, addr: &[u8; I2P_DEST_LEN]) {
    out.push(TAGTYPE_ADDRESS | 0x80);
    out.push(id);
    out.extend_from_slice(addr);
}

pub fn decode_kad2_search_res_sources(payload: &[u8]) -> Result<Kad2SearchResSources> {
    let res = decode_kad2_search_res(payload)?;
    let mut results = Vec::with_capacity(res.results.len());
    for r in res.results {
        results.push(Kad2SearchResSourceResult {
            source_id: r.answer,
            udp_dest: r.tags.best_udp_dest(),
            source_type: r.tags.source_type,
        });
    }
    Ok(Kad2SearchResSources {
        sender_id: res.sender_id,
        key: res.key,
        results,
    })
}

#[derive(Debug, Clone)]
pub struct Kad2SearchRes {
    pub sender_id: KadId,
    /// The key which was searched: for sources this is the file ID, for keyword search it is the
    /// keyword hash (MD4).
    pub key: KadId,
    pub results: Vec<Kad2SearchResEntry>,
}

#[derive(Debug, Clone)]
pub struct Kad2SearchResEntry {
    pub answer: KadId,
    pub tags: TaglistSearchInfo,
}

#[derive(Debug, Default, Clone)]
pub struct TaglistSearchInfo {
    pub source_type: Option<u8>,
    pub source_dest: Option<[u8; I2P_DEST_LEN]>,
    pub source_udest: Option<[u8; I2P_DEST_LEN]>,
    pub file_size: Option<u64>,
    pub filename: Option<String>,
    pub file_type: Option<String>,
    pub publish_info: Option<u32>,
    fallback_udpdest: Option<[u8; I2P_DEST_LEN]>,
}

impl TaglistSearchInfo {
    pub fn best_udp_dest(&self) -> Option<[u8; I2P_DEST_LEN]> {
        self.source_udest
            .or(self.source_dest)
            .or(self.fallback_udpdest)
    }
}

pub fn decode_kad2_search_res(payload: &[u8]) -> Result<Kad2SearchRes> {
    // iMule `CIndexed::SendResults` (Kad2):
    // <sender u128><key u128><count u16><result>*count
    // result = <answer u128><taglist>
    let mut r = Reader::new(payload);
    let sender_id = r.read_uint128_emule()?;
    let key = r.read_uint128_emule()?;
    let count = r.read_u16_le()? as usize;
    let alloc_count = clamp_allocation_count_by_remaining(
        count,
        payload.len().saturating_sub(r.i),
        KAD2_SEARCH_RESULT_MIN_WIRE_BYTES,
    );
    let mut results = Vec::with_capacity(alloc_count);
    for _ in 0..count {
        let answer = r.read_uint128_emule()?;
        let tags = r.read_taglist_search_info()?;
        results.push(Kad2SearchResEntry { answer, tags });
    }

    Ok(Kad2SearchRes {
        sender_id,
        key,
        results,
    })
}

pub fn decode_kad1_req(payload: &[u8]) -> Result<Kad1Req> {
    let mut r = Reader::new(payload);
    let kind = r.read_u8()? & 0x1F;
    if kind == 0 {
        wire_bail!("kademlia req kind=0");
    }
    let target = r.read_uint128_emule()?;
    let check = r.read_uint128_emule()?;
    let sender_id = r.read_uint128_emule()?;
    Ok(Kad1Req {
        kind,
        target,
        check,
        sender_id,
    })
}

pub fn encode_kad1_res(target: KadId, contacts: &[(KadId, [u8; I2P_DEST_LEN])]) -> Vec<u8> {
    // Layout (iMule `WriteToKad1Contact`):
    // <target u128><count u8><contact>*count
    // where contact = <client_id u128><udp_dest 387><tcp_dest 387><type u8>
    let mut out = Vec::with_capacity(16 + 1 + contacts.len() * (16 + 2 * I2P_DEST_LEN + 1));
    out.extend_from_slice(&target.to_crypt_bytes());
    out.push(contacts.len().min(255) as u8);
    for (id, udp_dest) in contacts.iter().take(255) {
        out.extend_from_slice(&id.to_crypt_bytes());
        out.extend_from_slice(udp_dest);
        out.extend_from_slice(udp_dest); // TCP dest == UDP dest for I2P in iMule
        out.push(3); // default contact type
    }
    out
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn new(b: &'a [u8]) -> Self {
        Self { b, i: 0 }
    }

    fn read_u8(&mut self) -> Result<u8> {
        let v = *self
            .b
            .get(self.i)
            .ok_or(WireError::UnexpectedEof { offset: self.i })?;
        self.i += 1;
        Ok(v)
    }

    fn read_u16_le(&mut self) -> Result<u16> {
        let s = self
            .b
            .get(self.i..self.i + 2)
            .ok_or(WireError::UnexpectedEof { offset: self.i })?;
        self.i += 2;
        let mut out = [0u8; 2];
        out.copy_from_slice(s);
        Ok(u16::from_le_bytes(out))
    }

    fn read_u32_le(&mut self) -> Result<u32> {
        let s = self
            .b
            .get(self.i..self.i + 4)
            .ok_or(WireError::UnexpectedEof { offset: self.i })?;
        self.i += 4;
        let mut out = [0u8; 4];
        out.copy_from_slice(s);
        Ok(u32::from_le_bytes(out))
    }

    fn read_i2p_dest(&mut self) -> Result<[u8; I2P_DEST_LEN]> {
        let s = self
            .b
            .get(self.i..self.i + I2P_DEST_LEN)
            .ok_or(WireError::UnexpectedEof { offset: self.i })?;
        self.i += I2P_DEST_LEN;
        let mut out = [0u8; I2P_DEST_LEN];
        out.copy_from_slice(s);
        Ok(out)
    }

    // iMule "weird" UInt128 encoding:
    // Four little-endian 32-bit numbers, stored in big-endian order.
    // Our `KadId` stores big-endian bytes.
    fn read_uint128_emule(&mut self) -> Result<KadId> {
        let mut out = [0u8; 16];
        for i in 0..4 {
            let le = self.read_u32_le()?;
            out[i * 4..i * 4 + 4].copy_from_slice(&le.to_be_bytes());
        }
        Ok(KadId(out))
    }

    fn read_taglist_hello_info(&mut self) -> Result<(BTreeMap<u8, u64>, Option<String>)> {
        // Like `TagList` in iMule (`Tag.cpp`). Some builds include an extra `u32` tag serial
        // (`_DEBUG_TAGS`), so we attempt both layouts.
        let start = self.i;
        match self.read_taglist_hello_info_impl(false) {
            Ok(v) => Ok(v),
            Err(e1) => {
                self.i = start;
                match self.read_taglist_hello_info_impl(true) {
                    Ok(v) => Ok(v),
                    Err(_) => Err(e1),
                }
            }
        }
    }

    fn read_taglist_hello_info_impl(
        &mut self,
        debug_serial: bool,
    ) -> Result<(BTreeMap<u8, u64>, Option<String>)> {
        let n = self.read_u8()? as usize;
        let mut ints = BTreeMap::<u8, u64>::new();
        let mut agent: Option<String> = None;

        for _ in 0..n {
            let type_raw = self.read_u8()?;
            let (tag_type, id) = if (type_raw & 0x80) != 0 {
                let tag_type = type_raw & 0x7F;
                let id = self.read_u8()?;
                (tag_type, Some(id))
            } else {
                let name_len = self.read_u16_le()? as usize;
                if name_len == 1 {
                    let id = self.read_u8()?;
                    (type_raw, Some(id))
                } else {
                    self.skip(name_len)?;
                    (type_raw, None)
                }
            };

            if debug_serial {
                let _ = self.read_u32_le()?;
            }

            match tag_type {
                TAGTYPE_UINT8 => {
                    let v = self.read_u8()? as u64;
                    if let Some(id) = id {
                        ints.insert(id, v);
                    }
                }
                TAGTYPE_UINT16 => {
                    let v = self.read_u16_le()? as u64;
                    if let Some(id) = id {
                        ints.insert(id, v);
                    }
                }
                TAGTYPE_UINT32 => {
                    let v = self.read_u32_le()? as u64;
                    if let Some(id) = id {
                        ints.insert(id, v);
                    }
                }
                TAGTYPE_UINT64 => {
                    let lo = self.read_u32_le()? as u64;
                    let hi = self.read_u32_le()? as u64;
                    let v = (hi << 32) | lo;
                    if let Some(id) = id {
                        ints.insert(id, v);
                    }
                }
                TAGTYPE_STRING => {
                    let len = self.read_u16_le()? as usize;
                    if len > MAX_TAG_STRING_LEN {
                        self.skip(len)?;
                        continue;
                    }
                    let s = self
                        .b
                        .get(self.i..self.i + len)
                        .ok_or(WireError::UnexpectedEof { offset: self.i })?;
                    self.i += len;
                    let s = String::from_utf8_lossy(s).into_owned();
                    if id == Some(TAG_RUST_MULE_AGENT) {
                        agent = Some(s);
                    }
                }
                0x01 => {
                    // TAGTYPE_HASH16
                    self.skip(16)?;
                }
                TAGTYPE_ADDRESS => {
                    self.skip(I2P_DEST_LEN)?;
                }
                TAGTYPE_FLOAT32 => {
                    self.skip(4)?;
                }
                TAGTYPE_BOOL => {
                    self.skip(1)?;
                }
                TAGTYPE_BOOLARRAY => {
                    let bits = self.read_u16_le()? as usize;
                    self.skip(bits.div_ceil(8))?;
                }
                TAGTYPE_BLOB => {
                    let len = self.read_u32_le()? as usize;
                    self.skip(len)?;
                }
                TAGTYPE_BSOB => {
                    let len = self.read_u8()? as usize;
                    self.skip(len)?;
                }
                t if (TAGTYPE_STR1..=TAGTYPE_STR16).contains(&t) => {
                    let len = (t - TAGTYPE_STR1 + 1) as usize;
                    let s = self
                        .b
                        .get(self.i..self.i + len)
                        .ok_or(WireError::UnexpectedEof { offset: self.i })?;
                    self.i += len;
                    let s = String::from_utf8_lossy(s).into_owned();
                    if id == Some(TAG_RUST_MULE_AGENT) {
                        agent = Some(s);
                    }
                }
                other => wire_bail!("unknown tag type 0x{other:02x}"),
            }
        }

        Ok((ints, agent))
    }

    fn skip(&mut self, len: usize) -> Result<()> {
        self.b
            .get(self.i..self.i + len)
            .ok_or(WireError::UnexpectedEof { offset: self.i })?;
        self.i += len;
        Ok(())
    }

    fn read_taglist_search_info(&mut self) -> Result<TaglistSearchInfo> {
        // Some iMule debug builds include an extra u32 tag serial after each tag header
        // (see `Tag.cpp` under `_DEBUG_TAGS`).
        //
        // We first try the normal parse; if it fails, retry with that 4-byte field enabled.
        let start = self.i;
        match self.read_taglist_search_info_impl(false) {
            Ok(v) => Ok(v),
            Err(e1) => {
                self.i = start;
                match self.read_taglist_search_info_impl(true) {
                    Ok(v) => Ok(v),
                    Err(_) => Err(e1),
                }
            }
        }
    }

    fn read_taglist_search_info_impl(&mut self, debug_serial: bool) -> Result<TaglistSearchInfo> {
        let count = self.read_u8()? as usize;
        let mut out = TaglistSearchInfo::default();

        for _ in 0..count {
            let type_raw = self.read_u8()?;
            let (tag_type, id) = if (type_raw & 0x80) != 0 {
                let tag_type = type_raw & 0x7F;
                let id = self.read_u8()?;
                (tag_type, Some(id))
            } else {
                // Old format: <type u8><nameLen u16><name bytes...>
                let name_len = self.read_u16_le()? as usize;
                if name_len == 1 {
                    let id = self.read_u8()?;
                    (type_raw, Some(id))
                } else {
                    self.skip(name_len)?;
                    (type_raw, None)
                }
            };

            if debug_serial {
                // `_DEBUG_TAGS`: Tag.cpp writes an extra u32 after the tag header.
                let _ = self.read_u32_le()?;
            }

            match tag_type {
                TAGTYPE_UINT8 => {
                    let v = self.read_u8()?;
                    if id == Some(TAG_SOURCETYPE) {
                        out.source_type = Some(v);
                    }
                }
                TAGTYPE_UINT16 => {
                    let v = self.read_u16_le()? as u64;
                    if id == Some(TAG_FILESIZE) {
                        out.file_size = Some(v);
                    }
                }
                TAGTYPE_UINT32 => {
                    let v = self.read_u32_le()? as u64;
                    match id {
                        Some(TAG_FILESIZE) => out.file_size = Some(v),
                        Some(TAG_PUBLISHINFO) => out.publish_info = Some(v as u32),
                        _ => {}
                    }
                }
                TAGTYPE_UINT64 => {
                    let lo = self.read_u32_le()? as u64;
                    let hi = self.read_u32_le()? as u64;
                    let v = (hi << 32) | lo;
                    match id {
                        Some(TAG_FILESIZE) => out.file_size = Some(v),
                        Some(TAG_PUBLISHINFO) => out.publish_info = Some(v as u32),
                        _ => {}
                    }
                }
                TAGTYPE_ADDRESS => {
                    let v = self.read_i2p_dest()?;
                    match id {
                        Some(TAG_SOURCEDEST) => out.source_dest = Some(v),
                        Some(TAG_SOURCEUDEST) => out.source_udest = Some(v),
                        _ => out.fallback_udpdest = Some(v),
                    }
                }
                TAGTYPE_STRING => {
                    let len = self.read_u16_le()? as usize;
                    if len > MAX_TAG_STRING_LEN {
                        self.skip(len)?;
                        continue;
                    }
                    let s = self
                        .b
                        .get(self.i..self.i + len)
                        .ok_or(WireError::UnexpectedEof { offset: self.i })?;
                    self.i += len;
                    let s = String::from_utf8_lossy(s).into_owned();
                    match id {
                        Some(TAG_FILENAME) => out.filename = Some(s),
                        Some(TAG_FILETYPE) => out.file_type = Some(s),
                        _ => {}
                    }
                }
                0x01 => {
                    // TAGTYPE_HASH16
                    self.skip(16)?;
                }
                TAGTYPE_FLOAT32 => {
                    self.skip(4)?;
                }
                TAGTYPE_BOOL => {
                    self.skip(1)?;
                }
                TAGTYPE_BOOLARRAY => {
                    // TAGTYPE_BOOLARRAY: <u16 bitCount><bytes...>
                    let bits = self.read_u16_le()? as usize;
                    self.skip(bits.div_ceil(8))?;
                }
                TAGTYPE_BLOB => {
                    let len = self.read_u32_le()? as usize;
                    self.skip(len)?;
                }
                TAGTYPE_BSOB => {
                    let len = self.read_u8()? as usize;
                    self.skip(len)?;
                }
                t if (TAGTYPE_STR1..=TAGTYPE_STR16).contains(&t) => {
                    let len = (t - TAGTYPE_STR1 + 1) as usize;
                    let s = self
                        .b
                        .get(self.i..self.i + len)
                        .ok_or(WireError::UnexpectedEof { offset: self.i })?;
                    self.i += len;
                    let s = String::from_utf8_lossy(s).into_owned();
                    match id {
                        Some(TAG_FILENAME) => out.filename = Some(s),
                        Some(TAG_FILETYPE) => out.file_type = Some(s),
                        _ => {}
                    }
                }
                other => wire_bail!("unknown tag type 0x{other:02x}"),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_unpacked_packet() {
        let raw = [OP_KADEMLIAHEADER, KADEMLIA2_PING, 1, 2, 3];
        let p = KadPacket::decode(&raw).unwrap();
        assert_eq!(p.protocol, OP_KADEMLIAHEADER);
        assert_eq!(p.opcode, KADEMLIA2_PING);
        assert_eq!(p.payload, vec![1, 2, 3]);
    }

    #[test]
    fn kad2_publish_key_req_contains_sources_tags() {
        // iMule rejects keyword publishes unless at least one non-filename/size tag exists
        // (`CIndexed::AddKeyword` checks `GetTagCount() != 0`).
        let keyword = KadId([0xAA; 16]);
        let file = KadId([0x11; 16]);
        let payload = encode_kad2_publish_key_req(keyword, &[(file, "x.bin", 123, None)]);

        // Layout:
        // <keyword u128><count u16><file u128><taglist...>
        let taglist_off = 16 + 2 + 16;
        assert!(payload.len() > taglist_off);

        // Tag count should be 4: filename + filesize + complete sources + sources.
        assert_eq!(payload[taglist_off], 4);

        // TAG_COMPLETE_SOURCES (66 / 0x42) + TAG_SOURCES (53 / 0x35), both uint32 (type 0x03).
        let want_complete = [0x83u8, TAG_COMPLETE_SOURCES, 1, 0, 0, 0];
        let want_sources = [0x83u8, TAG_SOURCES, 1, 0, 0, 0];

        assert!(
            payload
                .windows(want_complete.len())
                .any(|w| w == want_complete),
            "missing TAG_COMPLETE_SOURCES in publish taglist"
        );
        assert!(
            payload
                .windows(want_sources.len())
                .any(|w| w == want_sources),
            "missing TAG_SOURCES in publish taglist"
        );
    }

    #[test]
    fn kad2_publish_key_req_lenient_extracts_keyword_on_truncation() {
        let keyword = KadId([0xAB; 16]);
        let file = KadId([0x11; 16]);
        let full = encode_kad2_publish_key_req(keyword, &[(file, "x.bin", 123, None)]);

        // Truncate inside the first taglist so strict parsing would fail.
        let truncated = &full[..(16 + 2 + 16 + 3)];
        let parsed = decode_kad2_publish_key_req_lenient(truncated).unwrap();
        assert_eq!(parsed.keyword, keyword);
        assert!(!parsed.complete);
        // We might or might not have a parsed entry depending on where the truncation happened,
        // but keyword must be present so we can ACK.
    }

    #[test]
    fn kad2_source_opcode_values_match_imule() {
        // Source reference: iMule `src/include/protocol/kad2/Client2Client/UDP.h`.
        assert_eq!(KADEMLIA2_SEARCH_SOURCE_REQ, 0x15);
        assert_eq!(KADEMLIA2_PUBLISH_SOURCE_REQ, 0x19);
    }

    #[test]
    fn kad2_search_source_req_layout_matches_imule() {
        let target = KadId([0x22; 16]);
        let file_size = 0x0102_0304_0506_0708u64;
        let payload = encode_kad2_search_source_req(target, 0x9123, file_size);

        // iMule layout:
        // <fileID u128><startPos u16 masked to 0x7FFF><fileSize u64 little-endian>
        assert_eq!(payload.len(), 16 + 2 + 8);
        assert_eq!(&payload[0..16], &target.to_crypt_bytes());
        assert_eq!(&payload[16..18], &(0x1123u16).to_le_bytes());
        assert_eq!(&payload[18..26], &file_size.to_le_bytes());

        let parsed = decode_kad2_search_source_req(&payload).unwrap();
        assert_eq!(parsed.target, target);
        assert_eq!(parsed.start_position, 0x1123);
        assert_eq!(parsed.file_size, file_size);
    }

    #[test]
    fn kad2_publish_source_req_layout_has_required_source_tags() {
        let file = KadId([0x33; 16]);
        let source = KadId([0x44; 16]);
        let udp_dest = [0x55; I2P_DEST_LEN];
        let payload = encode_kad2_publish_source_req(file, source, &udp_dest, Some(123));

        // iMule layout:
        // <fileID u128><sourceID u128><taglist>
        assert_eq!(&payload[0..16], &file.to_crypt_bytes());
        assert_eq!(&payload[16..32], &source.to_crypt_bytes());
        assert!(payload.len() > 32);

        let taglist = &payload[32..];
        assert_eq!(
            taglist[0], 4,
            "expected SOURCETYPE+SOURCEDEST+SOURCEUDEST+FILESIZE"
        );

        let sourcetype_tag = [TAGTYPE_UINT8 | 0x80, TAG_SOURCETYPE, 1];
        assert!(
            taglist
                .windows(sourcetype_tag.len())
                .any(|w| w == sourcetype_tag),
            "missing TAG_SOURCETYPE in publish-source taglist"
        );

        let parsed_min = decode_kad2_publish_source_req_min(&payload).unwrap();
        assert_eq!(parsed_min.file, file);
        assert_eq!(parsed_min.source, source);
    }

    #[test]
    fn decode_kad2_bootstrap_res_rejects_truncated_large_count() {
        let sender = KadId([1; 16]);
        let mut payload = Vec::new();
        payload.extend_from_slice(&sender.to_crypt_bytes());
        payload.push(8); // sender kad version
        payload.extend_from_slice(&[0u8; I2P_DEST_LEN]);
        payload.extend_from_slice(&u16::MAX.to_le_bytes()); // hostile count
        // no contact entries
        assert!(decode_kad2_bootstrap_res(&payload).is_err());
    }

    #[test]
    fn decode_kad2_publish_key_req_rejects_truncated_large_count() {
        let keyword = KadId([2; 16]);
        let mut payload = Vec::new();
        payload.extend_from_slice(&keyword.to_crypt_bytes());
        payload.extend_from_slice(&u16::MAX.to_le_bytes()); // hostile count
        // no entries
        assert!(decode_kad2_publish_key_req(&payload).is_err());
    }

    #[test]
    fn decode_kad2_search_res_rejects_truncated_large_count() {
        let sender = KadId([3; 16]);
        let key = KadId([4; 16]);
        let mut payload = Vec::new();
        payload.extend_from_slice(&sender.to_crypt_bytes());
        payload.extend_from_slice(&key.to_crypt_bytes());
        payload.extend_from_slice(&u16::MAX.to_le_bytes()); // hostile count
        // no results
        assert!(decode_kad2_search_res(&payload).is_err());
    }

    #[test]
    fn decode_kad2_res_rejects_truncated_large_count() {
        let target = KadId([5; 16]);
        let mut payload = Vec::new();
        payload.extend_from_slice(&target.to_crypt_bytes());
        payload.push(u8::MAX); // hostile count
        // no contacts
        assert!(decode_kad2_res(&payload).is_err());
    }
}
