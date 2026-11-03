use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct TailscaleInfo {
    pub is_installed: bool,
    pub is_logged_in: bool,
    pub fqdn: Option<String>,
    pub funnel_active: bool,
}

#[derive(Deserialize)]
struct TailscaleStatusJson {
    #[serde(rename = "Self")]
    self_node: Option<TailscaleSelfNode>,
    #[serde(rename = "BackendState")]
    backend_state: Option<String>,
}

#[derive(Deserialize)]
struct TailscaleSelfNode {
    #[serde(rename = "DNSName")]
    dns_name: Option<String>,
}

pub struct TailscaleManager;

impl TailscaleManager {
    pub fn tailscale_binary() -> PathBuf {
        if PathBuf::from("/usr/bin/tailscale").exists() {
            return PathBuf::from("/usr/bin/tailscale");
        }
        PathBuf::from("tailscale")
    }

    pub fn is_installed() -> bool {
        Command::new("which")
            .arg("tailscale")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn get_info() -> TailscaleInfo {
        if !Self::is_installed() {
            return TailscaleInfo {
                is_installed: false,
                is_logged_in: false,
                fqdn: None,
                funnel_active: false,
            };
        }

        let tailscale = Self::tailscale_binary();
        let mut info = TailscaleInfo {
            is_installed: true,
            is_logged_in: false,
            fqdn: None,
            funnel_active: false,
        };

        if let Ok(output) = Command::new(&tailscale)
            .arg("status")
            .arg("--json")
            .output()
        {
            if output.status.success() {
                if let Ok(json) = serde_json::from_slice::<TailscaleStatusJson>(&output.stdout) {
                    if json.backend_state.as_deref() == Some("Running") {
                        info.is_logged_in = true;
                        if let Some(self_node) = json.self_node {
                            if let Some(mut dns) = self_node.dns_name {
                                if dns.ends_with('.') {
                                    dns.pop();
                                }
                                info.fqdn = Some(dns);
                            }
                        }
                    }
                }
            }
        }

        info.funnel_active = Self::is_funnel_active();
        info
    }

    pub fn is_funnel_active() -> bool {
        let tailscale = Self::tailscale_binary();
        if let Ok(output) = Command::new(tailscale).arg("funnel").arg("status").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return stdout.contains("Funnel on") && stdout.contains("8082");
        }
        false
    }

    pub fn set_funnel(enable: bool, port: u16) -> Result<(), std::io::Error> {
        let tailscale = Self::tailscale_binary();
        if enable {
            Command::new(tailscale)
                .arg("funnel")
                .arg("--bg")
                .arg(port.to_string())
                .output()?;
        } else {
            Command::new(tailscale)
                .arg("funnel")
                .arg("--https=443")
                .arg("off")
                .output()?;
        }
        Ok(())
    }

    pub fn launch_login_in_browser() -> Result<(), std::io::Error> {
        let tailscale = Self::tailscale_binary();
        let user = std::env::var("USER").unwrap_or_else(|_| "s8n".to_string());
        // Run tailscale up with current user as operator in background
        let _ = Command::new("sudo")
            .arg(tailscale)
            .arg("up")
            .arg(format!("--operator={}", user))
            .spawn();
        Ok(())
    }
}
