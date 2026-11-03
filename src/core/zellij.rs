use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZellijSessionEntry {
    pub name: String,
    pub is_exited: bool,
    pub details: String,
}

pub struct ZellijManager;

impl ZellijManager {
    pub fn zellij_binary() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/s8n".to_string());
        let cargo_bin = PathBuf::from(&home).join(".cargo/bin/zellij");
        if cargo_bin.exists() {
            return cargo_bin;
        }
        if PathBuf::from("/usr/local/bin/zellij").exists() {
            return PathBuf::from("/usr/local/bin/zellij");
        }
        PathBuf::from("/usr/bin/zellij")
    }

    pub fn tokens_db_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/s8n".to_string());
        PathBuf::from(home).join(".local/share/zellij/tokens.db")
    }

    pub fn init_tokens_db_if_needed() -> Result<(), rusqlite::Error> {
        let path = Self::tokens_db_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                token_hash TEXT UNIQUE NOT NULL,
                name TEXT UNIQUE NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                read_only BOOLEAN NOT NULL DEFAULT 0
            );",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS session_tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_token_hash TEXT UNIQUE NOT NULL,
                auth_token_hash TEXT NOT NULL,
                remember_me BOOLEAN NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                expires_at DATETIME NOT NULL,
                FOREIGN KEY (auth_token_hash) REFERENCES tokens(token_hash)
            );",
            [],
        )?;
        Ok(())
    }

    pub fn set_password(password: &str) -> Result<(), Box<dyn std::error::Error>> {
        Self::init_tokens_db_if_needed()?;
        let mut hasher = Sha256::new();
        hasher.update(password.trim().as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let conn = Connection::open(Self::tokens_db_path())?;
        // Remove previous password entries to avoid unique constraint violations
        let _ = conn.execute("DELETE FROM tokens WHERE name IN ('scry_password', 'lilith_password', 'user_password')", []);
        conn.execute(
            "INSERT INTO tokens (token_hash, name, read_only) VALUES (?1, 'scry_password', 0)",
            params![hash],
        )?;
        Ok(())
    }

    pub fn is_password_set() -> bool {
        if !Self::tokens_db_path().exists() {
            return false;
        }
        if let Ok(conn) = Connection::open(Self::tokens_db_path()) {
            if let Ok(mut stmt) = conn.prepare("SELECT COUNT(*) FROM tokens WHERE name IN ('scry_password', 'lilith_password', 'user_password')") {
                if let Ok(count) = stmt.query_row([], |row| row.get::<_, i64>(0)) {
                    return count > 0;
                }
            }
        }
        false
    }

    pub fn create_observer_token(session_name: &str) -> Result<String, Box<dyn std::error::Error>> {
        Self::init_tokens_db_if_needed()?;
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let raw_token = format!("view-{:x}", ts & 0xffffff);

        let mut hasher = Sha256::new();
        hasher.update(raw_token.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        let token_name = format!("observer_{}", session_name);
        let conn = Connection::open(Self::tokens_db_path())?;
        let _ = conn.execute("DELETE FROM tokens WHERE name = ?1", params![token_name]);
        conn.execute(
            "INSERT INTO tokens (token_hash, name, read_only) VALUES (?1, ?2, 1)",
            params![hash, token_name],
        )?;
        Ok(raw_token)
    }

    pub fn reset_credentials() -> Result<(), rusqlite::Error> {
        if Self::tokens_db_path().exists() {
            let conn = Connection::open(Self::tokens_db_path())?;
            let _ = conn.execute("DELETE FROM tokens WHERE name LIKE 'observer_%' OR name IN ('scry_password', 'lilith_password', 'user_password')", []);
        }
        Ok(())
    }

    pub fn list_all_sessions() -> Vec<ZellijSessionEntry> {
        let zellij = Self::zellij_binary();
        let mut list = Vec::new();
        if let Ok(output) = Command::new(zellij).arg("list-sessions").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let is_exited = trimmed.contains("EXITED");
                let name = trimmed.split_whitespace().next().unwrap_or("").to_string();
                if !name.is_empty() {
                    list.push(ZellijSessionEntry {
                        name,
                        is_exited,
                        details: trimmed.to_string(),
                    });
                }
            }
        }
        list
    }

    pub fn delete_session(name: &str) -> Result<(), std::io::Error> {
        let zellij = Self::zellij_binary();
        let _ = Command::new(zellij)
            .args(["delete-session", name])
            .output()?;
        Ok(())
    }

    pub fn resurrect_session(name: &str) -> Result<(), std::io::Error> {
        let zellij = Self::zellij_binary();
        let _ = Command::new(zellij)
            .args(["attach", "-b", name])
            .output()?;
        Ok(())
    }

    pub fn is_session_running(session_name: &str) -> bool {
        let zellij = Self::zellij_binary();
        if let Ok(output) = Command::new(zellij).arg("list-sessions").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.starts_with(session_name) && !line.contains("EXITED") {
                    return true;
                }
            }
        }
        false
    }

    pub fn ensure_session_running(session_name: &str) -> Result<(), std::io::Error> {
        if Self::is_session_running(session_name) {
            return Ok(());
        }
        let zellij = Self::zellij_binary();
        let _ = Command::new(zellij)
            .arg("attach")
            .arg("-b")
            .arg(session_name)
            .output()?;
        Ok(())
    }

    pub fn is_web_server_online(port: u16) -> bool {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok()
    }
}
