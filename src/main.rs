#[derive(Debug)]
enum MainError {
    Cli(CliError),
    LoadConfig(rust_mule::config_io::ConfigIoError),
    InvalidConfig(ConfigValidationError),
    App(Box<rust_mule::app::AppError>),
}

impl std::fmt::Display for MainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cli(source) => write!(f, "{source}"),
            Self::LoadConfig(source) => write!(f, "unable to load config: {source}"),
            Self::InvalidConfig(source) => write!(f, "{source}"),
            Self::App(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for MainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Cli(source) => Some(source),
            Self::LoadConfig(source) => Some(source),
            Self::InvalidConfig(source) => Some(source),
            Self::App(source) => Some(source),
        }
    }
}

impl From<CliError> for MainError {
    fn from(value: CliError) -> Self {
        Self::Cli(value)
    }
}

impl From<rust_mule::config_io::ConfigIoError> for MainError {
    fn from(value: rust_mule::config_io::ConfigIoError) -> Self {
        Self::LoadConfig(value)
    }
}

impl From<ConfigValidationError> for MainError {
    fn from(value: ConfigValidationError) -> Self {
        Self::InvalidConfig(value)
    }
}

impl From<rust_mule::app::AppError> for MainError {
    fn from(value: rust_mule::app::AppError) -> Self {
        Self::App(Box::new(value))
    }
}

#[derive(Debug)]
enum CliError {
    MissingConfigPath,
    InvalidConfigPath(String),
    UnknownArgument(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingConfigPath => write!(f, "--config requires a path"),
            Self::InvalidConfigPath(path) => write!(f, "--config requires a path, got '{path}'"),
            Self::UnknownArgument(arg) => write!(f, "unknown argument: {arg}"),
        }
    }
}

impl std::error::Error for CliError {}

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
    InvalidMaxConcurrentTransferStreams(usize),
    InvalidApiPort(u16),
    InvalidShareRoot(rust_mule::share::ShareError),
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
            Self::InvalidMaxConcurrentTransferStreams(v) => {
                write!(f, "Invalid sam.max_concurrent_transfer_streams '{}'", v)
            }
            Self::InvalidApiPort(port) => write!(f, "Invalid api.port '{}'", port),
            Self::InvalidShareRoot(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for ConfigValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidSamHost { source, .. } => Some(source),
            Self::InvalidForwardHost { source, .. } => Some(source),
            Self::InvalidSamPort(_)
            | Self::InvalidSamUdpPort(_)
            | Self::InvalidSessionName(_)
            | Self::InvalidControlTimeout(_)
            | Self::InvalidMaxConcurrentTransferStreams(_)
            | Self::InvalidApiPort(_) => None,
            Self::InvalidShareRoot(source) => Some(source),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliOptions {
    config_path: std::path::PathBuf,
    mode: RunMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Run,
    CheckConfig,
    PrintHelp,
    PrintVersion,
}

#[tokio::main]
async fn main() -> Result<(), MainError> {
    let opts = parse_cli(std::env::args_os())?;

    match opts.mode {
        RunMode::PrintHelp => {
            print!("{}", help_text());
            return Ok(());
        }
        RunMode::PrintVersion => {
            println!("rust-mule {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        RunMode::Run | RunMode::CheckConfig => {}
    }

    let cfg: rust_mule::config::Config =
        rust_mule::config_io::load_config(&opts.config_path).await?;

    validate_cfg(&cfg)?;

    if matches!(opts.mode, RunMode::CheckConfig) {
        println!("config OK: {}", opts.config_path.display());
        return Ok(());
    }

    rust_mule::config::init_tracing(&cfg);
    tracing::info!(config_path = %opts.config_path.display(), "rust-mule booting");

    rust_mule::app::run(cfg, opts.config_path).await?;
    Ok(())
}

fn parse_cli<I, T>(args: I) -> Result<CliOptions, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString>,
{
    let mut config_path = std::path::PathBuf::from("config.toml");
    let mut mode = RunMode::Run;
    let mut args = args.into_iter().map(Into::into);
    let _program = args.next();

    while let Some(arg) = args.next() {
        match arg.to_string_lossy().as_ref() {
            "--config" => {
                let Some(path) = args.next() else {
                    return Err(CliError::MissingConfigPath);
                };
                let path_text = path.to_string_lossy();
                if path_text.starts_with('-') {
                    return Err(CliError::InvalidConfigPath(path_text.into_owned()));
                }
                config_path = std::path::PathBuf::from(path);
            }
            "--help" | "-?" => {
                return Ok(CliOptions {
                    config_path,
                    mode: RunMode::PrintHelp,
                });
            }
            "--version" => {
                return Ok(CliOptions {
                    config_path,
                    mode: RunMode::PrintVersion,
                });
            }
            "--check-config" => mode = RunMode::CheckConfig,
            other => return Err(CliError::UnknownArgument(other.to_string())),
        }
    }

    Ok(CliOptions { config_path, mode })
}

fn help_text() -> String {
    format!(
        "\
rust-mule {version}

Usage:
  rust-mule [--config <path>] [--check-config]
  rust-mule --help
  rust-mule -?
  rust-mule --version

Options:
  --config <path>   Path to config TOML (default: config.toml)
  --check-config    Validate config, print result, and exit
  --help, -?        Show this help text
  --version         Print version and exit
",
        version = env!("CARGO_PKG_VERSION")
    )
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

    if cfg.sam.max_concurrent_transfer_streams == 0 {
        return Err(ConfigValidationError::InvalidMaxConcurrentTransferStreams(
            cfg.sam.max_concurrent_transfer_streams,
        ));
    }

    if !(1..=65535).contains(&cfg.api.port) {
        return Err(ConfigValidationError::InvalidApiPort(cfg.api.port));
    }

    rust_mule::share::canonicalize_share_roots(
        &cfg.sharing.share_roots,
        std::path::Path::new(&cfg.general.data_dir),
    )
    .map_err(ConfigValidationError::InvalidShareRoot)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CliError, RunMode, parse_cli};
    use std::path::PathBuf;

    #[test]
    fn parse_cli_defaults_to_run_with_default_config() {
        let opts = parse_cli(["rust-mule"]).expect("parse");
        assert_eq!(opts.config_path, PathBuf::from("config.toml"));
        assert_eq!(opts.mode, RunMode::Run);
    }

    #[test]
    fn parse_cli_supports_config_path_and_check_config() {
        let opts = parse_cli(["rust-mule", "--config", "/tmp/alpha.toml", "--check-config"])
            .expect("parse");
        assert_eq!(opts.config_path, PathBuf::from("/tmp/alpha.toml"));
        assert_eq!(opts.mode, RunMode::CheckConfig);
    }

    #[test]
    fn parse_cli_supports_help_and_version() {
        assert_eq!(
            parse_cli(["rust-mule", "--help"]).expect("help").mode,
            RunMode::PrintHelp
        );
        assert_eq!(
            parse_cli(["rust-mule", "-?"]).expect("short help").mode,
            RunMode::PrintHelp
        );
        assert_eq!(
            parse_cli(["rust-mule", "--version"]).expect("version").mode,
            RunMode::PrintVersion
        );
    }

    #[test]
    fn parse_cli_short_circuits_help_and_version() {
        assert_eq!(
            parse_cli(["rust-mule", "--help", "--wat"])
                .expect("help short-circuit")
                .mode,
            RunMode::PrintHelp
        );
        assert_eq!(
            parse_cli(["rust-mule", "--version", "--wat"])
                .expect("version short-circuit")
                .mode,
            RunMode::PrintVersion
        );
    }

    #[test]
    fn parse_cli_rejects_missing_config_path() {
        let err = parse_cli(["rust-mule", "--config"]).expect_err("missing path");
        assert!(matches!(err, CliError::MissingConfigPath));
    }

    #[test]
    fn parse_cli_rejects_flag_like_config_path() {
        let err = parse_cli(["rust-mule", "--config", "--check-config"])
            .expect_err("invalid config path");
        assert!(matches!(
            err,
            CliError::InvalidConfigPath(path) if path == "--check-config"
        ));
    }

    #[test]
    fn parse_cli_rejects_unknown_argument() {
        let err = parse_cli(["rust-mule", "--wat"]).expect_err("unknown argument");
        assert!(matches!(err, CliError::UnknownArgument(arg) if arg == "--wat"));
    }
}
