use tracing_subscriber::EnvFilter;
#[derive(Debug, Clone)]
pub struct Config {
    pub log_level: String,
    pub data_dir: String,
    pub tcp_probe_target: String,
}

impl Config {
    pub fn from_env() -> Self {
        // Keep it dead simple now. We'll upgrade to proper CLI parsing later.
        let log_level = std::env::var("RUST_MULE_LOG").unwrap_or_else(|_| "info".to_string());
        let data_dir = std::env::var("RUST_MULE_DATA_DIR").unwrap_or_else(|_| "./data".to_string());
        let tcp_probe_target =
            std::env::var("RUST_MULE_TCP_PROBE").unwrap_or_else(|_| "1.1.1.1:443".to_string());
        Self {
            log_level,
            data_dir,
            tcp_probe_target,
        }
    }
}

pub fn init_tracing(config: &Config) {
    // Priority order:
    // 1) RUST_LOG (standard in Rust ecosystem)
    // 2) RUST_MULE_LOG (your app-specific env)
    // 3) default (info)
    //
    // Example:
    // RUST_LOG=info,rust_mule=debug
    // RUST_MULE_LOG=debug

    let env_filter = std::env::var("RUST_LOG")
        .ok()
        .or_else(|| Some(config.log_level.clone()))
        .unwrap_or_else(|| "info".to_string());

    let filter = EnvFilter::try_new(env_filter).unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .compact()
        .init();
}
