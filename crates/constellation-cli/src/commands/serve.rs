//! `constellation serve` command — start the A2A server and discovery loop.

use anyhow::Result;
use constellation_discovery::{mdns::MdnsDiscoverer, tailscale::TailscaleDiscoverer, Discoverer};
use constellation_server::AppState;
use constellation_store::{peers as peers_store, Store};
use futures_util::future::join_all;
use std::{net::SocketAddr, path::Path, sync::Arc, time::Duration};
use tokio::net::TcpListener;

use crate::commands::{build_card_from_config, load_config};

/// Start the A2A HTTP server and background discovery loop, running until Ctrl-C.
pub async fn run(path: &Path) -> Result<()> {
    let cfg = load_config(path)?;
    let card = build_card_from_config(&cfg).await?;
    let store = Arc::new(Store::open(cfg.store_path())?);
    let bind: SocketAddr = cfg.network.bind.parse()?;
    let listener = TcpListener::bind(bind).await?;
    tracing::info!(%bind, "constellation a2a listener up");

    let app_state = AppState {
        store: store.clone(),
        card: card.clone(),
    };
    let serve_handle = tokio::spawn(async move {
        if let Err(e) = constellation_server::run(app_state, listener).await {
            tracing::error!(error=?e, "server exited");
        }
    });

    let port = bind.port();
    let mut discoverers: Vec<Box<dyn Discoverer>> = Vec::new();
    for d in &cfg.network.discovery {
        match d.as_str() {
            "tailscale" => discoverers.push(Box::new(TailscaleDiscoverer::new(
                port,
                std::time::Duration::from_secs(3),
            ))),
            "mdns" => match MdnsDiscoverer::new(card.name.clone()) {
                Ok(m) => {
                    if let Some(host_str) = card.url.host_str() {
                        match host_str.parse::<std::net::IpAddr>() {
                            Ok(ip) => {
                                if let Err(e) = m.advertise(&card.name, ip, port) {
                                    tracing::warn!(error=?e, "mdns advertise failed");
                                }
                            }
                            Err(_) => {
                                tracing::warn!(host=%host_str, "advertised host is not an IP; mdns advertisement skipped")
                            }
                        }
                    }
                    discoverers.push(Box::new(m));
                }
                Err(e) => tracing::warn!(error=?e, "mdns disabled"),
            },
            other => tracing::warn!(%other, "unknown discoverer"),
        }
    }

    let discovery_handle = tokio::spawn(async move {
        loop {
            let polls = discoverers
                .iter()
                .map(|d| async move { (d.name(), d.poll().await) });
            let results = join_all(polls).await;
            let mut all: Vec<constellation_discovery::DiscoveredPeer> = Vec::new();
            for (name, mut got) in results {
                tracing::debug!(discoverer = name, found = got.len(), "discovered");
                all.append(&mut got);
            }
            let store_for_writes = store.clone();
            match tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                for peer in all {
                    peers_store::upsert_peer(&store_for_writes, &peer.card, chrono::Utc::now())?;
                }
                let cutoff = chrono::Utc::now() - chrono::Duration::minutes(5);
                peers_store::prune_older_than(&store_for_writes, cutoff)?;
                Ok(())
            })
            .await
            {
                Err(e) => tracing::warn!(error=?e, "discovery store write join error"),
                Ok(Err(e)) => tracing::warn!(error=?e, "discovery store write error"),
                Ok(Ok(())) => {}
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });

    tokio::select! {
        _ = serve_handle => {},
        _ = discovery_handle => {},
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("shutdown requested");
        }
    }
    Ok(())
}
