use anyhow::{Result, bail};
use std::fs;

#[derive(Debug, Clone, Copy)]
pub enum NodesDatVersion {
    V0,
    V2,
}

pub async fn node_dat_version(path: &str) -> Result<(NodesDatVersion, u32, usize, usize)> {
    let bytes = fs::read(path)?;
    if bytes.len() < 4 {
        bail!("file too small");
    }

    let first = u32::from_le_bytes(bytes[0..4].try_into().unwrap());

    // v2: first 4 bytes are 0, then version=2, then count
    if first == 0 {
        if bytes.len() < 12 {
            bail!("v2 header too small");
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != 2 {
            bail!("unsupported nodes.dat version: {version}");
        }
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let header = 12usize;
        let entry = 34usize; // v2 contacts are 34 bytes :contentReference[oaicite:1]{index=1}
        let expected = header + (count as usize) * entry;
        Ok((NodesDatVersion::V2, count, expected, header))
    } else {
        // v0: first 4 bytes are count, then contacts are 25 bytes :contentReference[oaicite:2]{index=2}
        let count = first;
        let header = 4usize;
        let entry = 25usize;
        let expected = header + (count as usize) * entry;
        Ok((NodesDatVersion::V0, count, expected, header))
    }
}
