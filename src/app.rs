use anyhow::{Context, Result};
use powersync::{PowerSyncDatabase, StreamSubscription, SyncOptions};

use crate::{
    config::DaemonConfig, connector::FieldMidConnector, database::open_database, watcher,
};

pub async fn run() -> Result<()> {
    let config = DaemonConfig::from_env()?;
    let context = open_database(&config.base.database_path)?;
    let db = context.db;
    db.async_tasks().spawn_with_tokio();

    let connector = FieldMidConnector::new(
        db.clone(),
        context.client,
        config.powersync_url.clone(),
        config.powersync_token.clone(),
        config.base.device_id.clone(),
        config.backend_write.clone(),
    );

    db.connect(SyncOptions::new(connector)).await;
    let _subscription = subscribe_stream_if_configured(&db, &config).await?;

    println!(
        "fieldmid_edge_started device_id={} database_path={} write_uploads={} stream_subscription={}",
        config.base.device_id,
        config.base.database_path.display(),
        if config.backend_write.is_some() {
            "enabled"
        } else {
            "disabled"
        },
        if config.sync_stream.is_some() {
            "enabled"
        } else {
            "disabled"
        },
    );

    tokio::select! {
        result = watcher::watch_sync_status(db.clone()) => result?,
        result = watcher::watch_critical_incidents(db.clone()) => result?,
        result = tokio::signal::ctrl_c() => {
            result.context("failed to listen for shutdown signal")?;
        }
    }

    db.disconnect().await;
    Ok(())
}

async fn subscribe_stream_if_configured(
    db: &PowerSyncDatabase,
    config: &DaemonConfig,
) -> Result<Option<StreamSubscription>> {
    let Some(stream) = &config.sync_stream else {
        return Ok(None);
    };

    let subscription = db
        .sync_stream(&stream.name, stream.params.as_ref())
        .subscribe()
        .await
        .with_context(|| format!("failed to subscribe to sync stream {}", stream.name))?;

    println!("sync_stream_subscribed name={}", stream.name);
    Ok(Some(subscription))
}
