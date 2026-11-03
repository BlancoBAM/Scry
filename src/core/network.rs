use std::process::Command;

pub struct NetworkManager;

impl NetworkManager {
    pub fn get_mdns_hostname() -> String {
        if let Ok(output) = Command::new("hostname").output() {
            let host = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            if !host.is_empty() {
                return format!("{}.local", host);
            }
        }
        "lilith.local".to_string()
    }

    pub fn get_lan_ip() -> Option<String> {
        // Query kernel route to external IP to get active interface IP
        if let Ok(output) = Command::new("ip").args(["route", "get", "1.1.1.1"]).output() {
            let text = String::from_utf8_lossy(&output.stdout);
            // Output format: "1.1.1.1 via 192.168.1.1 dev wlp0s20f3 src 192.168.1.139 uid 1000"
            let words: Vec<&str> = text.split_whitespace().collect();
            if let Some(src_idx) = words.iter().position(|&w| w == "src") {
                if let Some(ip) = words.get(src_idx + 1) {
                    return Some(ip.to_string());
                }
            }
        }
        None
    }

    pub fn get_lan_url(_port: u16) -> String {
        let mdns = Self::get_mdns_hostname();
        format!("http://{}/", mdns)
    }

    pub fn get_local_browser_url(port: u16) -> String {
        format!("http://127.0.0.1:{}/", port)
    }

    pub fn get_web_url(tailscale_fqdn: &str) -> String {
        format!("https://{}/", tailscale_fqdn)
    }
}
