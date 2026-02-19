use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use rust_mule::kad::md4;

#[derive(Serialize)]
struct DownloadFixture {
    file_name: String,
    file_size: u64,
    file_hash_md4_hex: String,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn fixture_for_path(path: &Path) -> Result<DownloadFixture> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to read metadata for {}", path.display()))?;
    if !metadata.is_file() {
        anyhow::bail!("not a regular file: {}", path.display());
    }

    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let digest = md4::digest(&bytes);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| path.display().to_string());

    Ok(DownloadFixture {
        file_name,
        file_size: metadata.len(),
        file_hash_md4_hex: hex(&digest),
    })
}

fn run(args: Vec<OsString>) -> Result<()> {
    if args.is_empty() {
        anyhow::bail!("usage: cargo run --quiet --bin download_fixture_gen -- <file> [<file>...]");
    }

    let mut fixtures = Vec::with_capacity(args.len());
    for arg in args {
        let path = Path::new(&arg);
        fixtures.push(fixture_for_path(path)?);
    }

    println!("{}", serde_json::to_string_pretty(&fixtures)?);
    Ok(())
}

fn main() -> Result<()> {
    run(env::args_os().skip(1).collect())
}
