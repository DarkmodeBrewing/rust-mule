#[derive(Debug, Clone)]
pub struct Config {
    /// Enable more verbose output (wired to RUST_MULE_LOG=debug for now)
    pub log_level: String,

    /// Placeholder: where we later store node lists / state / downloads etc.
    pub data_dir: String,
}

impl Config {
    pub fn from_env() -> Self {
        // Keep it dead simple now. We'll upgrade to proper CLI parsing later.
        let log_level = std::env::var("RUST_MULE_LOG").unwrap_or_else(|_| "info".to_string());
        let data_dir = std::env::var("RUST_MULE_DATA_DIR").unwrap_or_else(|_| "./data".to_string());

        Self {
            log_level,
            data_dir,
        }
    }
}
