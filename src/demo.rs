use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::params;

use crate::{config::BaseConfig, database::open_database};

pub async fn insert_demo_critical() -> Result<()> {
    let config = BaseConfig::from_env();
    let context = open_database(&config.database_path)?;
    let id = format!("demo-{}", current_millis()?);
    let created_at = current_millis()?.to_string();

    let writer = context
        .db
        .writer()
        .await
        .context("failed to open SQLite writer")?;

    writer.execute(
        "INSERT INTO incidents (id, worker_id, site_id, title, description, severity, ai_severity, status, created_at, synced_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            id,
            "demo-worker",
            "demo-site",
            "Demo critical incident",
            "Inserted from rust-edge for manual testing",
            "CRITICAL",
            "CRITICAL",
            "OPEN",
            created_at,
            Option::<String>::None
        ],
    )?;

    println!(
        "inserted_demo_incident database_path={}",
        config.database_path.display()
    );
    Ok(())
}

fn current_millis() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?
        .as_millis())
}
