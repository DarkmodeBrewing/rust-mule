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
pub const KADEMLIA2_HELLO_RES_ACK: u8 = 0x12;
pub const KADEMLIA2_PING: u8 = 0x1E;
pub const KADEMLIA2_PONG: u8 = 0x1F;

// Kademlia v1 (deprecated) opcodes. Still seen in the wild (and in iMule codepaths).
pub const KADEMLIA_HELLO_REQ_DEPRECATED: u8 = 0x03;
pub const KADEMLIA_HELLO_RES_DEPRECATED: u8 = 0x04;

pub const I2P_DEST_LEN: usize = 387;

// FileTags.h (iMule/aMule). Used in Kad2 HELLO taglists.
pub const TAG_KADMISCOPTIONS: u8 = 88; // 0x58

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

#[derive(Debug, Clone)]
pub struct Kad2Hello {
    pub kad_version: u8,
    pub node_id: KadId,
    pub udp_dest: [u8; I2P_DEST_LEN],
    /// Parsed taglist, limited to integer tags we care about.
    pub tags: BTreeMap<u8, u64>,
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

            let v = match ty {
                // TagTypes.h: uint types are 0x08..0x0B in aMule/iMule; we only need ints.
                0x08 => self.read_u8()? as u64,
                0x09 => self.read_u16_le()? as u64,
                0x0A => self.read_u32_le()? as u64,
                0x0B => {
                    let lo = self.read_u32_le()? as u64;
                    let hi = self.read_u32_le()? as u64;
                    (hi << 32) | lo
                }
                other => bail!("unsupported tag type 0x{other:02x} for id={id}"),
            };
            out.insert(id, v);
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
}
