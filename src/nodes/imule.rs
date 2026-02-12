use std::path::Path;

pub type Result<T> = std::result::Result<T, ImuleNodesError>;

#[derive(Debug)]
pub enum ImuleNodesError {
    Read {
        path: String,
        source: std::io::Error,
    },
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    Write {
        path: String,
        source: std::io::Error,
    },
    Rename {
        from: String,
        to: String,
        source: std::io::Error,
    },
    TooManyNodes(usize),
    InvalidFormat(String),
    UnexpectedEof {
        offset: usize,
    },
}

impl std::fmt::Display for ImuleNodesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, .. } => write!(f, "failed to read {path}"),
            Self::CreateDir { path, .. } => write!(f, "failed creating directory {path}"),
            Self::Write { path, .. } => write!(f, "failed to write {path}"),
            Self::Rename { from, to, .. } => write!(f, "failed to rename {from} -> {to}"),
            Self::TooManyNodes(count) => write!(f, "too many nodes to encode: {count}"),
            Self::InvalidFormat(msg) => write!(f, "{msg}"),
            Self::UnexpectedEof { offset } => write!(f, "unexpected EOF at offset {offset}"),
        }
    }
}

impl std::error::Error for ImuleNodesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::CreateDir { source, .. } => Some(source),
            Self::Write { source, .. } => Some(source),
            Self::Rename { source, .. } => Some(source),
            Self::TooManyNodes(_) | Self::InvalidFormat(_) | Self::UnexpectedEof { .. } => None,
        }
    }
}

/// iMule/aMule I2P-only `nodes.dat` contact (version 2 file format).
///
/// File layout (see iMule `CRoutingZone::WriteFile` and `CContact::WriteToFile`):
/// - u32 magic = 0
/// - u32 version = 2
/// - u32 count
/// - repeated `count` times:
///   - u8  kad_version
///   - u128 client_id (aMule/eMule "weird" encoding)
///   - 387 bytes UDP destination (raw I2P public key bytes)
///   - u32 udp_key
///   - u32 udp_key_ip (iMule uses dest hash here)
///   - u8  verified (0/1)
#[derive(Debug, Clone)]
pub struct ImuleNode {
    pub kad_version: u8,
    pub client_id: [u8; 16],
    pub udp_dest: [u8; 387],
    pub udp_key: u32,
    pub udp_key_ip: u32,
    pub verified: bool,
}

impl ImuleNode {
    /// Mirrors iMule's `CI2PAddress::hashCode()`:
    /// reinterpret the first 4 destination bytes as little-endian u32.
    pub fn udp_dest_hash_code(&self) -> u32 {
        let mut b = [0u8; 4];
        b.copy_from_slice(&self.udp_dest[..4]);
        u32::from_le_bytes(b)
    }

    /// Encode the raw 387-byte destination using I2P base64 (`-~` alphabet).
    pub fn udp_dest_b64(&self) -> String {
        crate::i2p::b64::encode(&self.udp_dest)
    }
}

pub async fn nodes_dat_contacts(path: impl AsRef<std::path::Path>) -> Result<Vec<ImuleNode>> {
    let path = path.as_ref();
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|source| ImuleNodesError::Read {
            path: path.display().to_string(),
            source,
        })?;
    parse_nodes_dat(&bytes)
}

/// Persist a `nodes.dat` v2 file (iMule/aMule format).
///
/// This is the same format we parse in [`parse_nodes_dat_v2`].
pub async fn persist_nodes_dat_v2(path: &Path, nodes: &[ImuleNode]) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|source| ImuleNodesError::CreateDir {
                path: parent.display().to_string(),
                source,
            })?;
    }

    let tmp = path.with_extension("tmp");
    let bytes = encode_nodes_dat_v2(nodes)?;
    tokio::fs::write(&tmp, bytes)
        .await
        .map_err(|source| ImuleNodesError::Write {
            path: tmp.display().to_string(),
            source,
        })?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|source| ImuleNodesError::Rename {
            from: tmp.display().to_string(),
            to: path.display().to_string(),
            source,
        })?;
    Ok(())
}

pub fn encode_nodes_dat_v2(nodes: &[ImuleNode]) -> Result<Vec<u8>> {
    let count: u32 = nodes
        .len()
        .try_into()
        .map_err(|_| ImuleNodesError::TooManyNodes(nodes.len()))?;

    let entry_size = 1 + 16 + 387 + 4 + 4 + 1;
    let mut out = Vec::with_capacity(12 + nodes.len() * entry_size);

    out.extend_from_slice(&0u32.to_le_bytes()); // magic
    out.extend_from_slice(&2u32.to_le_bytes()); // version
    out.extend_from_slice(&count.to_le_bytes());

    for n in nodes {
        out.push(n.kad_version);
        out.extend_from_slice(&write_uint128_emule(&n.client_id));
        out.extend_from_slice(&n.udp_dest);
        out.extend_from_slice(&n.udp_key.to_le_bytes());
        out.extend_from_slice(&n.udp_key_ip.to_le_bytes());
        out.push(if n.verified { 1 } else { 0 });
    }

    Ok(out)
}

pub fn parse_nodes_dat(bytes: &[u8]) -> Result<Vec<ImuleNode>> {
    if bytes.len() < 4 {
        return Err(ImuleNodesError::InvalidFormat(format!(
            "nodes.dat too small: {} bytes",
            bytes.len()
        )));
    }

    let first = read_u32_le(bytes, 0)?;

    if first == 0 {
        parse_nodes_dat_v2(bytes)
    } else {
        // "bootstrap nodes.dat" style used by aMule/iMule: starts with count (v1-ish).
        parse_nodes_dat_bootstrap_v1(bytes)
    }
}

fn write_uint128_emule(id_be: &[u8; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let chunk = &id_be[(i * 4)..(i * 4 + 4)];
        let word_be = u32::from_be_bytes(chunk.try_into().unwrap());
        out[(i * 4)..(i * 4 + 4)].copy_from_slice(&word_be.to_le_bytes());
    }
    out
}

fn parse_nodes_dat_v2(bytes: &[u8]) -> Result<Vec<ImuleNode>> {
    if bytes.len() < 12 {
        return Err(ImuleNodesError::InvalidFormat(format!(
            "nodes.dat v2 header too small: {} bytes",
            bytes.len()
        )));
    }

    let magic = read_u32_le(bytes, 0)?;
    let version = read_u32_le(bytes, 4)?;
    let count = read_u32_le(bytes, 8)? as usize;

    if magic != 0 {
        return Err(ImuleNodesError::InvalidFormat(format!(
            "unexpected nodes.dat magic={magic}, expected 0"
        )));
    }
    if version != 2 {
        return Err(ImuleNodesError::InvalidFormat(format!(
            "unsupported nodes.dat version={version}, expected 2"
        )));
    }

    let entry_size = 1 + 16 + 387 + 4 + 4 + 1;
    let needed = 12 + count * entry_size;
    if bytes.len() < needed {
        return Err(ImuleNodesError::InvalidFormat(format!(
            "nodes.dat truncated: expected at least {needed} bytes for {count} entries, got {}",
            bytes.len()
        )));
    }

    let mut out = Vec::with_capacity(count);
    let mut off = 12usize;

    for _ in 0..count {
        let kad_version = read_u8(bytes, off)?;
        off += 1;

        let client_id = read_uint128_emule(bytes, off)?;
        off += 16;

        let udp_dest: [u8; 387] = bytes
            .get(off..off + 387)
            .ok_or_else(|| ImuleNodesError::InvalidFormat("truncated udp_dest".to_string()))?
            .try_into()
            .unwrap();
        off += 387;

        let udp_key = read_u32_le(bytes, off)?;
        off += 4;
        let udp_key_ip = read_u32_le(bytes, off)?;
        off += 4;

        let verified = read_u8(bytes, off)? != 0;
        off += 1;

        out.push(ImuleNode {
            kad_version,
            client_id,
            udp_dest,
            udp_key,
            udp_key_ip,
            verified,
        });
    }

    Ok(out)
}

fn parse_nodes_dat_bootstrap_v1(bytes: &[u8]) -> Result<Vec<ImuleNode>> {
    // iMule reads these as:
    // - u32 count
    // - repeated: u8 version, u128 client_id, 387 bytes udp_dest
    // (no udpkey / verified)
    if bytes.len() < 4 {
        return Err(ImuleNodesError::InvalidFormat(
            "bootstrap nodes.dat too small".to_string(),
        ));
    }
    let count = read_u32_le(bytes, 0)? as usize;
    let entry_size = 1 + 16 + 387;
    let needed = 4 + count * entry_size;
    if bytes.len() < needed {
        return Err(ImuleNodesError::InvalidFormat(format!(
            "bootstrap nodes.dat truncated: expected at least {needed} bytes for {count} entries, got {}",
            bytes.len()
        )));
    }

    let mut out = Vec::with_capacity(count);
    let mut off = 4usize;
    for _ in 0..count {
        let kad_version = read_u8(bytes, off)?;
        off += 1;

        let client_id = read_uint128_emule(bytes, off)?;
        off += 16;

        let udp_dest: [u8; 387] = bytes
            .get(off..off + 387)
            .ok_or_else(|| ImuleNodesError::InvalidFormat("truncated udp_dest".to_string()))?
            .try_into()
            .unwrap();
        off += 387;

        out.push(ImuleNode {
            kad_version,
            client_id,
            udp_dest,
            udp_key: 0,
            udp_key_ip: 0,
            verified: false,
        });
    }

    Ok(out)
}

fn read_u8(bytes: &[u8], off: usize) -> Result<u8> {
    bytes
        .get(off)
        .copied()
        .ok_or(ImuleNodesError::UnexpectedEof { offset: off })
}

fn read_u32_le(bytes: &[u8], off: usize) -> Result<u32> {
    let b: [u8; 4] = bytes
        .get(off..off + 4)
        .ok_or(ImuleNodesError::UnexpectedEof { offset: off })?
        .try_into()
        .unwrap();
    Ok(u32::from_le_bytes(b))
}

fn read_uint128_emule(bytes: &[u8], off: usize) -> Result<[u8; 16]> {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let le = read_u32_le(bytes, off + i * 4)?;
        out[i * 4..i * 4 + 4].copy_from_slice(&le.to_be_bytes());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v2_header_and_one_contact() {
        let mut b = Vec::new();
        b.extend_from_slice(&0u32.to_le_bytes()); // magic
        b.extend_from_slice(&2u32.to_le_bytes()); // version
        b.extend_from_slice(&1u32.to_le_bytes()); // count

        b.push(8); // kad_version

        // client_id: 16 bytes in emule encoding (4x u32 le).
        for _ in 0..4 {
            b.extend_from_slice(&0x11223344u32.to_le_bytes());
        }

        b.extend_from_slice(&[0xAA; 387]); // udp_dest
        b.extend_from_slice(&0xAABBCCDDu32.to_le_bytes()); // udp_key
        b.extend_from_slice(&0x01020304u32.to_le_bytes()); // udp_key_ip
        b.push(1); // verified

        let nodes = parse_nodes_dat(&b).unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].kad_version, 8);
        assert_eq!(nodes[0].udp_key, 0xAABBCCDD);
        assert_eq!(nodes[0].udp_key_ip, 0x01020304);
        assert!(nodes[0].verified);
    }
}
