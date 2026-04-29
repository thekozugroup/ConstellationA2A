use anyhow::{anyhow, Result};

pub async fn resolve_advertised_host(setting: &str) -> Result<String> {
    if setting != "auto" {
        return Ok(setting.to_string());
    }
    if let Some(ip) = tailscale_ip().await {
        return Ok(ip);
    }
    if let Some(ip) = first_lan_ipv4().await? {
        return Ok(ip);
    }
    Err(anyhow!(
        "could not resolve a non-loopback IP for advertisement"
    ))
}

async fn tailscale_ip() -> Option<String> {
    let out = tokio::process::Command::new("tailscale")
        .arg("ip")
        .arg("-4")
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.lines()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
}

async fn first_lan_ipv4() -> Result<Option<String>> {
    use std::net::{IpAddr, Ipv4Addr};
    let raw =
        tokio::task::spawn_blocking(|| std::process::Command::new("hostname").arg("-I").output())
            .await??;
    if !raw.status.success() {
        return Ok(None);
    }
    for tok in String::from_utf8_lossy(&raw.stdout).split_whitespace() {
        if let Ok(IpAddr::V4(v4)) = tok.parse::<IpAddr>() {
            if v4 != Ipv4Addr::LOCALHOST && !v4.is_loopback() && !v4.is_unspecified() {
                return Ok(Some(v4.to_string()));
            }
        }
    }
    Ok(None)
}
