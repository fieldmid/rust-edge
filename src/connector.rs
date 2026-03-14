use std::sync::Arc;

use async_trait::async_trait;
use http_client::{
    HttpClient, Request,
    http_types::{Body, Method, StatusCode, Url},
};
use powersync::{
    BackendConnector, CrudEntry, PowerSyncCredentials, PowerSyncDatabase, UpdateType,
    error::PowerSyncError,
};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::config::BackendWriteConfig;

#[derive(Clone)]
pub struct FieldMidConnector {
    db: PowerSyncDatabase,
    client: Arc<dyn HttpClient>,
    endpoint: String,
    token: String,
    device_id: String,
    backend_write: Option<BackendWriteConfig>,
}

impl FieldMidConnector {
    pub fn new(
        db: PowerSyncDatabase,
        client: Arc<dyn HttpClient>,
        endpoint: String,
        token: String,
        device_id: String,
        backend_write: Option<BackendWriteConfig>,
    ) -> Self {
        Self {
            db,
            client,
            endpoint,
            token,
            device_id,
            backend_write,
        }
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
        while let Some(transaction) = self.db.next_crud_transaction().await? {
            self.upload_transaction(transaction.id, &transaction.crud)
                .await?;
            transaction.complete().await?;
        }

        Ok(())
    }
}

impl FieldMidConnector {
    async fn upload_transaction(
        &self,
        transaction_id: Option<i64>,
        changes: &[CrudEntry],
    ) -> Result<(), PowerSyncError> {
        let Some(config) = &self.backend_write else {
            return Err(sync_error(
                StatusCode::Conflict,
                "pending local writes detected but BACKEND_WRITE_URL is not configured",
            ));
        };

        let payload = WriteBatch {
            device_id: &self.device_id,
            transaction_id,
            changes: changes.iter().map(WriteChange::from).collect(),
        };

        let url = Url::parse(&config.url)
            .map_err(|error| sync_error(StatusCode::BadRequest, error.to_string()))?;
        let mut request = Request::new(Method::Post, url);
        request.set_body(Body::from_json(&payload)?);
        request.insert_header("content-type", "application/json");

        if let Some(token) = &config.token {
            request.insert_header("authorization", format!("Bearer {token}"));
        }

        let mut response = self.client.send(request).await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.body_string().await.unwrap_or_default();
            return Err(sync_error(status, format_response_error(status, &body)));
        }

        println!(
            "uploaded_local_write_batch tx_id={} changes={}",
            transaction_id.unwrap_or_default(),
            changes.len()
        );

        Ok(())
    }
}

#[derive(Serialize)]
struct WriteBatch<'a> {
    device_id: &'a str,
    transaction_id: Option<i64>,
    changes: Vec<WriteChange<'a>>,
}

#[derive(Serialize)]
struct WriteChange<'a> {
    client_id: i64,
    transaction_id: i64,
    update_type: &'a UpdateType,
    table: &'a str,
    id: &'a str,
    metadata: Option<&'a str>,
    data: Option<&'a Map<String, Value>>,
    previous_values: Option<&'a Map<String, Value>>,
}

impl<'a> From<&'a CrudEntry> for WriteChange<'a> {
    fn from(value: &'a CrudEntry) -> Self {
        Self {
            client_id: value.client_id,
            transaction_id: value.transaction_id,
            update_type: &value.update_type,
            table: &value.table,
            id: &value.id,
            metadata: value.metadata.as_deref(),
            data: value.data.as_ref(),
            previous_values: value.previous_values.as_ref(),
        }
    }
}

fn format_response_error(status: StatusCode, body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        format!("write endpoint returned {status}")
    } else {
        format!("write endpoint returned {status}: {body}")
    }
}

fn sync_error(status: StatusCode, message: impl Into<String>) -> PowerSyncError {
    http_client::Error::from_str(status, message.into()).into()
}
