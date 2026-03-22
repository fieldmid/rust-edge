use std::{fs, path::PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::{auth, session};

#[derive(Clone)]
pub struct BaseConfig {
    pub device_id: String,
    pub database_path: PathBuf,
}

pub struct DaemonConfig {
    pub base: BaseConfig,
    pub powersync_url: String,
    pub powersync_token: String,
    pub sync_stream: Option<SyncStreamConfig>,
    pub role: Option<String>,
    pub org_name: Option<String>,
    pub email: Option<String>,
}

#[derive(Clone)]
pub struct SyncStreamConfig {
    pub name: String,
    pub params: Option<Value>,
}

impl BaseConfig {
    pub fn from_env() -> Self {
        Self {
            device_id: auto_device_id(),
            database_path: PathBuf::from("fieldmid-edge.db"),
        }
    }
}

impl DaemonConfig {
    pub async fn from_env_or_session() -> Result<Self> {
        let base = BaseConfig::from_env();

        if !session::has_session() {
            anyhow::bail!("Not logged in. Run `cargo run -- login` first.");
        }

        let sess = auth::ensure_session().await?;
        let powersync_url = "https://69b1eb15d42a433951017c06.powersync.journeyapps.com".to_string();

        let sync_stream = match sess.role.as_str() {
            "admin" => Some(SyncStreamConfig {
                name: "admin_overview".to_string(),
                params: None,
            }),
            "supervisor" => Some(SyncStreamConfig {
                name: "supervisor_site".to_string(),
                params: None,
            }),
            _ => None,
        };

        Ok(Self {
            base,
            powersync_url,
            powersync_token: sess.access_token.clone(),
            sync_stream,
            role: Some(sess.role.clone()),
            org_name: sess.org_name.clone(),
            email: Some(sess.email.clone()),
        })
    }
}

fn auto_device_id() -> String {
    let host = hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "fieldmid-edge".to_string());

    let model = read_trimmed("/sys/class/dmi/id/product_name")
        .or_else(|| read_trimmed("/sys/devices/virtual/dmi/id/product_name"))
        .unwrap_or_default();

    if model.is_empty() {
        return sanitize_device_fragment(&host);
    }

    let host_part = sanitize_device_fragment(&host);
    let model_part = sanitize_device_fragment(&model);
    format!("{host_part}-{model_part}")
}

fn sanitize_device_fragment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut previous_dash = false;

    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            out.push('-');
            previous_dash = true;
        }
    }

    out.trim_matches('-').to_string()
}

fn read_trimmed(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
