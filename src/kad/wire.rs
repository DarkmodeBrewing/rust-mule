use crate::kad::{KadId, packed};
use anyhow::{Result, bail};
use std::collections::BTreeMap;

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

pub const I2P_DEST_LEN: usize = 387;

// FileTags.h (iMule/aMule). Used in Kad2 HELLO taglists.
pub const TAG_KADMISCOPTIONS: u8 = 88; // 0x58
pub const TAG_FILESIZE: u8 = 36; // 0x24
pub const TAG_SERVERDEST: u8 = 81; // 0x51
pub const TAG_SOURCEUDEST: u8 = 82; // 0x52
pub const TAG_SOURCEDEST: u8 = 83; // 0x53
pub const TAG_SOURCETYPE: u8 = 84; // 0x54

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
            bail!("kademlia packet too short: {} bytes", bytes.len());
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
            other => bail!("unknown kademlia protocol byte: 0x{other:02x}"),
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
pub struct Kad2PublishSourceReq {
    pub file: KadId,
    pub source: KadId,
}

#[derive(Debug, Clone)]
pub struct Kad2Hello {
    pub kad_version: u8,
    pub node_id: KadId,
    pub udp_dest: [u8; I2P_DEST_LEN],
    /// Parsed taglist, limited to integer tags we care about.
    pub tags: BTreeMap<u8, u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct Kad2Req {
    pub kind: u8,
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

pub fn decode_kad2_bootstrap_res(payload: &[u8]) -> Result<Kad2BootstrapRes> {
    let mut r = Reader::new(payload);

    let sender_id = r.read_uint128_emule()?;
    let sender_kad_version = r.read_u8()?;
    let sender_tcp_dest = r.read_i2p_dest()?;

    let count = r.read_u16_le()? as usize;
    let mut contacts = Vec::with_capacity(count);
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

pub fn encode_kad2_hello(
    my_kad_version: u8,
    my_id: KadId,
    my_udp_dest: &[u8; I2P_DEST_LEN],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 16 + I2P_DEST_LEN + 1);
    out.push(my_kad_version);
    out.extend_from_slice(&my_id.to_crypt_bytes());
    out.extend_from_slice(my_udp_dest);
    out.push(0); // TagList count = 0
    out
}

pub fn decode_kad2_hello(payload: &[u8]) -> Result<Kad2Hello> {
    let mut r = Reader::new(payload);
    let kad_version = r.read_u8()?;
    let node_id = r.read_uint128_emule()?;
    let udp_dest = r.read_i2p_dest()?;
    let tags = r.read_taglist_ints()?;
    Ok(Kad2Hello {
        kad_version,
        node_id,
        udp_dest,
        tags,
    })
}

pub fn decode_kad2_req(payload: &[u8]) -> Result<Kad2Req> {
    let mut r = Reader::new(payload);
    let kind = r.read_u8()? & 0x1F;
    if kind == 0 {
        bail!("kademlia2 req kind=0");
    }
    let target = r.read_uint128_emule()?;
    let check = r.read_uint128_emule()?;
    let sender_id = if payload.len().saturating_sub(r.i) >= 16 {
        Some(r.read_uint128_emule()?)
    } else {
        None
    };
    Ok(Kad2Req {
        kind,
        target,
        check,
        sender_id,
    })
}

pub fn encode_kad2_req(kind: u8, target: KadId, check: KadId, sender_id: KadId) -> Vec<u8> {
    // iMule format:
    // <kind u8><target u128><check(receiver) u128><sender(my) u128>
    let mut out = Vec::with_capacity(1 + 16 + 16 + 16);
    // iMule masks this field with `0x1F` on decode, so only 1..=31 is representable.
    // Use a safe clamp so config mistakes (e.g. 32) don't silently become 1.
    let k = kind.clamp(1, 31) & 0x1F;
    out.push(k); // kind must be non-zero
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

    let mut contacts = Vec::with_capacity(count);
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

pub fn decode_kad2_publish_source_req_min(payload: &[u8]) -> Result<Kad2PublishSourceReq> {
    let mut r = Reader::new(payload);
    let file = r.read_uint128_emule()?;
    let source = r.read_uint128_emule()?;
    Ok(Kad2PublishSourceReq { file, source })
}

pub fn encode_kad2_publish_res_for_source(file: KadId, source_count: u32, complete_count: u32, load: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + 4 + 4 + 1);
    out.extend_from_slice(&file.to_crypt_bytes());
    out.extend_from_slice(&source_count.to_le_bytes());
    out.extend_from_slice(&complete_count.to_le_bytes());
    out.push(load);
    out
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

fn write_source_taglist(out: &mut Vec<u8>, udp_dest: &[u8; I2P_DEST_LEN]) {
    // Minimum tagset required by iMule/aMule to treat a result as usable.
    // See iMule `Search.cpp::ProcessResultFile`.
    out.push(3); // tag count

    write_tag_uint8(out, TAG_SOURCETYPE, 1);
    write_tag_address(out, TAG_SOURCEDEST, udp_dest);
    write_tag_address(out, TAG_SOURCEUDEST, udp_dest);
}

fn write_tag_uint8(out: &mut Vec<u8>, id: u8, val: u8) {
    out.push(TAGTYPE_UINT8 | 0x80);
    out.push(id);
    out.push(val);
}

fn write_tag_address(out: &mut Vec<u8>, id: u8, addr: &[u8; I2P_DEST_LEN]) {
    out.push(TAGTYPE_ADDRESS | 0x80);
    out.push(id);
    out.extend_from_slice(addr);
}

pub fn decode_kad1_req(payload: &[u8]) -> Result<Kad1Req> {
    let mut r = Reader::new(payload);
    let kind = r.read_u8()? & 0x1F;
    if kind == 0 {
        bail!("kademlia req kind=0");
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
            .ok_or_else(|| anyhow::anyhow!("unexpected EOF at {}", self.i))?;
        self.i += 1;
        Ok(v)
    }

    fn read_u16_le(&mut self) -> Result<u16> {
        let s = self
            .b
            .get(self.i..self.i + 2)
            .ok_or_else(|| anyhow::anyhow!("unexpected EOF at {}", self.i))?;
        self.i += 2;
        Ok(u16::from_le_bytes(s.try_into().unwrap()))
    }

    fn read_u32_le(&mut self) -> Result<u32> {
        let s = self
            .b
            .get(self.i..self.i + 4)
            .ok_or_else(|| anyhow::anyhow!("unexpected EOF at {}", self.i))?;
        self.i += 4;
        Ok(u32::from_le_bytes(s.try_into().unwrap()))
    }

    fn read_i2p_dest(&mut self) -> Result<[u8; I2P_DEST_LEN]> {
        let s = self
            .b
            .get(self.i..self.i + I2P_DEST_LEN)
            .ok_or_else(|| anyhow::anyhow!("unexpected EOF at {}", self.i))?;
        self.i += I2P_DEST_LEN;
        Ok(s.try_into().unwrap())
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

    fn read_taglist_ints(&mut self) -> Result<BTreeMap<u8, u64>> {
        // iMule TagList: <u8 count><tag>...
        let n = self.read_u8()? as usize;
        let mut out = BTreeMap::<u8, u64>::new();
        for _ in 0..n {
            let ty0 = self.read_u8()?;
            let (ty, id) = if (ty0 & 0x80) != 0 {
                (ty0 & 0x7F, self.read_u8()?)
            } else {
                bail!("unsupported string-named tag in taglist (type=0x{ty0:02x})");
            };

            match ty {
                // TagTypes.h (iMule/aMule)
                TAGTYPE_UINT8 => {
                    // TAGTYPE_UINT8
                    out.insert(id, self.read_u8()? as u64);
                }
                TAGTYPE_UINT16 => {
                    // TAGTYPE_UINT16
                    out.insert(id, self.read_u16_le()? as u64);
                }
                TAGTYPE_UINT32 => {
                    // TAGTYPE_UINT32
                    out.insert(id, self.read_u32_le()? as u64);
                }
                TAGTYPE_UINT64 => {
                    // TAGTYPE_UINT64
                    let lo = self.read_u32_le()? as u64;
                    let hi = self.read_u32_le()? as u64;
                    out.insert(id, (hi << 32) | lo);
                }
                TAGTYPE_STRING => {
                    // TAGTYPE_STRING: <u16 len><bytes...>
                    let len = self.read_u16_le()? as usize;
                    self.skip(len)?;
                }
                TAGTYPE_FLOAT32 => {
                    // TAGTYPE_FLOAT32
                    self.skip(4)?;
                }
                TAGTYPE_BOOL => {
                    // TAGTYPE_BOOL
                    self.skip(1)?;
                }
                TAGTYPE_BOOLARRAY => {
                    // TAGTYPE_BOOLARRAY: <u16 len><bytes...> (best-effort skip)
                    let len = self.read_u16_le()? as usize;
                    self.skip(len)?;
                }
                TAGTYPE_BLOB => {
                    // TAGTYPE_BLOB: <u32 len><bytes...>
                    let len = self.read_u32_le()? as usize;
                    self.skip(len)?;
                }
                TAGTYPE_BSOB => {
                    // TAGTYPE_BSOB: <u8 len><bytes...>
                    let len = self.read_u8()? as usize;
                    self.skip(len)?;
                }
                TAGTYPE_ADDRESS => {
                    // TAGTYPE_ADDRESS
                    self.skip(I2P_DEST_LEN)?;
                }
                0x01 => {
                    // TAGTYPE_HASH16
                    self.skip(16)?;
                }
                TAGTYPE_STR1..=TAGTYPE_STR16 => {
                    // TAGTYPE_STR1..TAGTYPE_STR16 (length encoded in type)
                    let len = (ty - TAGTYPE_STR1 + 1) as usize;
                    self.skip(len)?;
                }
                other => {
                    // Unknown tag type; bail to avoid desyncing the stream.
                    bail!("unsupported tag type 0x{other:02x} for id={id}")
                }
            }
        }
        Ok(out)
    }

    fn skip(&mut self, len: usize) -> Result<()> {
        self.b
            .get(self.i..self.i + len)
            .ok_or_else(|| anyhow::anyhow!("unexpected EOF at {}", self.i))?;
        self.i += len;
        Ok(())
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
}
