use std::net::IpAddr;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::time::sleep;

use crate::{probe::probe_card, DiscoveredPeer, Discoverer};

pub const SERVICE_TYPE: &str = "_a2a._tcp.local.";

pub struct MdnsDiscoverer {
    daemon: ServiceDaemon,
    pub local_name: String,
    pub poll_window: Duration,
}

impl MdnsDiscoverer {
    pub fn new(local_name: impl Into<String>) -> anyhow::Result<Self> {
        let daemon = ServiceDaemon::new()?;
        Ok(Self {
            daemon,
            local_name: local_name.into(),
            poll_window: Duration::from_millis(800),
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
        let mut out = Vec::new();
        let deadline = std::time::Instant::now() + self.poll_window;
        loop {
            if std::time::Instant::now() >= deadline {
                break;
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            tokio::select! {
                _ = sleep(remaining) => break,
                evt = tokio::task::spawn_blocking({
                    let r = receiver.clone();
                    move || r.recv_timeout(Duration::from_millis(200))
                }) => {
                    if let Ok(Ok(ServiceEvent::ServiceResolved(info))) = evt {
                        let host_name = info.get_fullname()
                            .trim_end_matches(SERVICE_TYPE)
                            .trim_end_matches('.')
                            .to_string();
                        if host_name == self.local_name { continue; }
                        for ip in info.get_addresses() {
                            let base = format!("http://{}:{}", ip, info.get_port());
                            if let Ok(card) = probe_card(&base).await {
                                out.push(DiscoveredPeer {
                                    host: host_name.clone(),
                                    ip: *ip,
                                    port: info.get_port(),
                                    card,
                                });
                                break;
                            }
                        }
                    }
                }
            }
        }
        out
    }
}
