#[derive(Debug)]
enum InspectError {
    Usage(&'static str),
    Parse {
        path: String,
        source: rust_mule::nodes::imule::ImuleNodesError,
    },
}

impl std::fmt::Display for InspectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(msg) => write!(f, "{msg}"),
            Self::Parse { path, .. } => write!(f, "failed to parse {path}"),
        }
    }
}

impl std::error::Error for InspectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Usage(_) => None,
            Self::Parse { source, .. } => Some(source),
        }
    }
}

fn usage() -> &'static str {
    "Usage: cargo run --bin imule_nodes_inspect -- <path/to/nodes.dat>"
}

#[tokio::main]
async fn main() -> Result<(), InspectError> {
    let path = std::env::args()
        .nth(1)
        .ok_or(InspectError::Usage(usage()))?;
    let nodes = rust_mule::nodes::imule::nodes_dat_contacts(&path)
        .await
        .map_err(|source| InspectError::Parse {
            path: path.clone(),
            source,
        })?;

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
