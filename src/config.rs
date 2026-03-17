use std::{env, path::PathBuf};

use anyhow::{Context, Result};
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
        dotenvy::dotenv().ok();
        Self {
            device_id: optional_env("DEVICE_ID").unwrap_or_else(|| "fieldmid-edge-001".to_string()),
            database_path: env::var_os("FIELDMID_DB_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("fieldmid-edge.db")),
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
        let powersync_url = optional_value("POWERSYNC_URL", &|key| env::var(key).ok())
            .context("POWERSYNC_URL is required")?;

        let sync_stream = match sess.role.as_str() {
            "admin" => Some(SyncStreamConfig {
                name: "admin_overview".to_string(),
                params: None,
            }),
            "supervisor" => {
                if let Some(site_id) = &sess.site_id {
                    Some(SyncStreamConfig {
                        name: "edge_critical_feed".to_string(),
                        params: Some(serde_json::json!({ "site_id": site_id })),
                    })
                } else {
                    Some(SyncStreamConfig {
                        name: "supervisor_site".to_string(),
                        params: None,
                    })
                }
            }
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

fn optional_value<F>(name: &str, env_value: &F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    env_value(name).and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn optional_env(name: &str) -> Option<String> {
    optional_value(name, &|key| env::var(key).ok())
}
