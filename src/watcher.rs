use std::pin::pin;

use anyhow::Result;
use futures_lite::StreamExt;
use powersync::{PowerSyncDatabase, SyncStatusData};
use rusqlite::params;

#[derive(Clone, Eq, PartialEq)]
struct IncidentSummary {
    id: String,
    title: String,
    status: String,
    created_at: Option<String>,
}

pub async fn watch_sync_status(db: PowerSyncDatabase) -> Result<()> {
    let stream = db.watch_status();
    let mut stream = pin!(stream);
    let mut last_line = None::<String>;

    while let Some(status) = stream.next().await {
        let line = format_sync_status(status.as_ref());
        if last_line.as_deref() == Some(line.as_str()) {
            continue;
        }

        println!("{line}");
        last_line = Some(line);
    }

    Ok(())
}

pub async fn watch_critical_incidents(db: PowerSyncDatabase) -> Result<()> {
    let stream = db.watch_statement(
        "SELECT id, title, status, created_at FROM incidents WHERE severity = 'CRITICAL' OR ai_severity = 'CRITICAL' ORDER BY created_at DESC LIMIT 20".to_string(),
        params![],
        |stmt, params| {
            let mut rows = stmt.query(params)?;
            let mut incidents = Vec::new();

            while let Some(row) = rows.next()? {
                incidents.push(IncidentSummary {
                    id: row.get("id")?,
                    title: row.get("title")?,
                    status: row
                        .get::<_, Option<String>>("status")?
                        .unwrap_or_else(|| "UNKNOWN".to_string()),
                    created_at: row.get("created_at")?,
                });
            }

            Ok(incidents)
        },
    );

    let mut stream = pin!(stream);
    let mut last_snapshot = None::<Vec<IncidentSummary>>;

    while let Some(result) = stream.next().await {
        let incidents = result?;
        if last_snapshot.as_ref() == Some(&incidents) {
            continue;
        }

        print_incidents(&incidents);
        last_snapshot = Some(incidents);
    }

    Ok(())
}

fn format_sync_status(status: &SyncStatusData) -> String {
    if let Some(error) = status.download_error() {
        return format!("sync_state=download_error error={error}");
    }

    if let Some(error) = status.upload_error() {
        return format!("sync_state=upload_error error={error}");
    }

    if status.is_uploading() {
        return "sync_state=uploading".to_string();
    }

    if status.is_downloading() {
        return "sync_state=downloading".to_string();
    }

    if status.is_connected() {
        return "sync_state=connected".to_string();
    }

    if status.is_connecting() {
        return "sync_state=connecting".to_string();
    }

    "sync_state=idle".to_string()
}

fn print_incidents(incidents: &[IncidentSummary]) {
    println!("critical_incidents={}", incidents.len());

    for incident in incidents {
        let created_at = incident.created_at.as_deref().unwrap_or("-");
        println!(
            "{} | {} | {} | {}",
            created_at, incident.status, incident.id, incident.title
        );
    }
}
