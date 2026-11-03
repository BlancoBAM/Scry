use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct SystemdManager;

impl SystemdManager {
    pub const SERVICE_NAME: &'static str = "scry.service";
    pub const LEGACY_SERVICE_NAME: &'static str = "lilith-web-terminal.service";

    pub fn service_file_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/s8n".to_string());
        PathBuf::from(home).join(format!(".config/systemd/user/{}", Self::SERVICE_NAME))
    }

    pub fn legacy_service_file_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/s8n".to_string());
        PathBuf::from(home).join(format!(".config/systemd/user/{}", Self::LEGACY_SERVICE_NAME))
    }

    pub fn write_service_unit(session_name: &str, port: u16) -> Result<(), std::io::Error> {
        let path = Self::service_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Clean up legacy unit file if it existed
        let old_path = Self::legacy_service_file_path();
        if old_path.exists() {
            let _ = Command::new("systemctl")
                .args(["--user", "stop", Self::LEGACY_SERVICE_NAME])
                .output();
            let _ = Command::new("systemctl")
                .args(["--user", "disable", Self::LEGACY_SERVICE_NAME])
                .output();
            let _ = fs::remove_file(old_path);
        }

        let zellij = super::zellij::ZellijManager::zellij_binary();
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/s8n".to_string());

        let unit_content = format!(
            r#"[Unit]
Description=Scry Persistent Web Terminal Service
After=network.target

[Service]
Type=exec
Environment="PATH={home}/.cargo/bin:/usr/local/bin:/usr/bin:/bin"
ExecStartPre=-/bin/sh -c 'fuser -k {port}/tcp 2>/dev/null || true'
ExecStartPre=-/bin/sh -c '{zellij_str} list-sessions 2>/dev/null | grep -E "^{session_name} " | grep -vq EXITED || {zellij_str} attach -b {session_name} || true'
ExecStart={zellij_str} web --port {port}
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
"#,
            home = home,
            session_name = session_name,
            port = port,
            zellij_str = zellij.display()
        );

        fs::write(path, unit_content)?;

        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();

        Ok(())
    }

    pub fn start() -> Result<(), std::io::Error> {
        let _ = Command::new("systemctl")
            .args(["--user", "start", Self::SERVICE_NAME])
            .output()?;
        Ok(())
    }

    pub fn stop() -> Result<(), std::io::Error> {
        let _ = Command::new("systemctl")
            .args(["--user", "stop", Self::SERVICE_NAME])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "stop", Self::LEGACY_SERVICE_NAME])
            .output();
        Ok(())
    }

    pub fn restart() -> Result<(), std::io::Error> {
        let _ = Command::new("systemctl")
            .args(["--user", "restart", Self::SERVICE_NAME])
            .output()?;
        Ok(())
    }

    pub fn enable() -> Result<(), std::io::Error> {
        let _ = Command::new("systemctl")
            .args(["--user", "enable", Self::SERVICE_NAME])
            .output()?;
        Ok(())
    }

    pub fn disable() -> Result<(), std::io::Error> {
        let _ = Command::new("systemctl")
            .args(["--user", "disable", Self::SERVICE_NAME])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "disable", Self::LEGACY_SERVICE_NAME])
            .output();
        Ok(())
    }

    pub fn is_active() -> bool {
        if let Ok(output) = Command::new("systemctl")
            .args(["--user", "is-active", Self::SERVICE_NAME])
            .output()
        {
            let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if status == "active" {
                return true;
            }
        }
        if let Ok(output) = Command::new("systemctl")
            .args(["--user", "is-active", Self::LEGACY_SERVICE_NAME])
            .output()
        {
            let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if status == "active" {
                return true;
            }
        }
        false
    }

    pub fn ensure_linger() -> Result<(), std::io::Error> {
        let user = std::env::var("USER").unwrap_or_else(|_| "s8n".to_string());
        let _ = Command::new("loginctl")
            .args(["enable-linger", &user])
            .output();
        Ok(())
    }

    pub fn remove_service_unit() -> Result<(), std::io::Error> {
        let _ = Self::stop();
        let _ = Self::disable();
        let path = Self::service_file_path();
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        let leg = Self::legacy_service_file_path();
        if leg.exists() {
            let _ = fs::remove_file(leg);
        }
        let _ = Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
        Ok(())
    }
}
