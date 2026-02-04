use anyhow::{Context, Result, bail};

pub mod bootstrap;
pub mod packed;
pub mod udp_crypto;
pub mod wire;

/// 128-bit Kademlia node ID (aMule/iMule: "ClientID").
///
/// Storage convention: big-endian bytes for display and interop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KadId(pub [u8; 16]);

impl KadId {
    pub fn random() -> Result<Self> {
        let mut b = [0u8; 16];
        getrandom::getrandom(&mut b)
            .map_err(|e| anyhow::anyhow!("failed to generate random KadId: {e}"))?;
        Ok(Self(b))
    }

    pub fn is_zero(self) -> bool {
        self.0.iter().all(|b| *b == 0)
    }

    pub fn to_hex_lower(self) -> String {
        let mut s = String::with_capacity(32);
        for b in self.0 {
            use std::fmt::Write as _;
            let _ = write!(&mut s, "{:02x}", b);
        }
        s
    }

    /// iMule/eMule "crypt value" format: four little-endian u32 chunks, in big-endian chunk order.
    ///
    /// This matches how iMule stores UInt128 values in files and uses them for UDP obfuscation keys.
    pub fn to_crypt_bytes(self) -> [u8; 16] {
        write_uint128_emule(self)
    }
}

/// `preferencesKad.dat` as written by aMule/iMule.
///
/// iMule reads:
/// - u16 (deprecated/unused)
/// - UInt128 (KadID)
///
/// and writes:
/// - u16 0
/// - UInt128 KadID
/// - u8 0 ("no tags" marker for older clients)
///
/// The UInt128 encoding is the aMule/eMule convention:
/// 4 x u32 little-endian words, in big-endian word order. See iMule `SafeFile.cpp`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreferencesKad {
    pub kad_id: KadId,
}

impl PreferencesKad {
    pub fn new(kad_id: KadId) -> Self {
        Self { kad_id }
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 2 + 16 {
            bail!("preferencesKad.dat too small: {} bytes", bytes.len());
        }

        // bytes[0..2] is deprecated/unused; keep for forward compatibility.
        let id = parse_uint128_emule(&bytes[2..18])?;
        Ok(Self { kad_id: id })
    }

    pub fn to_bytes(self) -> [u8; 19] {
        let mut out = [0u8; 19];

        // u16 0 (deprecated/unused)
        out[0..2].copy_from_slice(&0u16.to_le_bytes());

        // UInt128 (weird eMule/aMule encoding)
        out[2..18].copy_from_slice(&write_uint128_emule(self.kad_id));

        // "no tags"
        out[18] = 0;

        out
    }
}

pub async fn load_or_create_preferences_kad(path: &std::path::Path) -> Result<PreferencesKad> {
    if let Ok(bytes) = tokio::fs::read(path).await {
        let mut prefs = PreferencesKad::parse(&bytes)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        if prefs.kad_id.is_zero() {
            prefs.kad_id = KadId::random()?;
        }
        return Ok(prefs);
    }

    let prefs = PreferencesKad::new(KadId::random()?);

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

fn parse_uint128_emule(buf: &[u8]) -> Result<KadId> {
    if buf.len() < 16 {
        bail!("uint128 buffer too small: {} bytes", buf.len());
    }
    let mut id = [0u8; 16];
    for i in 0..4 {
        let le = u32::from_le_bytes(buf[(i * 4)..(i * 4 + 4)].try_into().unwrap());
        id[(i * 4)..(i * 4 + 4)].copy_from_slice(&le.to_be_bytes());
    }
    Ok(KadId(id))
}

fn write_uint128_emule(id: KadId) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..4 {
        let chunk = &id.0[(i * 4)..(i * 4 + 4)];
        let word_be = u32::from_be_bytes(chunk.try_into().unwrap());
        out[(i * 4)..(i * 4 + 4)].copy_from_slice(&word_be.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uint128_emule_round_trips() {
        let id = KadId([
            0x14, 0x52, 0xF1, 0xB4, 0x80, 0x9A, 0x17, 0x18, 0x8A, 0x29, 0x57, 0x44, 0x6F, 0x2B,
            0x3A, 0xB9,
        ]);

        let raw = write_uint128_emule(id);
        let parsed = parse_uint128_emule(&raw).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn preferences_kad_round_trips() {
        let prefs = PreferencesKad::new(KadId([1u8; 16]));
        let bytes = prefs.to_bytes();
        let parsed = PreferencesKad::parse(&bytes).unwrap();
        assert_eq!(parsed, prefs);
    }
}
