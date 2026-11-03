#![allow(dead_code)]

pub mod config;
pub mod network;
pub mod qr;
pub mod systemd;
pub mod tailscale;
pub mod zellij;

pub use config::AppConfig;
pub use network::NetworkManager;
pub use qr::QrGenerator;
pub use systemd::SystemdManager;
pub use tailscale::{TailscaleInfo, TailscaleManager};
pub use zellij::{ZellijManager, ZellijSessionEntry};
