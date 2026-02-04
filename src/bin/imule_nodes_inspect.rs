use anyhow::Context;

fn usage() -> &'static str {
    "Usage: cargo run --bin imule_nodes_inspect -- <path/to/nodes.dat>"
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!(usage()))?;
    let nodes = rust_mule::nodes::imule::nodes_dat_contacts(&path)
        .await
        .with_context(|| format!("failed to parse {path}"))?;

    let verified = nodes.iter().filter(|n| n.verified).count();
    let versions = {
        let mut m = std::collections::BTreeMap::<u8, usize>::new();
        for n in &nodes {
            *m.entry(n.kad_version).or_default() += 1;
        }
        m
    };

    println!("nodes: {}", nodes.len());
    println!("verified: {verified}");
    println!("kad_version histogram:");
    for (v, c) in versions {
        println!("  v{v}: {c}");
    }

    // iMule-style short ids
    println!("sample dest hashCodes (low16):");
    for n in nodes.iter().take(10) {
        let h = n.udp_dest_hash_code();
        println!("  {:04x}", h & 0xFFFF);
    }

    Ok(())
}
