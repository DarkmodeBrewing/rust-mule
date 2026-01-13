use crate::config::Config;

pub fn run(config: Config) {
    tracing::info!(log = %config.log_level, data_dir = %config.data_dir, "starting app");

    // Next milestone hook points:
    // - load node list
    // - start networking runtime
    // - connect to bootstrap node
}
