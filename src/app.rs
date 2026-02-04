use crate::{
    config::{Config, SamDatagramTransport},
    i2p::sam::{SamClient, SamDatagramSocket, SamDatagramTcp, SamKadSocket},
};
use anyhow::Context;
use std::{
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
};
use tokio::time::Duration;

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

    // Load or create a persistent Kad UDP obfuscation secret, iMule-style.
    //
    // If `config.toml` provides a non-zero value, we treat it as an explicit override.
    if config.kad.udp_key_secret == 0 {
        let udp_key_path = std::path::Path::new(&config.general.data_dir)
            .join("kad_udp_key_secret.dat");
        config.kad.udp_key_secret =
            crate::kad::udp_key::load_or_create_udp_key_secret(&udp_key_path).await?;
        tracing::info!(
            path = %udp_key_path.display(),
            "Loaded/generated Kad UDP key secret"
        );
    }

    tracing::info!(
        priv_len = config.i2p.sam_private_key.trim().len(),
        pub_len = config.i2p.sam_public_key.trim().len(),
        "Loaded SAM keys from config"
    );

    tracing::info!("Testing i2p + SAM connectivity");
    let mut sam: SamClient = SamClient::connect(&config.sam.host, config.sam.port)
        .await?
        .with_timeout(Duration::from_secs(config.sam.control_timeout_secs));
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

    let my_dest_hash: u32 = {
        let bytes = crate::i2p::b64::decode(&config.i2p.sam_public_key)
            .context("failed to decode i2p.sam_public_key as I2P base64")?;
        if bytes.len() < 4 {
            anyhow::bail!(
                "decoded i2p.sam_public_key is too short: {} bytes",
                bytes.len()
            );
        }
        u32::from_le_bytes(bytes[0..4].try_into().unwrap())
    };

    let kad_session_id = format!("{base_session_name}-kad");

    // KAD-over-I2P uses SAM `STYLE=DATAGRAM` sessions. We support both UDP-forwarding and
    // iMule-style TCP datagrams.
    let mut kad_sock: SamKadSocket = match config.sam.datagram_transport {
        SamDatagramTransport::Tcp => {
            let mut dg = SamDatagramTcp::connect(&config.sam.host, config.sam.port)
                .await?
                .with_timeout(Duration::from_secs(config.sam.control_timeout_secs));
            dg.hello("3.0", "3.3").await?;

            if let Err(err) = dg
                .session_create_datagram(
                    &kad_session_id,
                    &priv_key,
                    ["i2cp.messageReliability=BestEffort"],
                )
                .await
            {
                let msg = err.to_string();
                if msg.contains("Session already exists")
                    || msg.contains("DUPLICATED_ID")
                    || msg.contains("Duplicate")
                {
                    tracing::warn!(
                        session = %kad_session_id,
                        "SAM session exists; destroying and retrying"
                    );
                    let _ = dg.session_destroy(&kad_session_id).await;
                    dg.session_create_datagram(
                        &kad_session_id,
                        &priv_key,
                        ["i2cp.messageReliability=BestEffort"],
                    )
                    .await?;
                } else {
                    return Err(err);
                }
            }

            tracing::info!(
                session = %kad_session_id,
                transport = "tcp",
                "SAM DATAGRAM session ready"
            );
            SamKadSocket::Tcp(dg)
        }

        SamDatagramTransport::UdpForward => {
            let sam_host_ip: IpAddr = config.sam.host.parse()?;
            let sam_forward_ip: IpAddr = config.sam.forward_host.parse()?;

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
                    ["i2cp.messageReliability=BestEffort"],
                )
                .await
            {
                if err.to_string().contains("Session already exists") {
                    tracing::warn!(
                        session = %kad_session_id,
                        "SAM session exists; destroying and retrying"
                    );
                    let _ = sam.session_destroy(&kad_session_id).await;
                    sam.session_create_datagram_forward(
                        &kad_session_id,
                        &priv_key,
                        dg.forward_port(),
                        sam_forward_ip,
                        ["i2cp.messageReliability=BestEffort"],
                    )
                    .await?;
                } else {
                    return Err(err);
                }
            }

            tracing::info!(
                session = %kad_session_id,
                transport = "udp_forward",
                forward = %dg.forward_addr(),
                sam_udp = %dg.sam_udp_addr(),
                "SAM DATAGRAM session ready"
            );
            SamKadSocket::UdpForward(dg)
        }
    };

    let nodes_path = resolve_path(&config.general.data_dir, &config.kad.bootstrap_nodes_path);
    let nodes_path = pick_existing_nodes_dat(&nodes_path);
    let mut nodes = crate::nodes::imule::nodes_dat_contacts(&nodes_path).await?;
    tracing::info!(path = %nodes_path.display(), count = nodes.len(), "loaded nodes.dat");

    // If we are forced to fall back to repo-bundled reference nodes, try to refresh them via I2P.
    // iMule defaults to `http://www.imule.i2p/nodes2.dat`.
    if nodes_path.starts_with("source_ref") || nodes_path.starts_with("datfiles") {
        match try_download_nodes2_dat(
            &mut sam,
            &config.sam.host,
            config.sam.port,
            base_session_name,
            &priv_key,
        )
        .await
        {
            Ok(downloaded) => nodes = downloaded,
            Err(err) => {
                tracing::warn!(error = %err, "failed to download nodes2.dat over I2P; continuing with bundled nodes.dat")
            }
        }
    }

    crate::kad::bootstrap::bootstrap(
        &mut kad_sock,
        &nodes,
        crate::kad::bootstrap::BootstrapCrypto {
            my_kad_id: kad_prefs.kad_id,
            my_dest_hash,
            udp_key_secret: config.kad.udp_key_secret,
        },
        Default::default(),
    )
    .await?;

    tracing::info!("shutting down gracefully");
    Ok(())
}

async fn try_download_nodes2_dat(
    sam: &mut SamClient,
    sam_host: &str,
    sam_port: u16,
    base_session_name: &str,
    _priv_key: &str,
) -> anyhow::Result<Vec<crate::nodes::imule::ImuleNode>> {
    let url = "http://www.imule.i2p/nodes2.dat";
    let (host, port, path) = parse_http_url(url)?;
    if port != 80 {
        anyhow::bail!("only port 80 is supported for now (got {port})");
    }

    // Resolve eepsite hostname to destination.
    let dest = match sam.naming_lookup(host).await {
        Ok(d) => d,
        Err(err) if host.starts_with("www.") => {
            let alt = &host["www.".len()..];
            tracing::warn!(
                host,
                alt,
                error = %err,
                "NAMING LOOKUP failed; trying without 'www.'"
            );
            sam.naming_lookup(alt).await?
        }
        Err(err) => return Err(err),
    };

    // Ensure a STREAM session exists for outgoing HTTP.
    //
    // NOTE: SAM typically forbids attaching the same Destination to multiple sessions. Since we
    // already use our configured Destination for the KAD datagram session, create a temporary
    // Destination just for this download (iMule uses separate tcp/udp privkeys as well).
    let stream_id = format!("{base_session_name}-stream");
    let (stream_priv, _stream_pub) = sam.dest_generate().await?;

    let _ = sam.session_destroy(&stream_id).await;
    sam.session_create(
        "STREAM",
        &stream_id,
        &stream_priv,
        ["i2cp.messageReliability=BestEffort"],
    )
    .await?;

    let stream = crate::i2p::sam::SamStream::connect(sam_host, sam_port, &stream_id, &dest)
        .await
        .context("failed to STREAM CONNECT to nodes2.dat eepsite")?;
    let resp =
        crate::i2p::http::http_get_bytes(stream, host, path, Duration::from_secs(60)).await?;

    if resp.status != 200 {
        anyhow::bail!("nodes2.dat download returned HTTP {}", resp.status);
    }

    // Persist to data/nodes.dat for reproducible future runs.
    tokio::fs::create_dir_all("data").await.ok();
    let out = Path::new("data").join("nodes.dat");
    let tmp = out.with_extension("tmp");
    tokio::fs::write(&tmp, &resp.body).await?;
    tokio::fs::rename(&tmp, &out).await?;

    let nodes = crate::nodes::imule::parse_nodes_dat(&resp.body)
        .context("downloaded nodes2.dat was not a valid nodes.dat")?;

    tracing::info!(url, path = %out.display(), count = nodes.len(), "downloaded fresh nodes.dat over I2P");
    let _ = sam.session_destroy(&stream_id).await;
    Ok(nodes)
}

fn parse_http_url(url: &str) -> anyhow::Result<(&str, u16, &str)> {
    let url = url
        .strip_prefix("http://")
        .ok_or_else(|| anyhow::anyhow!("only http:// URLs are supported (got '{url}')"))?;

    let (host_port, path) = match url.find('/') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, "/"),
    };

    let (host, port) = if let Some((h, p)) = host_port.split_once(':') {
        (h, p.parse::<u16>()?)
    } else {
        (host_port, 80)
    };
    Ok((host, port, path))
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
    // Prefer `source_ref/nodes.dat` if present; it tends to be the freshest reference snapshot.
    for p in ["source_ref/nodes.dat", "datfiles/nodes.dat"] {
        let c = Path::new(p);
        if c.exists() {
            return c.to_path_buf();
        }
    }
    preferred.to_path_buf()
}
