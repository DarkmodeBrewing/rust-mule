#[derive(Debug)]
enum MainError {
    LoadConfig(rust_mule::config_io::ConfigIoError),
    InvalidConfig(ConfigValidationError),
    App(rust_mule::app::AppError),
}

impl std::fmt::Display for MainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LoadConfig(source) => write!(f, "unable to load config: {source}"),
            Self::InvalidConfig(source) => write!(f, "{source}"),
            Self::App(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for MainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LoadConfig(source) => Some(source),
            Self::InvalidConfig(source) => Some(source),
            Self::App(source) => Some(source),
        }
    }
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
enum ConfigValidationError {
    InvalidSamHost {
        host: String,
        source: std::net::AddrParseError,
    },
    InvalidSamPort(u16),
    InvalidSamUdpPort(u16),
    InvalidSessionName(String),
    InvalidForwardHost {
        host: String,
        source: std::net::AddrParseError,
    },
    InvalidControlTimeout(u64),
    InvalidApiHost(rust_mule::config::ConfigError),
    InvalidApiPort(u16),
}

impl std::fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSamHost { host, .. } => write!(f, "Invalid sam.host '{}'", host),
            Self::InvalidSamPort(port) => write!(f, "Invalid sam.port '{}'", port),
            Self::InvalidSamUdpPort(port) => write!(f, "Invalid sam.udp_port '{}'", port),
            Self::InvalidSessionName(name) => write!(f, "Invalid sam.session_name '{}'", name),
            Self::InvalidForwardHost { host, .. } => {
                write!(f, "Invalid sam.forward_host '{}'", host)
            }
            Self::InvalidControlTimeout(v) => write!(f, "Invalid sam.control_timeout_secs '{}'", v),
            Self::InvalidApiHost(source) => write!(f, "{source}"),
            Self::InvalidApiPort(port) => write!(f, "Invalid api.port '{}'", port),
        }
    }
}

impl std::error::Error for ConfigValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidSamHost { source, .. } => Some(source),
            Self::InvalidForwardHost { source, .. } => Some(source),
            Self::InvalidApiHost(source) => Some(source),
            Self::InvalidSamPort(_)
            | Self::InvalidSamUdpPort(_)
            | Self::InvalidSessionName(_)
            | Self::InvalidControlTimeout(_)
            | Self::InvalidApiPort(_) => None,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<MainError>> {
    let config_path = std::path::PathBuf::from("config.toml");
    let cfg: rust_mule::config::Config = rust_mule::config_io::load_or_create_config(&config_path)
        .await
        .map_err(|e| Box::new(MainError::LoadConfig(e)))?;

    validate_cfg(&cfg).map_err(|e| Box::new(MainError::InvalidConfig(e)))?;

    rust_mule::config::init_tracing(&cfg);
    tracing::info!("rust-mule booting");

    rust_mule::app::run(cfg, config_path)
        .await
        .map_err(|e| Box::new(MainError::App(e)))?;
    Ok(())
}

fn validate_cfg(cfg: &rust_mule::config::Config) -> Result<(), ConfigValidationError> {
    use rust_mule::config::SamDatagramTransport;

    cfg.sam.host.parse::<std::net::IpAddr>().map_err(|source| {
        ConfigValidationError::InvalidSamHost {
            host: cfg.sam.host.clone(),
            source,
        }
    })?;

    if !(1..=65535).contains(&cfg.sam.port) {
        return Err(ConfigValidationError::InvalidSamPort(cfg.sam.port));
    }

    if matches!(cfg.sam.datagram_transport, SamDatagramTransport::UdpForward)
        && !(1..=65535).contains(&cfg.sam.udp_port)
    {
        return Err(ConfigValidationError::InvalidSamUdpPort(cfg.sam.udp_port));
    }

    if cfg.sam.session_name.trim().is_empty() {
        return Err(ConfigValidationError::InvalidSessionName(
            cfg.sam.session_name.clone(),
        ));
    }

    if matches!(cfg.sam.datagram_transport, SamDatagramTransport::UdpForward) {
        cfg.sam
            .forward_host
            .parse::<std::net::IpAddr>()
            .map_err(|source| ConfigValidationError::InvalidForwardHost {
                host: cfg.sam.forward_host.clone(),
                source,
            })?;
    }

    if cfg.sam.control_timeout_secs == 0 {
        return Err(ConfigValidationError::InvalidControlTimeout(
            cfg.sam.control_timeout_secs,
        ));
    }

    rust_mule::config::parse_api_bind_host(&cfg.api.host)
        .map_err(ConfigValidationError::InvalidApiHost)?;
    if !(1..=65535).contains(&cfg.api.port) {
        return Err(ConfigValidationError::InvalidApiPort(cfg.api.port));
    }

    Ok(())
}
