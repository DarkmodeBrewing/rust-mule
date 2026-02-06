use crate::{
    config::{Config, SamDatagramTransport},
    i2p::sam::{SamClient, SamDatagramSocket, SamDatagramTcp, SamKadSocket, SamKeys},
};
use anyhow::Context;
use std::collections::BTreeMap;
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
        let udp_key_path =
            std::path::Path::new(&config.general.data_dir).join("kad_udp_key_secret.dat");
        config.kad.udp_key_secret =
            crate::kad::udp_key::load_or_create_udp_key_secret(&udp_key_path).await?;
        tracing::info!(
            path = %udp_key_path.display(),
            "Loaded/generated Kad UDP key secret"
        );
    }

    tracing::info!("Loading SAM keys");

    tracing::info!("Testing i2p + SAM connectivity");
    let mut sam: SamClient = SamClient::connect(&config.sam.host, config.sam.port)
        .await?
        .with_timeout(Duration::from_secs(config.sam.control_timeout_secs));
    let reply = sam.hello("3.0", "3.3").await?;
    tracing::info!("(RAW) SAM Replies: {}", reply.raw);

    // Load SAM destination keys.
    //
    // Canonical location is `data/sam.keys` (under `general.data_dir`) so we don't commit secrets
    // in `config.toml`.
    let sam_keys_path = Path::new(&config.general.data_dir).join("sam.keys");
    let sam_keys: Option<SamKeys> = SamKeys::load(&sam_keys_path).await?;

    // Ensure we have a long-lived destination.
    let base_session_name = &config.sam.session_name;
    let keys: SamKeys = if let Some(k) = sam_keys {
        k
    } else {
        tracing::info!("No SAM destination in config; generating new identity");

        let (generated_priv, generated_pub) = sam.dest_generate().await?;
        tracing::info!(
            priv_len = generated_priv.len(),
            pub_len = generated_pub.len(),
            "DEST generated"
        );

        let keys = SamKeys {
            pub_key: generated_pub,
            priv_key: generated_priv,
        };
        SamKeys::store(&sam_keys_path, &keys).await?;
        tracing::info!(path = %sam_keys_path.display(), "saved SAM keys");
        keys
    };

    tracing::info!(
        priv_len = keys.priv_key.trim().len(),
        pub_len = keys.pub_key.trim().len(),
        path = %sam_keys_path.display(),
        "SAM keys ready"
    );

    let my_dest_bytes = crate::i2p::b64::decode(&keys.pub_key)
        .context("failed to decode SAM PUB key as I2P base64")?;
    if my_dest_bytes.len() != crate::kad::wire::I2P_DEST_LEN {
        anyhow::bail!(
            "decoded SAM PUB key has wrong length: {} bytes (expected {})",
            my_dest_bytes.len(),
            crate::kad::wire::I2P_DEST_LEN
        );
    }
    let my_dest_hash: u32 = u32::from_le_bytes(my_dest_bytes[0..4].try_into().unwrap());
    let my_dest: [u8; crate::kad::wire::I2P_DEST_LEN] = my_dest_bytes.try_into().unwrap();

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
                    &keys.priv_key,
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
                        &keys.priv_key,
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
                    &keys.priv_key,
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
                        &keys.priv_key,
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

    let data_dir = Path::new(&config.general.data_dir);
    let nodes_path = resolve_path(&config.general.data_dir, &config.kad.bootstrap_nodes_path);
    let preferred_nodes_path = nodes_path.clone();

    // Seed files under `data/` so we never depend on repo-local `source_ref/` paths at runtime.
    let initseed_path = data_dir.join("nodes.initseed.dat");
    let fallback_path = data_dir.join("nodes.fallback.dat");
    ensure_nodes_seed_files(&initseed_path, &fallback_path).await;

    // Pick which file we load from (and log why).
    let (nodes_path, nodes_source) =
        pick_nodes_dat(&preferred_nodes_path, &initseed_path, &fallback_path);
    let mut nodes = crate::nodes::imule::nodes_dat_contacts(&nodes_path).await?;
    tracing::info!(
        path = %nodes_path.display(),
        source = nodes_source,
        count = nodes.len(),
        "loaded nodes.dat"
    );

    // If our persisted `data/nodes.dat` ever shrinks to a tiny set (e.g. after a long eviction
    // cycle), re-seed it from repo-bundled reference nodes to avoid getting stuck with too few
    // bootstrap candidates on the next run.
    if nodes_path == preferred_nodes_path && nodes.len() < 50 {
        let original_count = nodes.len();
        let mut merged = BTreeMap::<String, crate::nodes::imule::ImuleNode>::new();
        for n in nodes.drain(..) {
            merged.insert(n.udp_dest_b64(), n);
        }

        for p in [&initseed_path, &fallback_path] {
            if !p.exists() {
                continue;
            }
            if let Ok(extra) = crate::nodes::imule::nodes_dat_contacts(p).await {
                for n in extra {
                    merged.entry(n.udp_dest_b64()).or_insert(n);
                }
            }
        }

        let mut out_nodes: Vec<_> = merged.into_values().collect();
        out_nodes.sort_by_key(|n| {
            (
                std::cmp::Reverse(n.verified),
                std::cmp::Reverse(n.kad_version),
            )
        });
        out_nodes.truncate(config.kad.service_max_persist_nodes.max(2000));

        if out_nodes.len() >= 50 && out_nodes.len() > original_count {
            tracing::warn!(
                from = original_count,
                to = out_nodes.len(),
                path = %preferred_nodes_path.display(),
                "nodes.dat seed pool was small; re-seeded from initseed/fallback"
            );
            let _ =
                crate::nodes::imule::persist_nodes_dat_v2(&preferred_nodes_path, &out_nodes).await;
            nodes = out_nodes;
        } else {
            nodes = out_nodes;
        }
    }

    // If we are not using the persisted `data/nodes.dat`, try to refresh over I2P.
    // iMule defaults to `http://www.imule.i2p/nodes2.dat`.
    if nodes_source != "primary" && nodes_source != "custom" {
        match try_download_nodes2_dat(
            &mut sam,
            &config.sam.host,
            config.sam.port,
            base_session_name,
            &keys.priv_key,
        )
        .await
        {
            Ok(downloaded) => nodes = downloaded,
            Err(err) => {
                tracing::warn!(error = %err, "failed to download nodes2.dat over I2P; continuing with bundled nodes.dat")
            }
        }
    }

    let outcome = crate::kad::bootstrap::bootstrap(
        &mut kad_sock,
        &nodes,
        crate::kad::bootstrap::BootstrapCrypto {
            my_kad_id: kad_prefs.kad_id,
            my_dest_hash,
            udp_key_secret: config.kad.udp_key_secret,
            my_dest,
        },
        Default::default(),
    )
    .await?;

    // Merge known nodes + discovered peers, persist once, and optionally continue in a long-lived
    // service loop (crawler + routing table).
    let out_path = resolve_path(&config.general.data_dir, &config.kad.bootstrap_nodes_path);
    let mut merged = BTreeMap::<String, crate::nodes::imule::ImuleNode>::new();

    for n in nodes.into_iter() {
        merged.insert(n.udp_dest_b64(), n);
    }

    for n in outcome.discovered {
        let k = n.udp_dest_b64();
        merged
            .entry(k)
            .and_modify(|old| {
                // Prefer newer/verified entries, and keep UDP keys if we learn them.
                if n.kad_version > old.kad_version {
                    old.kad_version = n.kad_version;
                }
                if n.verified {
                    old.verified = true;
                }
                if old.udp_key == 0 && n.udp_key != 0 {
                    old.udp_key = n.udp_key;
                    old.udp_key_ip = n.udp_key_ip;
                }
                if old.client_id == [0u8; 16] && n.client_id != [0u8; 16] {
                    old.client_id = n.client_id;
                }
            })
            .or_insert(n);
    }

    let mut out_nodes: Vec<crate::nodes::imule::ImuleNode> = merged.into_values().collect();
    out_nodes.sort_by_key(|n| {
        (
            std::cmp::Reverse(n.verified),
            std::cmp::Reverse(n.kad_version),
        )
    });
    out_nodes.truncate(config.kad.service_max_persist_nodes.max(2000));
    if let Err(err) = crate::nodes::imule::persist_nodes_dat_v2(&out_path, &out_nodes).await {
        tracing::warn!(
            error = %err,
            path = %out_path.display(),
            "failed to persist refreshed nodes.dat"
        );
    } else {
        tracing::info!(
            path = %out_path.display(),
            count = out_nodes.len(),
            "persisted refreshed nodes.dat"
        );
    }

    if config.kad.service_enabled {
        let mut svc = crate::kad::service::KadService::new(kad_prefs.kad_id);
        crate::kad::service::run_service(
            &mut svc,
            &mut kad_sock,
            out_nodes,
            crate::kad::service::KadServiceCrypto {
                my_kad_id: kad_prefs.kad_id,
                my_dest_hash,
                udp_key_secret: config.kad.udp_key_secret,
                my_dest,
            },
            crate::kad::service::KadServiceConfig {
                runtime_secs: config.kad.service_runtime_secs,
                crawl_every_secs: config.kad.service_crawl_every_secs,
                persist_every_secs: config.kad.service_persist_every_secs,
                alpha: config.kad.service_alpha,
                req_contacts: config.kad.service_req_contacts,
                max_persist_nodes: config.kad.service_max_persist_nodes,
                req_timeout_secs: config.kad.service_req_timeout_secs,
                req_min_interval_secs: config.kad.service_req_min_interval_secs,
                hello_every_secs: config.kad.service_hello_every_secs,
                hello_batch: config.kad.service_hello_batch,
                hello_min_interval_secs: config.kad.service_hello_min_interval_secs,
                maintenance_every_secs: config.kad.service_maintenance_every_secs,
                status_every_secs: config.kad.service_status_every_secs,
                max_failures: config.kad.service_max_failures,
                evict_age_secs: config.kad.service_evict_age_secs,
            },
            &out_path,
        )
        .await?;
    }

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

fn pick_nodes_dat(preferred: &Path, initseed: &Path, fallback: &Path) -> (PathBuf, &'static str) {
    if preferred.exists() {
        return (preferred.to_path_buf(), "primary");
    }

    // Only fall back automatically for the default `data/nodes.dat` case.
    let preferred_name = preferred.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if preferred_name != "nodes.dat" {
        return (preferred.to_path_buf(), "custom");
    }

    if initseed.exists() {
        return (initseed.to_path_buf(), "initseed");
    }
    if fallback.exists() {
        return (fallback.to_path_buf(), "fallback");
    }

    (preferred.to_path_buf(), "missing")
}

async fn ensure_nodes_seed_files(initseed: &Path, fallback: &Path) {
    // We intentionally keep this best-effort: failures should not prevent startup.
    let _ = tokio::fs::create_dir_all(initseed.parent().unwrap_or_else(|| Path::new("data"))).await;

    if !initseed.exists() {
        for p in ["source_ref/nodes.dat", "datfiles/nodes.dat"] {
            let src = Path::new(p);
            if !src.exists() {
                continue;
            }
            if let Ok(bytes) = tokio::fs::read(src).await {
                let tmp = initseed.with_extension("tmp");
                if tokio::fs::write(&tmp, bytes).await.is_ok()
                    && tokio::fs::rename(&tmp, initseed).await.is_ok()
                {
                    tracing::info!(
                        src = %src.display(),
                        dst = %initseed.display(),
                        "created nodes.initseed.dat"
                    );
                    break;
                }
            }
        }
    }

    if !fallback.exists() {
        let mut merged = BTreeMap::<String, crate::nodes::imule::ImuleNode>::new();
        for p in [
            initseed,
            Path::new("source_ref/nodes.dat"),
            Path::new("datfiles/nodes.dat"),
        ] {
            if !p.exists() {
                continue;
            }
            if let Ok(nodes) = crate::nodes::imule::nodes_dat_contacts(p).await {
                for n in nodes {
                    merged.entry(n.udp_dest_b64()).or_insert(n);
                }
            }
        }

        if !merged.is_empty() {
            let mut nodes: Vec<_> = merged.into_values().collect();
            nodes.sort_by_key(|n| {
                (
                    std::cmp::Reverse(n.verified),
                    std::cmp::Reverse(n.kad_version),
                )
            });
            if crate::nodes::imule::persist_nodes_dat_v2(fallback, &nodes)
                .await
                .is_ok()
            {
                tracing::info!(
                    dst = %fallback.display(),
                    count = nodes.len(),
                    "created nodes.fallback.dat"
                );
            }
        }
    }
}
