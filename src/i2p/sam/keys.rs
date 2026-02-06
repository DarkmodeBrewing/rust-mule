use anyhow::{Context, Result, bail};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SamKeys {
    pub pub_key: String,
    pub priv_key: String,
}

impl SamKeys {
    pub fn parse(text: &str) -> Result<Self> {
        let mut pub_key: Option<String> = None;
        let mut priv_key: Option<String> = None;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                bail!("invalid sam.keys line (expected k=v): {line}");
            };
            let k = k.trim().to_ascii_uppercase();
            let v = v.trim().to_string();
            match k.as_str() {
                "PUB" => pub_key = Some(v),
                "PRIV" => priv_key = Some(v),
                other => bail!("unknown sam.keys key: {other}"),
            }
        }

        let pub_key = pub_key.ok_or_else(|| anyhow::anyhow!("sam.keys missing PUB="))?;
        let priv_key = priv_key.ok_or_else(|| anyhow::anyhow!("sam.keys missing PRIV="))?;
        Ok(Self { pub_key, priv_key })
    }

    pub fn to_string_kv(&self) -> String {
        // Keep it simple and greppable.
        format!(
            "PUB={}\nPRIV={}\n",
            self.pub_key.trim(),
            self.priv_key.trim()
        )
    }

    pub async fn load(path: &Path) -> Result<Option<Self>> {
        let bytes = match tokio::fs::read(path).await {
            Ok(b) => b,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(err).with_context(|| format!("failed to read {}", path.display()));
            }
        };
        let text = String::from_utf8(bytes)
            .with_context(|| format!("{} was not valid UTF-8", path.display()))?;
        Ok(Some(Self::parse(&text).with_context(|| {
            format!("failed to parse {}", path.display())
        })?))
    }

    pub async fn store(path: &Path, keys: &SamKeys) -> Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let tmp = path.with_extension("tmp");
        tokio::fs::write(&tmp, keys.to_string_kv())
            .await
            .with_context(|| format!("failed to write {}", tmp.display()))?;
        tokio::fs::rename(&tmp, path)
            .await
            .with_context(|| format!("failed to rename {} -> {}", tmp.display(), path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let perm = std::fs::Permissions::from_mode(0o600);
            let _ = std::fs::set_permissions(path, perm);
        }

        Ok(())
    }
}
