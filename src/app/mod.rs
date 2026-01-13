use crate::config::Config;

pub fn run(config: Config) {
    // Minimal "structured" boot message. Later: swap to tracing/log.
    eprintln!(
        "[rust-mule] booted | log={} | data_dir={}",
        config.log_level, config.data_dir
    );

    // Next milestone hook points:
    // - load node list
    // - start networking runtime
    // - connect to bootstrap node
}

