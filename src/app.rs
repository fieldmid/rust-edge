use std::{io::IsTerminal, time::Duration};

use anyhow::{Context, Result};
use powersync::{PowerSyncDatabase, StreamSubscription, SyncOptions};

use crate::{
    banner,
    config::{BaseConfig, DaemonConfig},
    connector::FieldMidConnector,
    database::open_database,
    tui, watcher,
};

pub async fn run() -> Result<()> {
    let config = DaemonConfig::from_env()?;
    let context = open_database(&config.base.database_path)?;
    let db = context.db;
    db.async_tasks().spawn_with_tokio();

    watcher::assert_no_local_writes(&db).await?;

    let connector =
        FieldMidConnector::new(config.powersync_url.clone(), config.powersync_token.clone());

    db.connect(SyncOptions::new(connector)).await;
    let _subscription = subscribe_stream_if_configured(&db, &config).await?;

    if should_use_tui() {
        tui::run(
            db.clone(),
            tui::TuiConfig {
                device_id: config.base.device_id.clone(),
                database_path: config.base.database_path.display().to_string(),
                stream_subscription_enabled: config.sync_stream.is_some(),
            },
        )
        .await?;
    } else {
        banner::print_banner();
        println!(
            "fieldmid_edge_started device_id={} database_path={} mode=read_only stream_subscription={}",
            config.base.device_id,
            config.base.database_path.display(),
            if config.sync_stream.is_some() {
                "enabled"
            } else {
                "disabled"
            },
        );

        tokio::select! {
            result = watcher::watch_sync_status(db.clone()) => result?,
            result = watcher::watch_critical_incidents(db.clone()) => result?,
            result = watcher::watch_for_local_writes(db.clone()) => result?,
            result = tokio::signal::ctrl_c() => {
                result.context("failed to listen for shutdown signal")?;
            }
        }
    }

    db.disconnect().await;
    Ok(())
}

pub async fn check_connectivity() -> Result<()> {
    let config = DaemonConfig::from_env()?;
    let context = open_database(&config.base.database_path)?;
    let db = context.db;
    db.async_tasks().spawn_with_tokio();
    watcher::assert_no_local_writes(&db).await?;

    let connector =
        FieldMidConnector::new(config.powersync_url.clone(), config.powersync_token.clone());
    db.connect(SyncOptions::new(connector)).await;
    let _subscription = subscribe_stream_if_configured(&db, &config).await?;

    let status = watcher::wait_for_first_sync_status(db.clone(), Duration::from_secs(10)).await?;
    let queue_depth = watcher::read_local_write_queue_depth(&db).await?;
    let incidents = watcher::fetch_critical_incidents(&db, 5).await?;

    println!("connectivity_status={status}");
    println!("local_write_queue_depth={queue_depth}");
    println!("critical_incidents_snapshot={}", incidents.len());
    for incident in incidents {
        let created_at = incident.created_at.as_deref().unwrap_or("-");
        println!(
            "{} | {} | {} | {}",
            created_at, incident.status, incident.id, incident.title
        );
    }

    db.disconnect().await;
    Ok(())
}

pub async fn show_latest_incidents() -> Result<()> {
    let base = BaseConfig::from_env();
    let context = open_database(&base.database_path)?;
    let incidents = watcher::fetch_critical_incidents(&context.db, 20).await?;

    println!(
        "critical_incidents_snapshot={} database_path={}",
        incidents.len(),
        base.database_path.display()
    );
    for incident in incidents {
        let created_at = incident.created_at.as_deref().unwrap_or("-");
        println!(
            "{} | {} | {} | {}",
            created_at, incident.status, incident.id, incident.title
        );
    }
    Ok(())
}

fn should_use_tui() -> bool {
    std::io::stdout().is_terminal() && std::io::stdin().is_terminal()
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
