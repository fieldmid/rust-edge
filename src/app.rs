use std::{io::IsTerminal, time::Duration};

use anyhow::{Context, Result};
use powersync::{PowerSyncDatabase, StreamSubscription, SyncOptions};

use crate::{
    auth, banner,
    config::{BaseConfig, DaemonConfig},
    connector::FieldMidConnector,
    database::open_database,
    session, tui, watcher,
};

pub async fn run() -> Result<()> {
    let config = DaemonConfig::from_env_or_session().await?;
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
                role: config.role.clone(),
                org_name: config.org_name.clone(),
                email: config.email.clone(),
            },
        )
        .await?;
    } else {
        banner::print_banner();
        println!(
            "fieldmid_edge_started device_id={} database_path={} mode=read_only stream_subscription={}{}{}",
            config.base.device_id,
            config.base.database_path.display(),
            if config.sync_stream.is_some() {
                "enabled"
            } else {
                "disabled"
            },
            config.role.as_deref().map(|r| format!(" role={r}")).unwrap_or_default(),
            config.org_name.as_deref().map(|o| format!(" org={o}")).unwrap_or_default(),
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

pub async fn login() -> Result<()> {
    banner::print_banner();
    println!("\n  FieldMid Edge — Authentication\n");

    dotenvy::dotenv().ok();

    let supabase_url = std::env::var("SUPABASE_URL")
        .unwrap_or_else(|_| "https://ktohrdqtvqimvcostvcu.supabase.co".to_string());
    let anon_key = std::env::var("SUPABASE_ANON_KEY")
        .unwrap_or_else(|_| "sb_publishable_McJGO7Sh2_JR81mmbKkVZA_AydlBOHQ".to_string());

    let email: String = dialoguer::Input::new()
        .with_prompt("Email")
        .interact_text()
        .context("failed to read email")?;

    let password: String = dialoguer::Password::new()
        .with_prompt("Password")
        .interact()
        .context("failed to read password")?;

    println!("\nauthenticating...");

    let mut sess = auth::login(&supabase_url, &anon_key, &email, &password).await?;

    println!("authenticated as {}", sess.email);

    // Fetch user profile for org/role info
    if let Ok(Some(profile)) = auth::fetch_user_profile(&sess).await {
        if let Some(role) = &profile.role {
            sess.role = role.clone();
        }
        if let Some(org_id) = &profile.org_id {
            sess.org_id = Some(org_id.clone());
        }
        if let Some(name) = &profile.full_name {
            sess.full_name = Some(name.clone());
        }
    }

    // Fetch organizations the user can see
    let orgs = auth::fetch_organizations(&sess).await.unwrap_or_default();
    if !orgs.is_empty() {
        println!("\nOrganizations:");
        for (i, org) in orgs.iter().enumerate() {
            let industry = org.industry.as_deref().unwrap_or("-");
            println!("  {}. {} ({})", i + 1, org.name, industry);
        }

        if orgs.len() == 1 {
            sess.org_id = Some(orgs[0].id.clone());
            sess.org_name = Some(orgs[0].name.clone());
            println!("\nOrganization set: {}", orgs[0].name);
        } else if orgs.len() > 1 {
            let selection: usize = dialoguer::Select::new()
                .with_prompt("Select organization")
                .items(&orgs.iter().map(|o| o.name.as_str()).collect::<Vec<_>>())
                .default(0)
                .interact()
                .context("failed to select organization")?;

            sess.org_id = Some(orgs[selection].id.clone());
            sess.org_name = Some(orgs[selection].name.clone());
        }
    }

    // Role selection for edge daemon
    let role_options = if sess.role == "admin" {
        vec!["admin", "supervisor"]
    } else if sess.role == "supervisor" {
        vec!["supervisor"]
    } else {
        vec!["field_worker"]
    };

    if role_options.len() > 1 {
        println!();
        let role_selection: usize = dialoguer::Select::new()
            .with_prompt("Choose your role for this edge device")
            .items(&role_options.iter().map(|r| match *r {
                "admin" => "Org Admin (full overview of all incidents)",
                "supervisor" => "Supervisor (site-specific incidents and escalations)",
                _ => "Field Worker",
            }).collect::<Vec<_>>())
            .default(0)
            .interact()
            .context("failed to select role")?;

        sess.role = role_options[role_selection].to_string();
    }

    // If supervisor, fetch and select site
    if sess.role == "supervisor" {
        let sites = auth::fetch_sites(&sess).await.unwrap_or_default();
        if !sites.is_empty() {
            println!("\nAvailable sites:");
            for (i, site) in sites.iter().enumerate() {
                let loc = site.location.as_deref().unwrap_or("-");
                println!("  {}. {} ({})", i + 1, site.name, loc);
            }

            let site_selection: usize = dialoguer::Select::new()
                .with_prompt("Select site to monitor")
                .items(&sites.iter().map(|s| s.name.as_str()).collect::<Vec<_>>())
                .default(0)
                .interact()
                .context("failed to select site")?;

            sess.site_id = Some(sites[site_selection].id.clone());
            sess.site_name = Some(sites[site_selection].name.clone());
        }
    }

    session::save_session(&sess)?;

    println!("\nSession saved. Summary:");
    println!("  User:  {} ({})", sess.full_name.as_deref().unwrap_or(&sess.email), sess.email);
    println!("  Role:  {}", sess.role);
    if let Some(org) = &sess.org_name {
        println!("  Org:   {}", org);
    }
    if let Some(site) = &sess.site_name {
        println!("  Site:  {}", site);
    }
    println!("\nRun `fieldmid` to start the edge daemon.");

    Ok(())
}

pub fn logout() -> Result<()> {
    session::clear_session()?;
    println!("session cleared");
    Ok(())
}

pub async fn whoami() -> Result<()> {
    if !session::has_session() {
        println!("not logged in — run `fieldmid login`");
        return Ok(());
    }

    let sess = auth::ensure_session().await?;

    println!("FieldMid Edge — Current Session");
    println!("  User:   {} ({})", sess.full_name.as_deref().unwrap_or("-"), sess.email);
    println!("  Role:   {}", sess.role);
    println!("  UserID: {}", sess.user_id);
    if let Some(org) = &sess.org_name {
        println!("  Org:    {} ({})", org, sess.org_id.as_deref().unwrap_or("-"));
    }
    if let Some(site) = &sess.site_name {
        println!("  Site:   {} ({})", site, sess.site_id.as_deref().unwrap_or("-"));
    }

    let now = chrono::Utc::now().timestamp();
    let remaining = sess.expires_at - now;
    if remaining > 0 {
        println!("  Token:  valid ({}m remaining)", remaining / 60);
    } else {
        println!("  Token:  expired (will auto-refresh on next run)");
    }

    Ok(())
}

pub async fn check_connectivity() -> Result<()> {
    let config = DaemonConfig::from_env_or_session().await?;
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
    if let Some(role) = &config.role {
        println!("role={role}");
    }
    if let Some(org) = &config.org_name {
        println!("org={org}");
    }
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
