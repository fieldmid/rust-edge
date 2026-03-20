use std::{io::IsTerminal, net::ToSocketAddrs, time::Duration};

use anyhow::{Context, Result};
use powersync::{PowerSyncDatabase, StreamSubscription, SyncOptions};
use reqwest::Url;

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
                host_name: current_host_name(),
                hardware_model: current_device_model(),
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
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
        println!(
            "device_runtime host={} os={} arch={}",
            current_host_name(),
            std::env::consts::OS,
            std::env::consts::ARCH,
        );

        tokio::select! {
            result = watcher::watch_sync_status(db.clone()) => result?,
            result = watcher::watch_live_incidents(db.clone()) => result?,
            result = watcher::watch_for_local_writes(db.clone()) => result?,
            result = tokio::signal::ctrl_c() => {
                result.context("failed to listen for shutdown signal")?;
            }
        }
    }

    db.disconnect().await;
    Ok(())
}

fn current_host_name() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .filter(|h| !h.trim().is_empty())
        .unwrap_or_else(|| "unknown-host".to_string())
}

fn current_device_model() -> Option<String> {
    std::fs::read_to_string("/sys/class/dmi/id/product_name")
        .ok()
        .or_else(|| std::fs::read_to_string("/sys/devices/virtual/dmi/id/product_name").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub async fn login() -> Result<()> {
    banner::print_banner();
    println!("\n  \x1b[1mFieldMid Edge — Authentication\x1b[0m\n");

    dotenvy::dotenv().ok();

    let supabase_url = std::env::var("SUPABASE_URL")
        .unwrap_or_else(|_| "https://ktohrdqtvqimvcostvcu.supabase.co".to_string());
    let anon_key = std::env::var("SUPABASE_ANON_KEY")
        .unwrap_or_else(|_| "sb_publishable_McJGO7Sh2_JR81mmbKkVZA_AydlBOHQ".to_string());
    let core_url = std::env::var("FIELDMID_DASHBOARD_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());

    // Determine login method
    let use_browser = if std::io::stdin().is_terminal() {
        let items = vec![
            "Browser login (recommended — opens your browser)",
            "Email & password (enter credentials in terminal)",
        ];
        let selection = dialoguer::Select::new()
            .with_prompt("Choose login method")
            .items(&items)
            .default(0)
            .interact()
            .context("failed to select login method")?;
        selection == 0
    } else {
        // Non-interactive: default to password flow
        false
    };

    let mut sess = if use_browser {
        auth::browser_login(&supabase_url, &anon_key, &core_url).await?
    } else {
        let email: String = dialoguer::Input::new()
            .with_prompt("  Email")
            .interact_text()
            .context("failed to read email")?;

        let password: String = dialoguer::Password::new()
            .with_prompt("  Password")
            .interact()
            .context("failed to read password")?;

        println!("\n  \x1b[2mAuthenticating...\x1b[0m");

        auth::login(&supabase_url, &anon_key, &email, &password).await?
    };

    println!(
        "  \x1b[32m✓\x1b[0m Authenticated as \x1b[1m{}\x1b[0m",
        sess.email
    );

    // Fetch user profile for org/role info
    print!("  \x1b[2mLoading profile...\x1b[0m");
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
        println!(" \x1b[32m✓\x1b[0m");
    } else {
        println!(" \x1b[33m⚠ no profile found\x1b[0m");
        println!();
        println!("  \x1b[33m⚠ User profile not found.\x1b[0m");
        println!("  Your account exists but has no profile entry.");
        println!("  This usually means:");
        println!("    • You signed up but haven't completed onboarding");
        println!("    • Your organization admin hasn't approved your account yet");
        println!();
        println!("  \x1b[1mNext steps:\x1b[0m");
        println!("    1. Visit the FieldMid dashboard to complete setup");
        println!("    2. Contact your organization admin if you need access");
        println!();
    }

    // Fetch organizations
    let orgs = auth::fetch_organizations(&sess).await.unwrap_or_default();
    if orgs.is_empty() {
        println!();
        println!("  \x1b[33m⚠ No organizations found.\x1b[0m");
        println!("  You are not a member of any organization.");
        println!("  Ask your admin to invite you, or create one in the dashboard.");
    } else {
        println!();
        println!("  \x1b[1mOrganizations:\x1b[0m");
        for (i, org) in orgs.iter().enumerate() {
            let industry = org.industry.as_deref().unwrap_or("—");
            println!("    {}. {} \x1b[2m({})\x1b[0m", i + 1, org.name, industry);
        }

        if orgs.len() == 1 {
            sess.org_id = Some(orgs[0].id.clone());
            sess.org_name = Some(orgs[0].name.clone());
            println!(
                "\n  \x1b[32m✓\x1b[0m Organization set: \x1b[1m{}\x1b[0m",
                orgs[0].name
            );
        } else if orgs.len() > 1 {
            println!();
            let selection: usize = dialoguer::Select::new()
                .with_prompt("  Select organization")
                .items(&orgs.iter().map(|o| o.name.as_str()).collect::<Vec<_>>())
                .default(0)
                .interact()
                .context("failed to select organization")?;

            sess.org_id = Some(orgs[selection].id.clone());
            sess.org_name = Some(orgs[selection].name.clone());
        }
    }

    // Device role follows account role directly.
    // Admin accounts remain admin on the edge device.
    let allowed_role = match sess.role.as_str() {
        "admin" => "admin",
        "supervisor" => "supervisor",
        _ => "field_worker",
    };
    sess.role = allowed_role.to_string();

    // If supervisor, fetch and select site
    if sess.role == "supervisor" {
        let sites = auth::fetch_sites(&sess).await.unwrap_or_default();
        if sites.is_empty() {
            println!();
            println!("  \x1b[33m⚠ No sites found.\x1b[0m");
            println!("  No active sites are available for monitoring.");
            println!("  Ask your organization admin to create and assign sites.");
        } else {
            println!();
            println!("  \x1b[1mAvailable sites:\x1b[0m");
            for (i, site) in sites.iter().enumerate() {
                let loc = site.location.as_deref().unwrap_or("—");
                println!("    {}. {} \x1b[2m({})\x1b[0m", i + 1, site.name, loc);
            }

            let site_selection: usize = dialoguer::Select::new()
                .with_prompt("  Select site to monitor")
                .items(&sites.iter().map(|s| s.name.as_str()).collect::<Vec<_>>())
                .default(0)
                .interact()
                .context("failed to select site")?;

            sess.site_id = Some(sites[site_selection].id.clone());
            sess.site_name = Some(sites[site_selection].name.clone());
        }
    }

    session::save_session(&sess)?;

    // Summary
    println!();
    println!("  \x1b[32m✓ Login successful!\x1b[0m");
    println!();
    println!("  ┌─────────────────────────────────────────┐");
    println!(
        "  │  \x1b[1mUser:\x1b[0m   {} ({})",
        sess.full_name.as_deref().unwrap_or("—"),
        sess.email
    );
    println!("  │  \x1b[1mRole:\x1b[0m   {}", format_role(&sess.role));
    if let Some(org) = &sess.org_name {
        println!("  │  \x1b[1mOrg:\x1b[0m    {}", org);
    }
    if let Some(site) = &sess.site_name {
        println!("  │  \x1b[1mSite:\x1b[0m   {}", site);
    }
    println!("  └─────────────────────────────────────────┘");
    println!();
    println!(
        "  You are now logged in. \x1b[2mHappy monitoring!\x1b[0m"
    );
    println!();
    println!(
        "  Run \x1b[1mfieldmid\x1b[0m to start the edge daemon."
    );
    println!();

    Ok(())
}

pub async fn list_users() -> Result<()> {
    println!();
    let sess = auth::ensure_session().await?;

    if sess.role != "admin" && sess.role != "supervisor" {
        println!("  \x1b[33m⚠ Only admins and supervisors can view org users.\x1b[0m");
        println!();
        return Ok(());
    }

    let org_name = sess.org_name.as_deref().unwrap_or("your org");
    println!(
        "  \x1b[1mFieldMid — Users in {}\x1b[0m",
        org_name
    );
    println!();

    let (users, count) = auth::fetch_org_users(&sess).await?;

    if users.is_empty() {
        println!("  No users found in this organization.");
        println!();
        return Ok(());
    }

    println!(
        "  \x1b[1m{}\x1b[0m user(s) total",
        count
    );
    println!();
    println!(
        "  {:<28} {:<30} {:<14} {}",
        "Name", "Email", "Role", "Status"
    );
    println!("  {}", "─".repeat(80));

    for user in &users {
        let name = user.full_name.as_deref().unwrap_or("—");
        let email = user.email.as_deref().unwrap_or("—");
        let role = user.role.as_deref().unwrap_or("field_worker");
        let status = user.membership_status.as_deref().unwrap_or("—");

        let role_colored = match role {
            "admin" => format!("\x1b[35m{:<14}\x1b[0m", role),
            "supervisor" => format!("\x1b[36m{:<14}\x1b[0m", role),
            _ => format!("\x1b[34m{:<14}\x1b[0m", role),
        };

        let status_colored = match status {
            "approved" => format!("\x1b[32m{}\x1b[0m", status),
            "pending" => format!("\x1b[33m{}\x1b[0m", status),
            "rejected" => format!("\x1b[31m{}\x1b[0m", status),
            _ => status.to_string(),
        };

        println!(
            "  {:<28} {:<30} {} {}",
            truncate(name, 26),
            truncate(email, 28),
            role_colored,
            status_colored,
        );
    }

    println!();
    Ok(())
}

pub async fn list_requests() -> Result<()> {
    println!();
    let sess = auth::ensure_session().await?;

    if sess.role != "admin" && sess.role != "supervisor" {
        println!("  \x1b[33m⚠ Only admins and supervisors can view join requests.\x1b[0m");
        println!();
        return Ok(());
    }

    let org_name = sess.org_name.as_deref().unwrap_or("your org");
    println!(
        "  \x1b[1mFieldMid — Join Requests for {}\x1b[0m",
        org_name
    );
    println!();

    let requests = auth::fetch_join_requests(&sess).await?;

    let pending: Vec<_> = requests.iter().filter(|r| r.status == "pending").collect();
    let others: Vec<_> = requests.iter().filter(|r| r.status != "pending").collect();

    if requests.is_empty() {
        println!("  No join requests found.");
        println!();
        return Ok(());
    }

    if !pending.is_empty() {
        println!(
            "  \x1b[33m● {} pending request(s)\x1b[0m",
            pending.len()
        );
        println!();
        println!(
            "  {:<24} {:<28} {:<16} {}",
            "Name", "Email", "Requested Role", "Date"
        );
        println!("  {}", "─".repeat(80));

        for req in &pending {
            let date = req.created_at.as_deref().unwrap_or("—");
            let short_date = if date.len() > 10 { &date[..10] } else { date };
            println!(
                "  {:<24} {:<28} {:<16} {}",
                truncate(&req.name, 22),
                truncate(&req.email, 26),
                req.requested_role,
                short_date,
            );
        }
        println!();
    } else {
        println!("  \x1b[32m✓\x1b[0m No pending join requests.");
        println!();
    }

    if !others.is_empty() {
        println!("  \x1b[2mPrevious requests:\x1b[0m");
        for req in &others {
            let status_colored = match req.status.as_str() {
                "approved" => format!("\x1b[32m{}\x1b[0m", req.status),
                "rejected" => format!("\x1b[31m{}\x1b[0m", req.status),
                _ => req.status.clone(),
            };
            let date = req.created_at.as_deref().unwrap_or("—");
            let short_date = if date.len() > 10 { &date[..10] } else { date };
            println!(
                "    {} — {} ({}) [{}]",
                req.name, req.email, status_colored, short_date
            );
        }
        println!();
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

pub fn logout() -> Result<()> {
    if !session::has_session() {
        println!();
        println!("  \x1b[2mNo active session. Already logged out.\x1b[0m");
        println!();
        return Ok(());
    }

    // Show who we're logging out
    if let Ok(sess) = session::load_session() {
        println!();
        println!(
            "  Logging out \x1b[1m{}\x1b[0m...",
            sess.email
        );
    }

    session::clear_session()?;
    println!("  \x1b[32m✓\x1b[0m Session cleared.");
    println!();
    Ok(())
}

pub fn print_install_hint() -> Result<()> {
    let script_url = std::env::var("FIELDMID_INSTALL_SCRIPT_URL")
        .unwrap_or_else(|_| "https://downloads.fieldmid.dev/install.sh".to_string());

    println!();
    println!("  \x1b[1mFieldMid CLI install (planned)\x1b[0m");
    println!();
    println!("  curl -fsSL {} | sh", script_url);
    println!();
    println!("  Tip: set FIELDMID_INSTALL_SCRIPT_URL to test a staging installer URL.");
    println!();

    Ok(())
}

pub async fn whoami() -> Result<()> {
    println!();
    if !session::has_session() {
        println!("  \x1b[31m✗ Not logged in.\x1b[0m");
        println!();
        println!("  Run \x1b[1mfieldmid login\x1b[0m to authenticate.");
        println!();
        return Ok(());
    }

    let sess = auth::ensure_session().await?;

    println!("  \x1b[1mFieldMid Edge — Current Session\x1b[0m");
    println!();
    println!(
        "  User:   \x1b[1m{}\x1b[0m ({})",
        sess.full_name.as_deref().unwrap_or("—"),
        sess.email
    );
    println!("  Role:   {}", format_role(&sess.role));
    println!("  UserID: \x1b[2m{}\x1b[0m", sess.user_id);
    if let Some(org) = &sess.org_name {
        println!(
            "  Org:    {} \x1b[2m({})\x1b[0m",
            org,
            sess.org_id.as_deref().unwrap_or("—")
        );
    }
    if let Some(site) = &sess.site_name {
        println!(
            "  Site:   {} \x1b[2m({})\x1b[0m",
            site,
            sess.site_id.as_deref().unwrap_or("—")
        );
    }

    let now = chrono::Utc::now().timestamp();
    let remaining = sess.expires_at - now;
    if remaining > 0 {
        let mins = remaining / 60;
        if mins > 30 {
            println!(
                "  Token:  \x1b[32m● valid\x1b[0m ({}m remaining)",
                mins
            );
        } else {
            println!(
                "  Token:  \x1b[33m● expiring soon\x1b[0m ({}m remaining)",
                mins
            );
        }
    } else {
        println!(
            "  Token:  \x1b[31m● expired\x1b[0m (will auto-refresh on next run)"
        );
    }
    println!();

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

    println!();
    println!("  \x1b[1mFieldMid Edge — Connectivity Check\x1b[0m");
    println!();

    let status = watcher::wait_for_first_sync_status(db.clone(), Duration::from_secs(10)).await?;
    let queue_depth = watcher::read_local_write_queue_depth(&db).await?;
    let incidents = watcher::fetch_live_incidents(&db, 5).await?;

    let status_icon = if status == "connected" {
        "\x1b[32m●\x1b[0m"
    } else {
        "\x1b[31m●\x1b[0m"
    };

    println!("  Sync Status:    {} {}", status_icon, status);
    println!("  Write Queue:    {} pending", queue_depth);
    if let Some(role) = &config.role {
        println!("  Role:           {}", format_role(role));
    }
    if let Some(org) = &config.org_name {
        println!("  Organization:   {}", org);
    }
    println!("  Incidents:      {} live", incidents.len());

    if !incidents.is_empty() {
        println!();
        println!("  \x1b[1mRecent Live Incidents:\x1b[0m");
        for incident in incidents {
            let created_at = incident.created_at.as_deref().unwrap_or("—");
            println!(
                "    \x1b[2m{}\x1b[0m [{}] {} \x1b[2m({})\x1b[0m",
                created_at, incident.severity, incident.title, incident.status
            );
        }
    }

    println!();
    db.disconnect().await;
    Ok(())
}

pub async fn doctor() -> Result<()> {
    dotenvy::dotenv().ok();

    println!();
    println!("  \x1b[1mFieldMid Edge — Doctor\x1b[0m");
    println!();

    let base = BaseConfig::from_env();
    let host = current_host_name();
    let model = current_device_model().unwrap_or_else(|| "unknown".to_string());

    println!("  \x1b[1mRuntime:\x1b[0m");
    println!("    Host:      {}", host);
    println!("    Model:     {}", model);
    println!("    Device ID: {}", base.device_id);
    println!("    OS/Arch:   {}/{}", std::env::consts::OS, std::env::consts::ARCH);
    println!("    DB Path:   {}", base.database_path.display());

    let db_exists = std::path::Path::new(&base.database_path).exists();
    println!(
        "    DB File:   {}",
        if db_exists { "present" } else { "missing (will be created on run)" }
    );

    println!();
    println!("  \x1b[1mEnvironment:\x1b[0m");
    let powersync_url = std::env::var("POWERSYNC_URL").unwrap_or_default();
    let dashboard_url = std::env::var("FIELDMID_DASHBOARD_URL").unwrap_or_default();
    println!("    POWERSYNC_URL:        {}", mask_env_value(&powersync_url));
    println!(
        "    SUPABASE_URL:         {}",
        mask_env_value(&std::env::var("SUPABASE_URL").unwrap_or_default())
    );
    println!(
        "    SUPABASE_ANON_KEY:    {}",
        if std::env::var("SUPABASE_ANON_KEY").ok().filter(|v| !v.is_empty()).is_some() {
            "set"
        } else {
            "missing"
        }
    );
    println!(
        "    FIELDMID_DASHBOARD_URL: {}",
        mask_env_value(&dashboard_url)
    );

    println!();
    println!("  \x1b[1mSession:\x1b[0m");
    if session::has_session() {
        match auth::ensure_session().await {
            Ok(sess) => {
                let remaining = sess.expires_at - chrono::Utc::now().timestamp();
                println!("    Status:    valid");
                println!("    User:      {}", sess.email);
                println!("    Role:      {}", sess.role);
                println!("    Expires:   {}s remaining", remaining.max(0));
            }
            Err(error) => {
                println!("    Status:    invalid ({})", error);
                println!("    Fix:       run cargo run -- logout && cargo run -- login");
            }
        }
    } else {
        println!("    Status:    missing (run cargo run -- login)");
    }

    println!();
    println!("  \x1b[1mPowerSync Connectivity:\x1b[0m");
    if powersync_url.is_empty() {
        println!("    DNS:       skipped (POWERSYNC_URL missing)");
        println!("    HTTP:      skipped (POWERSYNC_URL missing)");
    } else if let Ok(url) = Url::parse(&powersync_url) {
        let host = url.host_str().unwrap_or_default();
        let port = url.port_or_known_default().unwrap_or(443);
        let lookup = format!("{}:{}", host, port);

        match lookup.to_socket_addrs() {
            Ok(mut addrs) => {
                if let Some(addr) = addrs.next() {
                    println!("    DNS:       ok ({})", addr.ip());
                } else {
                    println!("    DNS:       no addresses returned");
                }
            }
            Err(error) => println!("    DNS:       failed ({})", error),
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .context("failed to construct HTTP client")?;

        match client.get(url.clone()).send().await {
            Ok(response) => println!("    HTTP:      ok (status {})", response.status()),
            Err(error) => println!("    HTTP:      failed ({})", error),
        }
    } else {
        println!("    DNS:       skipped (invalid POWERSYNC_URL)");
        println!("    HTTP:      skipped (invalid POWERSYNC_URL)");
    }

    if db_exists {
        if let Ok(context) = open_database(&base.database_path) {
            if let Ok(incidents) = watcher::fetch_live_incidents(&context.db, 3).await {
                println!();
                println!("  \x1b[1mLocal Live Snapshot:\x1b[0m {} row(s)", incidents.len());
                for incident in incidents {
                    println!(
                        "    - [{}] {} [{}]",
                        incident.severity, incident.title, incident.status
                    );
                }
            }
        }
    }

    println!();
    println!("  Doctor complete.");
    println!();

    Ok(())
}

pub async fn show_latest_incidents() -> Result<()> {
    let base = BaseConfig::from_env();
    let context = open_database(&base.database_path)?;
    let incidents = watcher::fetch_live_incidents(&context.db, 20).await?;

    println!();
    println!(
        "  \x1b[1mFieldMid Edge — Live Incidents\x1b[0m \x1b[2m({})\x1b[0m",
        base.database_path.display()
    );
    println!();

    if incidents.is_empty() {
        println!("  \x1b[32m✓\x1b[0m No incidents found.");
    } else {
        println!(
            "  Found \x1b[1m{}\x1b[0m live incident(s):",
            incidents.len()
        );
        println!();
        for incident in incidents {
            let created_at = incident.created_at.as_deref().unwrap_or("—");
            let status_color = match incident.status.as_str() {
                "open" => "\x1b[31m",
                "in_progress" => "\x1b[33m",
                "resolved" => "\x1b[32m",
                _ => "\x1b[2m",
            };
            println!(
                "    \x1b[2m{}\x1b[0m  [{}] {}{}\x1b[0m  {}",
                created_at, incident.severity, status_color, incident.status, incident.title
            );
        }
    }
    println!();
    Ok(())
}

fn should_use_tui() -> bool {
    std::io::stdout().is_terminal() && std::io::stdin().is_terminal()
}

fn mask_env_value(value: &str) -> String {
    if value.trim().is_empty() {
        return "missing".to_string();
    }

    if value.len() <= 16 {
        return "set".to_string();
    }

    let start = &value[..8];
    let end = &value[value.len() - 8..];
    format!("{}...{}", start, end)
}

fn format_role(role: &str) -> String {
    match role {
        "admin" => "\x1b[35madmin\x1b[0m".to_string(),
        "supervisor" => "\x1b[36msupervisor\x1b[0m".to_string(),
        "field_worker" => "\x1b[34mfield_worker\x1b[0m".to_string(),
        other => other.to_string(),
    }
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

    println!(
        "  \x1b[32m✓\x1b[0m Sync stream subscribed: {}",
        stream.name
    );
    Ok(Some(subscription))
}
