use std::{env, path::PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

#[derive(Clone)]
pub struct BaseConfig {
    pub device_id: String,
    pub database_path: PathBuf,
}

pub struct DaemonConfig {
    pub base: BaseConfig,
    pub powersync_url: String,
    pub powersync_token: String,
    pub backend_write: Option<BackendWriteConfig>,
    pub sync_stream: Option<SyncStreamConfig>,
}

#[derive(Clone)]
pub struct BackendWriteConfig {
    pub url: String,
    pub token: Option<String>,
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
            device_id: optional_env("DEVICE_ID")
                .unwrap_or_else(|| "fieldmind-edge-001".to_string()),
            database_path: env::var_os("FIELDMIND_DB_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("fieldmind-edge.db")),
        }
    }
}

impl DaemonConfig {
    pub fn from_env() -> Result<Self> {
        let base = BaseConfig::from_env();
        let backend_write_url = optional_env("BACKEND_WRITE_URL");
        let backend_write_token = optional_env("BACKEND_WRITE_TOKEN");
        let sync_stream_name = optional_env("POWERSYNC_STREAM");
        let sync_stream_params = optional_env("POWERSYNC_STREAM_PARAMS")
            .map(|value| parse_json_object("POWERSYNC_STREAM_PARAMS", &value))
            .transpose()?;

        Ok(Self {
            base,
            powersync_url: required_env("POWERSYNC_URL")?,
            powersync_token: required_env("POWERSYNC_TOKEN")?,
            backend_write: backend_write_url.map(|url| BackendWriteConfig {
                url,
                token: backend_write_token,
            }),
            sync_stream: sync_stream_name.map(|name| SyncStreamConfig {
                name,
                params: sync_stream_params,
            }),
        })
    }
}

fn required_env(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("{name} is not set"))
}

fn optional_env(name: &str) -> Option<String> {
    env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_json_object(name: &str, value: &str) -> Result<Value> {
    let parsed: Value =
        serde_json::from_str(value).with_context(|| format!("{name} must be valid JSON"))?;

    if parsed.is_object() {
        Ok(parsed)
    } else {
        anyhow::bail!("{name} must be a JSON object")
    }
}
