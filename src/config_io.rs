use crate::config::Config;
use std::{fs, path::Path};

pub type Result<T> = std::result::Result<T, ConfigIoError>;

#[derive(Debug)]
pub enum ConfigIoError {
    CreateDefault {
        path: String,
        source: Box<ConfigIoError>,
    },
    Read {
        path: String,
        source: std::io::Error,
    },
    ParseToml {
        path: String,
        source: toml::de::Error,
    },
    SerializeToml {
        source: toml::ser::Error,
    },
    CreateDir {
        path: String,
        source: std::io::Error,
    },
    Write {
        path: String,
        source: std::io::Error,
    },
}

impl std::fmt::Display for ConfigIoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateDefault { path, .. } => {
                write!(f, "failed to create default config at {path}")
            }
            Self::Read { path, .. } => write!(f, "failed reading config file {path}"),
            Self::ParseToml { path, .. } => write!(f, "invalid TOML in {path}"),
            Self::SerializeToml { .. } => write!(f, "failed serializing config to TOML"),
            Self::CreateDir { path, .. } => write!(f, "failed creating directory {path}"),
            Self::Write { path, .. } => write!(f, "failed writing config file {path}"),
        }
    }
}

impl std::error::Error for ConfigIoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDefault { source, .. } => Some(source.as_ref()),
            Self::Read { source, .. } => Some(source),
            Self::ParseToml { source, .. } => Some(source),
            Self::SerializeToml { source } => Some(source),
            Self::CreateDir { source, .. } => Some(source),
            Self::Write { source, .. } => Some(source),
        }
    }
}

pub async fn load_or_create_config(path: impl AsRef<Path>) -> Result<Config> {
    let path = path.as_ref();

    if !path.exists() {
        let default_cfg: Config = Config::default();
        save_config(path, &default_cfg)
            .await
            .map_err(|source| ConfigIoError::CreateDefault {
                path: path.display().to_string(),
                source: Box::new(source),
            })?;
        return Ok(default_cfg);
    }

    let content: String = fs::read_to_string(path).map_err(|source| ConfigIoError::Read {
        path: path.display().to_string(),
        source,
    })?;

    let cfg: Config = toml::from_str(&content).map_err(|source| ConfigIoError::ParseToml {
        path: path.display().to_string(),
        source,
    })?;

    Ok(cfg)
}

pub async fn save_config(path: impl AsRef<Path>, cfg: &Config) -> Result<()> {
    let path = path.as_ref();

    // Pretty output TOML
    let toml_string =
        toml::to_string_pretty(cfg).map_err(|source| ConfigIoError::SerializeToml { source })?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ConfigIoError::CreateDir {
            path: parent.display().to_string(),
            source,
        })?;
    }

    fs::write(path, toml_string).map_err(|source| ConfigIoError::Write {
        path: path.display().to_string(),
        source,
    })?;

    Ok(())
}
