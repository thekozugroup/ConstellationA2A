//! mDNS-based peer discoverer.

use std::net::IpAddr;
use std::sync::Mutex;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::{
    probe::{default_client, probe_card},
    DiscoveredPeer, Discoverer,
};

/// mDNS service type used for A2A peer advertisement.
pub const SERVICE_TYPE: &str = "_a2a._tcp.local.";

const POLL_WINDOW: Duration = Duration::from_millis(800);
const RECV_CHUNK: Duration = Duration::from_millis(200);

/// Discovers A2A peers on the local network via mDNS service discovery.
pub struct MdnsDiscoverer {
    daemon: ServiceDaemon,
    local_name: String,
    poll_window: Duration,
    client: reqwest::Client,
    /// Cached browse receiver; initialised on first poll.
    receiver: Mutex<Option<mdns_sd::Receiver<ServiceEvent>>>,
}

impl MdnsDiscoverer {
    /// Create a new discoverer that skips the given `local_name` in results.
    pub fn new(local_name: impl Into<String>) -> anyhow::Result<Self> {
        let daemon = ServiceDaemon::new()?;
        Ok(Self {
            daemon,
            local_name: local_name.into(),
            poll_window: POLL_WINDOW,
            client: default_client(Duration::from_secs(3)),
            receiver: Mutex::new(None),
        })
    }

    /// Return the local agent name that is excluded from discovery results.
    pub fn local_name(&self) -> &str {
        &self.local_name
    }

    /// Register an mDNS advertisement for `name` at `ip:port`.
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

    /// Obtain (or reuse the cached) browse receiver for [`SERVICE_TYPE`].
    fn ensure_receiver(&self) -> Option<mdns_sd::Receiver<ServiceEvent>> {
        let mut guard = self.receiver.lock().ok()?;
        if let Some(rx) = guard.as_ref() {
            return Some(rx.clone());
        }
        match self.daemon.browse(SERVICE_TYPE) {
            Ok(rx) => {
                *guard = Some(rx.clone());
                Some(rx)
            }
            Err(e) => {
                tracing::warn!(error=%e, "mdns browse failed");
                None
            }
        }
    }
}

#[async_trait::async_trait]
impl Discoverer for MdnsDiscoverer {
    fn name(&self) -> &'static str {
        "mdns"
    }

    async fn poll(&self) -> Vec<DiscoveredPeer> {
        let receiver = match self.ensure_receiver() {
            Some(rx) => rx,
            None => return vec![],
        };
        let window = self.poll_window;
        let local_name = self.local_name.clone();
        let infos = tokio::task::spawn_blocking(move || {
            let mut out = Vec::new();
            let deadline = std::time::Instant::now() + window;
            while std::time::Instant::now() < deadline {
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                match receiver.recv_timeout(remaining.min(RECV_CHUNK)) {
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
