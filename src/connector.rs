use async_trait::async_trait;
use http_client::http_types::StatusCode;
use powersync::{BackendConnector, PowerSyncCredentials, error::PowerSyncError};

pub const READ_ONLY_UPLOAD_MESSAGE: &str = "local write queue is not supported in rust-edge deadline mode; keep the daemon read-only and send writes through your backend workflow";

#[derive(Clone)]
pub struct FieldMidConnector {
    endpoint: String,
    token: String,
}

impl FieldMidConnector {
    pub fn new(endpoint: String, token: String) -> Self {
        Self { endpoint, token }
    }
}

#[async_trait]
impl BackendConnector for FieldMidConnector {
    async fn fetch_credentials(&self) -> Result<PowerSyncCredentials, PowerSyncError> {
        Ok(PowerSyncCredentials {
            endpoint: self.endpoint.clone(),
            token: self.token.clone(),
        })
    }

    async fn upload_data(&self) -> Result<(), PowerSyncError> {
        Err(read_only_upload_error())
    }
}

fn read_only_upload_error() -> PowerSyncError {
    http_client::Error::from_str(StatusCode::Conflict, READ_ONLY_UPLOAD_MESSAGE).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn upload_data_returns_read_only_error() {
        let connector = FieldMidConnector::new("https://example.com".to_string(), "t".to_string());
        let err = connector
            .upload_data()
            .await
            .expect_err("upload_data should fail in read-only mode")
            .to_string();
        assert!(err.contains("read-only"));
    }
}
