use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub user_id: String,
    pub email: String,
    pub role: String,
    pub full_name: Option<String>,
    pub org_id: Option<String>,
    pub org_name: Option<String>,
    pub site_id: Option<String>,
    pub site_name: Option<String>,
    pub supabase_url: String,
    pub supabase_anon_key: String,
}

fn session_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not resolve home directory")?;
    let dir = home.join(".fieldmid");
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create {}", dir.display()))?;
    }
    Ok(dir)
}

fn session_path() -> Result<PathBuf> {
    Ok(session_dir()?.join("session.json"))
}

pub fn save_session(session: &Session) -> Result<()> {
    let path = session_path()?;
    let json = serde_json::to_string_pretty(session)?;
    fs::write(&path, json)
        .with_context(|| format!("failed to write session to {}", path.display()))?;
    Ok(())
}

pub fn load_session() -> Result<Session> {
    let path = session_path()?;
    let json = fs::read_to_string(&path)
        .with_context(|| format!("no session file at {}; run `fieldmid login` first", path.display()))?;
    let session: Session =
        serde_json::from_str(&json).context("session file is corrupted; run `fieldmid login`")?;
    Ok(session)
}

pub fn clear_session() -> Result<()> {
    let path = session_path()?;
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

pub fn has_session() -> bool {
    session_path()
        .map(|p| p.exists())
        .unwrap_or(false)
}
