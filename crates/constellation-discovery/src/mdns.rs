use std::net::IpAddr;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::{
    probe::{default_client, probe_card},
    DiscoveredPeer, Discoverer,
};

pub const SERVICE_TYPE: &str = "_a2a._tcp.local.";

pub struct MdnsDiscoverer {
    daemon: ServiceDaemon,
    pub(crate) local_name: String,
    pub poll_window: Duration,
    client: reqwest::Client,
}

impl MdnsDiscoverer {
    pub fn new(local_name: impl Into<String>) -> anyhow::Result<Self> {
        let daemon = ServiceDaemon::new()?;
        Ok(Self {
            daemon,
            local_name: local_name.into(),
            poll_window: Duration::from_millis(800),
            client: default_client(Duration::from_secs(3)),
        })
    }

    pub fn advertise(&self, name: &str, ip: IpAddr, port: u16) -> anyhow::Result<()> {
        let info = ServiceInfo::new(
            SERVICE_TYPE,
            name,
            &format!("{name}.local."),
            ip,
            port,
            &[("name", name)][..],
        )?;
        self.daemon.register(info)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Discoverer for MdnsDiscoverer {
    fn name(&self) -> &'static str {
        "mdns"
    }

    async fn poll(&self) -> Vec<DiscoveredPeer> {
        let receiver = match self.daemon.browse(SERVICE_TYPE) {
            Ok(rx) => rx,
            Err(e) => {
                tracing::warn!(error=%e, "mdns browse failed");
                return vec![];
            }
        };
        let window = self.poll_window;
        let local_name = self.local_name.clone();
        let infos = tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            let deadline = std::time::Instant::now() + window;
            while std::time::Instant::now() < deadline {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                match receiver.recv_timeout(remaining.min(Duration::from_millis(200))) {
                    Ok(ServiceEvent::ServiceResolved(info)) => {
                        let host_name = info
                            .get_fullname()
                            .trim_end_matches(SERVICE_TYPE)
                            .trim_end_matches('.')
                            .to_string();
                        if host_name == local_name {
                            continue;
                        }
                        out.push((
                            host_name,
                            info.get_addresses().iter().copied().collect::<Vec<_>>(),
                            info.get_port(),
                        ));
                    }
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
            out
        })
        .await
        .unwrap_or_default();

        let mut out = Vec::new();
        for (host_name, ips, port) in infos {
            for ip in ips {
                let base = format!("http://{}:{}", ip, port);
                if let Ok(card) = probe_card(&self.client, &base).await {
                    out.push(DiscoveredPeer {
                        host: host_name.clone(),
                        ip,
                        port,
                        card,
                    });
                    break;
                }
            }
        }
        out
    }
}
