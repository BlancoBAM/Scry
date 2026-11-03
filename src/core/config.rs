use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub serve_to_web: bool,
    pub require_password: bool,
    pub session_name: String,
    pub port: u16,
    pub auto_start_on_boot: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            serve_to_web: false,
            require_password: true,
            session_name: "scry".to_string(),
            port: 8082,
            auto_start_on_boot: true,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/s8n".to_string());
        PathBuf::from(home).join(".config/scry/config.json")
    }

    pub fn legacy_config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/s8n".to_string());
        PathBuf::from(home).join(".config/lilith/web-terminal.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    return config;
                }
            }
        }

        // Migrate or fallback from legacy path if present
        let leg = Self::legacy_config_path();
        if leg.exists() {
            if let Ok(content) = fs::read_to_string(&leg) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    let _ = config.save(); // Save to new location
                    return config;
                }
            }
        }

        Self::default()
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)
    }

    pub fn reset() -> Result<(), std::io::Error> {
        let path = Self::config_path();
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        let leg = Self::legacy_config_path();
        if leg.exists() {
            let _ = fs::remove_file(leg);
        }
        Ok(())
    }
}
