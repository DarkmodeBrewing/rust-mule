use crate::{
    config::Config,
    i2p::sam::{SamClient, SamDatagramSocket},
};
use std::{
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
};

pub async fn run(mut config: Config) -> anyhow::Result<()> {
    tracing::info!(
        log = %config.general.log_level,
        data_dir = %config.general.data_dir,
        "starting app"
    );

    // Load or create aMule/iMule-compatible KadID.
    let prefs_path =
        std::path::Path::new(&config.general.data_dir).join(&config.kad.preferences_kad_path);
    let kad_prefs = crate::kad::load_or_create_preferences_kad(&prefs_path).await?;
    tracing::info!(
        kad_id = %kad_prefs.kad_id.to_hex_lower(),
        prefs = %prefs_path.display(),
        "Loaded Kademlia identity"
    );

    tracing::info!(
        priv_len = config.i2p.sam_private_key.trim().len(),
        pub_len = config.i2p.sam_public_key.trim().len(),
        "Loaded SAM keys from config"
    );

    tracing::info!("Testing i2p + SAM connectivity");
    let mut sam: SamClient = SamClient::connect(&config.sam.host, config.sam.port).await?;
    let reply = sam.hello("3.0", "3.3").await?;
    tracing::info!("(RAW) SAM Replies: {}", reply.raw);

    // Ensure we have a long-lived destination.
    let base_session_name = &config.sam.session_name;
    let priv_key: String = if !config.i2p.sam_private_key.trim().is_empty() {
        config.i2p.sam_private_key.clone()
    } else {
        tracing::info!("No SAM destination in config; generating new identity");

        let (generated_priv, generated_pub) = sam.dest_generate().await?;
        tracing::info!(
            priv_len = generated_priv.len(),
            pub_len = generated_pub.len(),
            "DEST generated"
        );

        config.i2p.sam_private_key = generated_priv.clone();
        config.i2p.sam_public_key = generated_pub.clone();
        config.persist().await?;

        generated_priv
    };

    // KAD-over-I2P uses SAM DATAGRAM sessions and UDP forwarding.
    let sam_host_ip: IpAddr = config.sam.host.parse()?;
    let sam_forward_ip: IpAddr = config.sam.forward_host.parse()?;
    let kad_session_id = format!("{base_session_name}-kad");

    if sam_forward_ip.is_loopback() && !sam_host_ip.is_loopback() {
        tracing::warn!(
            sam_host = %sam_host_ip,
            forward_host = %sam_forward_ip,
            "sam.forward_host is loopback but the SAM bridge is remote; UDP forwarding will not reach this process"
        );
    }
    if config.sam.forward_port == 0 && sam_host_ip != sam_forward_ip {
        tracing::warn!(
            sam_host = %sam_host_ip,
            forward_host = %sam_forward_ip,
            "sam.forward_port=0 (ephemeral) with remote SAM bridge; consider setting a fixed forward_port and opening/mapping it for UDP"
        );
    }

    let bind_ip = if sam_forward_ip.is_loopback() {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    } else {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    };

    let dg: SamDatagramSocket = SamDatagramSocket::bind_for_forwarding(
        kad_session_id.clone(),
        sam_host_ip,
        config.sam.udp_port,
        bind_ip,
        config.sam.forward_port,
    )
    .await?;

    // Create SAM datagram session (idempotent-ish).
    if let Err(err) = sam
        .session_create_datagram_forward(
            &kad_session_id,
            &priv_key,
            dg.forward_port(),
            sam_forward_ip,
            ["i2cp.leaseSetEncType=4"],
        )
        .await
    {
        if err.to_string().contains("Session already exists") {
            tracing::warn!(session = %kad_session_id, "SAM session exists; destroying and retrying");
            let _ = sam.session_destroy(&kad_session_id).await;
            sam.session_create_datagram_forward(
                &kad_session_id,
                &priv_key,
                dg.forward_port(),
                sam_forward_ip,
                ["i2cp.leaseSetEncType=4"],
            )
            .await?;
        } else {
            return Err(err);
        }
    }

    tracing::info!(
        session = %kad_session_id,
        forward = %dg.forward_addr(),
        sam_udp = %dg.sam_udp_addr(),
        "SAM DATAGRAM session ready"
    );

    let nodes_path = resolve_path(&config.general.data_dir, &config.kad.bootstrap_nodes_path);
    let nodes_path = pick_existing_nodes_dat(&nodes_path);
    let nodes = crate::nodes::imule::nodes_dat_contacts(&nodes_path).await?;
    tracing::info!(path = %nodes_path.display(), count = nodes.len(), "loaded nodes.dat");

    crate::kad::bootstrap::bootstrap(
        &dg,
        &nodes,
        crate::kad::bootstrap::BootstrapConfig::default(),
    )
    .await?;

    tracing::info!("shutting down gracefully");
    Ok(())
}

fn resolve_path(data_dir: &str, p: &str) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    Path::new(data_dir).join(path)
}

fn pick_existing_nodes_dat(preferred: &Path) -> PathBuf {
    if preferred.exists() {
        return preferred.to_path_buf();
    }
    // Developer-friendly fallbacks.
    for p in ["datfiles/nodes.dat", "source_ref/nodes.dat"] {
        let c = Path::new(p);
        if c.exists() {
            return c.to_path_buf();
        }
    }
    preferred.to_path_buf()
}
