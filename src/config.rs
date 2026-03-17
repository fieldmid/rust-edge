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
    #[allow(dead_code)]
    pub fn from_env() -> Result<Self> {
        let base = BaseConfig::from_env();
        Self::from_env_values(base, |name| env::var(name).ok())
    }

    /// Try env vars first; if POWERSYNC_TOKEN is missing, fall back to stored session.
    pub async fn from_env_or_session() -> Result<Self> {
        let base = BaseConfig::from_env();
        let env_token = optional_value("POWERSYNC_TOKEN", &|key| env::var(key).ok());

        if env_token.is_some() {
            // Static token set — use env-only config (legacy/dev mode)
            return Self::from_env_values(base, |name| env::var(name).ok());
        }

        // No static token — try stored session
        if session::has_session() {
            let sess = auth::ensure_session().await?;
            let powersync_url = optional_value("POWERSYNC_URL", &|key| env::var(key).ok())
                .context("POWERSYNC_URL is required")?;

            // Determine sync stream based on role
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

            return Ok(Self {
                base,
                powersync_url,
                powersync_token: sess.access_token.clone(),
                sync_stream,
                role: Some(sess.role.clone()),
                org_name: sess.org_name.clone(),
                email: Some(sess.email.clone()),
            });
        }

        // No session either — require env vars
        Self::from_env_values(base, |name| env::var(name).ok())
    }

    fn from_env_values<F>(base: BaseConfig, env_value: F) -> Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let sync_stream_name = optional_value("POWERSYNC_STREAM", &env_value);
        let sync_stream_params = optional_value("POWERSYNC_STREAM_PARAMS", &env_value)
            .map(|value| parse_json_object("POWERSYNC_STREAM_PARAMS", &value))
            .transpose()?;

        Ok(Self {
            base,
            powersync_url: required_value("POWERSYNC_URL", &env_value)?,
            powersync_token: required_value("POWERSYNC_TOKEN", &env_value)?,
            sync_stream: sync_stream_name.map(|name| SyncStreamConfig {
                name,
                params: sync_stream_params,
            }),
            role: None,
            org_name: None,
            email: None,
        })
    }
}

fn required_value<F>(name: &str, env_value: &F) -> Result<String>
where
    F: Fn(&str) -> Option<String>,
{
    optional_value(name, env_value).with_context(|| format!("{name} is not set"))
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

fn parse_json_object(name: &str, value: &str) -> Result<Value> {
    let parsed: Value =
        serde_json::from_str(value).with_context(|| format!("{name} must be valid JSON"))?;

    if parsed.is_object() {
        Ok(parsed)
    } else {
        anyhow::bail!("{name} must be a JSON object")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn test_base() -> BaseConfig {
        BaseConfig {
            device_id: "test-device".to_string(),
            database_path: PathBuf::from("test.db"),
        }
    }

    #[test]
    fn read_only_env_values_are_enough_to_load_daemon_config() {
        let values = HashMap::from([
            (
                "POWERSYNC_URL".to_string(),
                "https://example.powersync.com".to_string(),
            ),
            ("POWERSYNC_TOKEN".to_string(), "dev-token".to_string()),
        ]);

        let config =
            DaemonConfig::from_env_values(test_base(), |key| values.get(key).cloned()).unwrap();

        assert_eq!(config.powersync_url, "https://example.powersync.com");
        assert_eq!(config.powersync_token, "dev-token");
        assert!(config.sync_stream.is_none());
    }

    #[test]
    fn legacy_write_env_values_are_ignored() {
        let values = HashMap::from([
            (
                "POWERSYNC_URL".to_string(),
                "https://example.powersync.com".to_string(),
            ),
            ("POWERSYNC_TOKEN".to_string(), "dev-token".to_string()),
            (
                "BACKEND_WRITE_URL".to_string(),
                "https://example.com/write".to_string(),
            ),
            ("BACKEND_WRITE_TOKEN".to_string(), "legacy".to_string()),
        ]);

        let config =
            DaemonConfig::from_env_values(test_base(), |key| values.get(key).cloned()).unwrap();

        assert_eq!(config.powersync_token, "dev-token");
        assert!(config.sync_stream.is_none());
    }
}
