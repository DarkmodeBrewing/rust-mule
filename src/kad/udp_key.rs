use anyhow::{Context, Result, bail};
use std::path::Path;

/// Load or create our local Kad UDP "key secret" (iMule `thePrefs::GetKadUDPKey()` equivalent).
///
/// We keep this **out of** `config.toml` so running the app doesn't mutate a tracked file.
/// The secret only needs to be stable for the lifetime of a node identity; storing it under
/// `data/` achieves that.
pub async fn load_or_create_udp_key_secret(path: &Path) -> Result<u32> {
    if let Ok(bytes) = tokio::fs::read(path).await {
        if bytes.len() != 4 {
            bail!(
                "kad udp key secret file has wrong size: {} bytes (expected 4): {}",
                bytes.len(),
                path.display()
            );
        }
        let v = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        // 0 is reserved as "unset" in config; avoid persisting it as a real value.
        if v != 0 {
            return Ok(v);
        }
    }

    let v: u32 = loop {
        let mut b = [0u8; 4];
        getrandom::getrandom(&mut b)
            .map_err(|e| anyhow::anyhow!("failed to generate kad udp key secret: {e}"))?;
        let v = u32::from_le_bytes(b);
        if v != 0 {
            break v;
        }
    };

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed creating directory {}", parent.display()))?;
    }

    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, v.to_le_bytes())
        .await
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    tokio::fs::rename(&tmp, path)
        .await
        .with_context(|| format!("failed to rename {} -> {}", tmp.display(), path.display()))?;

    Ok(v)
}
