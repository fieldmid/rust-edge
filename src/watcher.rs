use std::{pin::pin, time::Duration};

use anyhow::{Context, Result, bail};
use futures_lite::StreamExt;
use powersync::{PowerSyncDatabase, SyncStatusData};
use rusqlite::params;

const DEFAULT_INCIDENT_LIMIT: i64 = 20;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct IncidentSummary {
    pub id: String,
    pub title: String,
    pub severity: String,
    pub status: String,
    pub created_at: Option<String>,
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

pub async fn watch_live_incidents(db: PowerSyncDatabase) -> Result<()> {
    let stream = db.watch_statement(
        format!(
            "SELECT id, title, COALESCE(ai_severity, severity, 'UNKNOWN') AS severity, status, created_at FROM incidents ORDER BY created_at DESC LIMIT {DEFAULT_INCIDENT_LIMIT}"
        ),
        params![],
        |stmt, params| {
            let mut rows = stmt.query(params)?;
            let mut incidents = Vec::new();

            while let Some(row) = rows.next()? {
                incidents.push(IncidentSummary {
                    id: row.get("id")?,
                    title: row.get("title")?,
                    severity: row
                        .get::<_, Option<String>>("severity")?
                        .unwrap_or_else(|| "UNKNOWN".to_string()),
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

pub async fn fetch_live_incidents(
    db: &PowerSyncDatabase,
    limit: i64,
) -> Result<Vec<IncidentSummary>> {
    let reader = db
        .reader()
        .await
        .context("failed to open SQLite reader for incidents snapshot")?;
    let mut stmt = reader.prepare(
        "SELECT id, title, COALESCE(ai_severity, severity, 'UNKNOWN') AS severity, status, created_at FROM incidents ORDER BY created_at DESC LIMIT ?",
    )?;
    let mut rows = stmt.query(params![limit])?;
    let mut incidents = Vec::new();

    while let Some(row) = rows.next()? {
        incidents.push(IncidentSummary {
            id: row.get("id")?,
            title: row.get("title")?,
            severity: row
                .get::<_, Option<String>>("severity")?
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            status: row
                .get::<_, Option<String>>("status")?
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            created_at: row.get("created_at")?,
        });
    }

    Ok(incidents)
}

pub async fn read_local_write_queue_depth(db: &PowerSyncDatabase) -> Result<i64> {
    let reader = db
        .reader()
        .await
        .context("failed to open queue reader for local write guard")?;
    let count = reader
        .query_row("SELECT COUNT(*) FROM powersync_crud", params![], |row| {
            row.get::<_, i64>(0)
        })
        .or_else(|_| {
            reader.query_row("SELECT COUNT(*) FROM ps_crud", params![], |row| {
                row.get::<_, i64>(0)
            })
        })
        .unwrap_or(0);

    Ok(count)
}

pub async fn assert_no_local_writes(db: &PowerSyncDatabase) -> Result<()> {
    let queue_depth = read_local_write_queue_depth(db).await?;
    if queue_depth > 0 {
        bail!(local_write_guard_message(queue_depth));
    }
    Ok(())
}

pub async fn watch_for_local_writes(db: PowerSyncDatabase) -> Result<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;
        let queue_depth = read_local_write_queue_depth(&db).await?;
        if queue_depth > 0 {
            bail!(local_write_guard_message(queue_depth));
        }
    }
}

pub async fn wait_for_first_sync_status(
    db: PowerSyncDatabase,
    timeout: Duration,
) -> Result<String> {
    let stream = db.watch_status();
    let mut stream = pin!(stream);
    let deadline = tokio::time::Instant::now() + timeout;
    let mut latest = "sync_state=idle".to_string();

    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Ok(latest);
        }

        let remaining = deadline.duration_since(now);
        let maybe_status = tokio::time::timeout(remaining, stream.next())
            .await
            .with_context(|| {
                format!(
                    "timed out waiting for sync status after {}s",
                    timeout.as_secs()
                )
            })?;
        let status = maybe_status.context("sync status stream ended unexpectedly")?;
        latest = format_sync_status(status.as_ref());

        if latest != "sync_state=idle" {
            return Ok(latest);
        }
    }
}

pub fn format_sync_status(status: &SyncStatusData) -> String {
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
    println!("incidents={}", incidents.len());

    for incident in incidents {
        let created_at = incident.created_at.as_deref().unwrap_or("-");
        println!(
            "{} | {} | {} | {} | {}",
            created_at, incident.severity, incident.status, incident.id, incident.title
        );
    }
}

pub fn local_write_guard_message(queue_depth: i64) -> String {
    format!(
        "detected {queue_depth} queued local write(s) in sqlite; rust-edge is strict read-only for the deadline build. reset the local edge database files or remove the local writes before starting the daemon"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_write_guard_message_mentions_queue_depth() {
        let message = local_write_guard_message(3);
        assert!(message.contains("3 queued local write"));
        assert!(message.contains("strict read-only"));
    }
}
