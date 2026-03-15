use crate::errors::Result;
use std::path::Path;

pub fn write_pid(config_dir: &Path) -> Result<()> {
    let pid_path = config_dir.join("giantd.pid");
    std::fs::write(&pid_path, std::process::id().to_string())?;
    Ok(())
}

pub fn read_pid(config_dir: &Path) -> Result<Option<u32>> {
    let pid_path = config_dir.join("giantd.pid");
    if !pid_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&pid_path)?;
    match content.trim().parse::<u32>() {
        Ok(pid) => Ok(Some(pid)),
        Err(_) => Ok(None),
    }
}

pub fn is_running(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn cleanup_pid(config_dir: &Path) -> Result<()> {
    let pid_path = config_dir.join("giantd.pid");
    if pid_path.exists() {
        std::fs::remove_file(&pid_path)?;
    }
    Ok(())
}
