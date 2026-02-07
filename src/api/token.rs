use getrandom::getrandom;
use std::path::Path;

pub async fn load_or_create_token(path: &Path) -> anyhow::Result<String> {
    if let Ok(bytes) = tokio::fs::read(path).await {
        let s = String::from_utf8(bytes)?;
        let s = s.trim().to_string();
        if !s.is_empty() {
            return Ok(s);
        }
    }

    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    let mut raw = [0u8; 32];
    getrandom(&mut raw).map_err(|e| anyhow::anyhow!("getrandom failed: {e:?}"))?;
    let token = hex_lower(&raw);

    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, token.as_bytes()).await?;
    tokio::fs::rename(&tmp, path).await?;

    // Best-effort to restrict secrets on Unix; ignore failures (still portable).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let perm = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(path, perm);
    }

    Ok(token)
}

fn hex_lower(b: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(b.len() * 2);
    for v in b {
        let _ = write!(&mut out, "{v:02x}");
    }
    out
}
