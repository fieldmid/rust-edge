use std::path::Path;

use anyhow::{Context, Result};
use http_client::isahc::IsahcClient;
use powersync::{ConnectionPool, PowerSyncDatabase, env::PowerSyncEnvironment};

use crate::schema::app_schema;

pub struct DatabaseContext {
    pub db: PowerSyncDatabase,
}

pub fn open_database(path: &Path) -> Result<DatabaseContext> {
    PowerSyncEnvironment::powersync_auto_extension()
        .context("failed to load the PowerSync SQLite extension")?;

    let pool = ConnectionPool::open(path)
        .with_context(|| format!("failed to open SQLite database at {}", path.display()))?;

    let client = std::sync::Arc::new(IsahcClient::new());
    let env = PowerSyncEnvironment::custom(
        client.clone(),
        pool,
        Box::new(PowerSyncEnvironment::tokio_timer()),
    );

    Ok(DatabaseContext {
        db: PowerSyncDatabase::new(env, app_schema()),
    })
}
