use anyhow::Result;
use constellation_discovery::{
    mdns::MdnsDiscoverer, tailscale::TailscaleDiscoverer, DiscoveredPeer, Discoverer,
};
use constellation_server::{build_app, AppState};
use constellation_store::{peers as peers_store, Store};
use std::{net::SocketAddr, path::Path, sync::Arc, time::Duration};
use tokio::net::TcpListener;

use crate::commands::{build_card_from_config, load_config};

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
    let app = build_app(app_state);
    let serve_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!(error=?e, "server exited");
        }
    });

    let port = bind.port();
    let mut discoverers: Vec<Box<dyn Discoverer>> = Vec::new();
    for d in &cfg.network.discovery {
        match d.as_str() {
            "tailscale" => discoverers.push(Box::new(TailscaleDiscoverer {
                port,
                ..Default::default()
            })),
            "mdns" => match MdnsDiscoverer::new(card.name.clone()) {
                Ok(m) => {
                    if let Some(host_str) = card.url.host_str() {
                        if let Ok(ip) = host_str.parse() {
                            let _ = m.advertise(&card.name, ip, port);
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
            let mut all: Vec<DiscoveredPeer> = Vec::new();
            for d in &discoverers {
                let mut got = d.poll().await;
                tracing::debug!(target = d.name(), found = got.len(), "discovered");
                all.append(&mut got);
            }
            for peer in all {
                let _ = peers_store::upsert_peer(&store, &peer.card, chrono::Utc::now());
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
