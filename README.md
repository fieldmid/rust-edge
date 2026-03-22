# FieldMid Rust Edge Daemon

A read-only PowerSync daemon for monitoring critical field incidents on edge devices. Designed for rugged Linux hardware (industrial PCs, gateways, Raspberry Pi) deployed at remote sites. Features an interactive TUI dashboard and CLI tools for authentication, diagnostics, and incident viewing.

## Tech Stack

- **Rust** (2024 edition)
- **PowerSync SDK** v0.0.4 — Real-time data sync
- **SQLite** (via rusqlite) — Local persistent storage
- **Ratatui** — Terminal UI dashboard
- **Crossterm** — Terminal control
- **Tokio** — Async runtime
- **Supabase** — Authentication and session management
- **Serde** — JSON serialization

## Features

- **Read-Only Sync** — Receives incident data from PowerSync; strictly prevents local writes
- **Interactive TUI** — Live dashboard showing incidents, escalations, and sync status (auto-detects terminal)
- **Headless Mode** — Falls back to structured logging when no terminal is available (servers, containers)
- **Browser-Based Login** — Authenticate via browser without credentials touching the terminal
- **Role-Based Streams** — Admins, supervisors, and field workers see different data
- **WiFi Telemetry** — Monitors signal quality from `/proc/net/wireless`
- **Diagnostics** — Built-in `doctor` command for connectivity and configuration troubleshooting

## Commands

```bash
fieldmid                         # Start daemon (TUI or headless)
fieldmid run                     # Explicit daemon start
fieldmid login                   # Authenticate (browser or email/password)
fieldmid logout                  # Clear session
fieldmid whoami                  # Show current session info
fieldmid users                   # List org users (admin/supervisor only)
fieldmid requests                # View pending join requests
fieldmid doctor                  # Diagnose environment, DB, DNS, connectivity
fieldmid check-connectivity      # Test PowerSync connection
fieldmid latest-incidents        # Show latest 20 incidents from local DB
fieldmid install-hint            # Print curl installer command
fieldmid help                    # Show help
```

## Setup

### 1. Environment Variables

```bash
cp .env.example .env
```

Required:
```env
POWERSYNC_URL=https://<instance>.powersync.journeyapps.com
POWERSYNC_TOKEN=<jwt-token>
```

Optional:
```env
POWERSYNC_STREAM=edge_critical_feed
POWERSYNC_STREAM_PARAMS={"site_id":"<site-uuid>"}
DEVICE_ID=<custom-device-id>
FIELDMID_DB_PATH=fieldmid-edge.db
SUPABASE_URL=https://<project-ref>.supabase.co
SUPABASE_ANON_KEY=<anon-key>
FIELDMID_DASHBOARD_URL=http://localhost:3000
```

### 2. Build & Run

```bash
cargo build
cargo run
```

Or with specific commands:
```bash
cargo run -- login
cargo run -- doctor
cargo run -- latest-incidents
```

## Authentication Flow

1. Run `fieldmid login` — choose browser login (recommended) or email/password
2. Browser opens Supabase auth page; no credentials enter the terminal
3. Session saved to `~/.fieldmid/session.json`
4. Select organization (auto-selects if only one exists)
5. Supervisors select which site to monitor

## Role-Based Data Streams

| Role | Stream | Data |
|------|--------|------|
| Admin | `admin_overview` | All org incidents (30-day window) |
| Supervisor | `supervisor_site` | All incidents for supervised site |
| Field Worker | — | Read-only incident access |

## Architecture

```
PowerSync Cloud
    |  (real-time sync)
Local SQLite (fieldmid-edge.db)
    |
Watcher threads (incidents, sync status, write guard)
    |
TUI Renderer (ratatui) or Headless Logger
```

**Local Schema:**
- `incidents` — Safety incidents with severity and AI analysis
- `sites` — Monitored field sites
- `escalations` — Escalation tracking
- `sync_logs` — Local sync metadata (not synced to backend)

## Key Design Decisions

- **Strict read-only** — `upload_data()` always returns error to prevent accidental writes
- **Auto-detect terminal** — TUI mode when stdout/stdin are terminals, headless otherwise
- **Device auto-ID** — Combines hostname + hardware model for unique identification
- **Session persistence** — JSON file at `~/.fieldmid/session.json`
- **Write queue guard** — Actively monitors and blocks any local modification attempts

## Planned: curl Install

```bash
curl -fsSL https://downloads.fieldmid.dev/install.sh | sh
```

Override for staging:
```bash
FIELDMID_INSTALL_SCRIPT_URL=https://staging.fieldmid.dev/install.sh cargo run -- install-hint
```

## Related Repos

| Repo | Purpose |
|------|---------|
| [supabase-repo](../supabase-repo) | Backend (provides `edge_critical_feed` PowerSync stream) |
| [core-repo](../core-repo) | Web dashboard for supervisors and admins |
| [mobile-app-repo](../mobile-app-repo) | Mobile app for field incident reporting |
| [mastra-agents-repo](../mastra-agents-repo) | AI agents for triage and escalation |
| [landing-page-repo](../landing-page-repo) | Marketing landing page |
