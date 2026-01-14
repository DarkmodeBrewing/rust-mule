use anyhow::{Context, Result, bail};
use std::{fs, net::Ipv4Addr};

#[derive(Debug, Clone, Copy)]
pub struct KadNode {
    pub node_id: [u8; 16],
    pub ip: Ipv4Addr,
    pub udp_port: u16,
    pub tcp_port: u16,
    pub kad_version: u8,
    pub kad_udp_key: u64,
    pub verified: bool,
}

pub async fn nodes_dat_contacts(path: &str) -> Result<Vec<KadNode>> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {path}"))?;

    // v2 header: 0x00000000, version=2, count
    if bytes.len() < 12 {
        bail!("nodes.dat too small: {} bytes", bytes.len());
    }

    let magic = u32::from_le_bytes(bytes[0..4].try_into()?);
    let version = u32::from_le_bytes(bytes[4..8].try_into()?);
    let count = u32::from_le_bytes(bytes[8..12].try_into()?) as usize;

    if magic != 0 {
        bail!("unexpected nodes.dat header (magic={magic}); expected v2 (magic=0)");
    }
    if version != 2 {
        bail!("unsupported nodes.dat version {version}; expected 2");
    }

    let entry_size = 34usize;
    let expected_min = 12usize + count * entry_size;
    if bytes.len() < expected_min {
        bail!(
            "nodes.dat truncated: expected at least {expected_min} bytes for {count} entries, got {}",
            bytes.len()
        );
    }

    let mut nodes = Vec::with_capacity(count);
    let mut offset = 12usize;

    let before = nodes.len();
    nodes.retain(|n: &KadNode| is_routable_public_ipv4(n.ip));
    tracing::info!(before, after = nodes.len(), "filtered bootstrap nodes");

    for _ in 0..count {
        let entry = &bytes[offset..offset + entry_size];
        nodes.push(parse_v2_contact(entry));
        offset += entry_size;
    }

    Ok(nodes)
}

fn parse_v2_contact(buf: &[u8]) -> KadNode {
    debug_assert!(buf.len() >= 34);

    let node_id: [u8; 16] = buf[0..16].try_into().unwrap();
    let ip = parse_ip_heuristic(buf);

    // Ports are little-endian
    let udp_port = u16::from_le_bytes(buf[20..22].try_into().unwrap());
    let tcp_port = u16::from_le_bytes(buf[22..24].try_into().unwrap());

    let kad_version = buf[24];
    let kad_udp_key = u64::from_le_bytes(buf[25..33].try_into().unwrap());
    let verified = buf[33] != 0;

    KadNode {
        node_id,
        ip,
        udp_port,
        tcp_port,
        kad_version,
        kad_udp_key,
        verified,
    }
}

fn is_routable_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();

    // Reject obvious nonsense for KAD bootstrap contacts
    if a == 0 || a == 127 {
        return false;
    }
    if a >= 224 {
        return false;
    } // multicast + reserved
    if a == 10 {
        return false;
    }
    if a == 172 && (16..=31).contains(&b) {
        return false;
    }
    if a == 192 && b == 168 {
        return false;
    }
    if a == 169 && b == 254 {
        return false;
    } // link-local
    true
}

fn parse_ip_heuristic(b16_19: &[u8]) -> Ipv4Addr {
    let ip_a = Ipv4Addr::new(b16_19[0], b16_19[1], b16_19[2], b16_19[3]); // as-is
    let ip_b = Ipv4Addr::new(b16_19[3], b16_19[2], b16_19[1], b16_19[0]); // reversed

    match (is_routable_public_ipv4(ip_a), is_routable_public_ipv4(ip_b)) {
        (true, false) => ip_a,
        (false, true) => ip_b,
        (true, true) => ip_a,
        (false, false) => ip_a,
    }
}
