use anyhow::Result;
use std::path::PathBuf;

const TEMPLATE: &str = include_str!("../../assets/constellation.service.tmpl");

pub async fn run() -> Result<()> {
    let exe = std::env::current_exe()?;
    let unit = TEMPLATE
        .replace("{{EXE}}", &exe.display().to_string())
        .replace("{{USER}}", &whoami_user());
    let target = systemd_user_unit_path()?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&target, unit)?;
    println!("installed: {}", target.display());
    println!("enable with: systemctl --user enable --now constellation");
    Ok(())
}

fn whoami_user() -> String {
    std::env::var("USER").unwrap_or_else(|_| "user".into())
}

fn systemd_user_unit_path() -> Result<PathBuf> {
    let base = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
    Ok(base.join("systemd/user/constellation.service"))
}
