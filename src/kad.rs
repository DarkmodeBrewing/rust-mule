use anyhow::{Context, Result, bail};
use std::net::Ipv4Addr;

/// iMule (aMule-derived) uses a `preferencesKad.dat` file to persist the Kademlia "ClientID"
/// (128-bit node ID). We keep the same on-disk format so we can import/export directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KadId(pub [u8; 16]); // big-endian bytes, for display/interop

impl KadId {
    pub fn random() -> Result<Self> {
        let mut b = [0u8; 16];
        getrandom::getrandom(&mut b)
            .map_err(|e| anyhow::anyhow!("failed to generate random KadId: {e}"))?;
        Ok(Self(b))
    }

    pub fn to_hex_lower(self) -> String {
        let mut s = String::with_capacity(32);
        for b in self.0 {
            use std::fmt::Write as _;
            let _ = write!(&mut s, "{:02x}", b);
        }
        s
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PreferencesKad {
    pub ip: Ipv4Addr,
    pub kad_id: KadId,
}

impl PreferencesKad {
    pub fn new(ip: Ipv4Addr, kad_id: KadId) -> Self {
        Self { ip, kad_id }
    }

    /// Serialize to the on-disk format:
    /// - 4 bytes: IP address, LSB first
    /// - 2 bytes: deprecated / padding
    /// - 16 bytes: KadID as 4x u32 little-endian words, each word representing the big-endian
    ///   KadID chunk
    /// - optional: 1 byte (aMule writes a trailing 0; older variants may omit it)
    pub fn to_bytes(self) -> [u8; 23] {
        let mut out = [0u8; 23];

        let ip = self.ip.octets();
        out[0..4].copy_from_slice(&ip);

        out[4] = 0;
        out[5] = 0;

        // Encode the 128-bit ID:
        // Interpret each 4-byte chunk as a big-endian u32, then store as little-endian u32.
        for i in 0..4 {
            let chunk = &self.kad_id.0[(i * 4)..(i * 4 + 4)];
            let word_be = u32::from_be_bytes(chunk.try_into().unwrap());
            let word_le = word_be.to_le_bytes();
            out[(6 + i * 4)..(6 + i * 4 + 4)].copy_from_slice(&word_le);
        }

        // Trailing marker (optional, but matches aMule's common behavior)
        out[22] = 0;

        out
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 22 {
            bail!("preferencesKad.dat too small: {} bytes", bytes.len());
        }

        let ip = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);

        // bytes[4..6] are deprecated/padding, ignore.

        let mut id = [0u8; 16];
        for i in 0..4 {
            let le = u32::from_le_bytes(bytes[(6 + i * 4)..(6 + i * 4 + 4)].try_into().unwrap());
            id[(i * 4)..(i * 4 + 4)].copy_from_slice(&le.to_be_bytes());
        }

        Ok(Self {
            ip,
            kad_id: KadId(id),
        })
    }
}

pub async fn load_or_create_preferences_kad(path: &std::path::Path) -> Result<PreferencesKad> {
    if let Ok(bytes) = tokio::fs::read(path).await {
        return PreferencesKad::parse(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()));
    }

    let kad_id = KadId::random()?;
    let prefs = PreferencesKad::new(Ipv4Addr::new(0, 0, 0, 0), kad_id);

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed creating directory {}", parent.display()))?;
    }

    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, prefs.to_bytes())
        .await
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    tokio::fs::rename(&tmp, path)
        .await
        .with_context(|| format!("failed to rename {} -> {}", tmp.display(), path.display()))?;

    Ok(prefs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_preferences_kad() {
        let prefs = PreferencesKad::new(
            Ipv4Addr::new(1, 2, 3, 4),
            KadId([
                0x14, 0x52, 0xF1, 0xB4, 0x80, 0x9A, 0x17, 0x18, 0x8A, 0x29, 0x57, 0x44, 0x6F, 0x2B,
                0x3A, 0xB9,
            ]),
        );

        let bytes = prefs.to_bytes();
        let parsed = PreferencesKad::parse(&bytes).unwrap();
        assert_eq!(parsed.ip, prefs.ip);
        assert_eq!(parsed.kad_id, prefs.kad_id);
    }
}
