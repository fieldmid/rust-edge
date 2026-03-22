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

- **Local-First & Offline-First** — All data (incidents, join requests, user profiles) synced to local SQLite via PowerSync; queries read from local DB first, auto-syncs from cloud when local is empty
- **Read-Only Sync** — Receives incident data from PowerSync; strictly prevents local writes
- **Interactive TUI** — Live dashboard showing incidents, escalations, and sync status (auto-detects terminal)
- **Headless Mode** — Falls back to structured logging when no terminal is available (servers, containers)
- **Join Request Management** — View pending requests and interactively approve or reject them from the CLI via Supabase RPC
- **Browser-Based Login** — Authenticate via browser without credentials touching the terminal
- **Role-Based Streams** — Admins, supervisors, and field workers see different data scoped by 9 PowerSync Sync Streams
- **WiFi Telemetry** — Monitors signal quality from `/proc/net/wireless`
- **Diagnostics** — Built-in `doctor` command for connectivity and configuration troubleshooting
- **curl Install** — One-line installer script with platform detection and cargo fallback

## Install

```bash
curl -fsSL https://fieldmid.com/install.sh | sh
```

Or build from source:

```bash
cargo install --git https://github.com/fieldmid/rust-edge-repo.git
```

## Commands

```bash
fieldmid                         # Start daemon (TUI or headless)
fieldmid run                     # Explicit daemon start
fieldmid login                   # Authenticate (browser or email/password)
fieldmid logout                  # Clear session
fieldmid whoami                  # Show current session info
fieldmid users                   # List org users (admin/supervisor only)
fieldmid requests                # View and approve/reject join requests
fieldmid doctor                  # Diagnose environment, DB, DNS, connectivity
fieldmid check-connectivity      # Test PowerSync connection
fieldmid latest-incidents        # Show latest 20 incidents (syncs if local DB empty)
fieldmid install-hint            # Print install options
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

**Local Schema (7 tables synced via PowerSync):**
- `incidents` — Safety incidents with severity and AI analysis
- `sites` — Monitored field sites
- `escalations` — Escalation tracking
- `user_profiles` — User roles, org membership, and status
- `org_join_requests` — Join requests with approval workflow
- `notifications` — In-app alerts and push notification payloads
- `sync_logs` — Local sync metadata (local-only, not synced to backend)

## Local-First & Offline-First

- **PowerSync-synced local SQLite** — Incidents, join requests, user profiles, and notifications are synced to a local SQLite database via PowerSync
- **Offline reads** — All queries (`latest-incidents`, `requests`, TUI dashboard) read from local SQLite first. If the local DB is empty, the CLI auto-connects to PowerSync and syncs before showing data
- **Offline request queue** — Join requests read from the local replica are always available, even without connectivity
- **Online actions** — Approve/reject decisions are sent to Supabase via RPC when connectivity is available
- **Opportunistic sync** — The daemon continuously syncs in the background when connected; cached data is shown instantly when offline

## Key Design Decisions

- **Strict read-only** — `upload_data()` always returns error to prevent accidental writes
- **Auto-detect terminal** — TUI mode when stdout/stdin are terminals, headless otherwise
- **Device auto-ID** — Combines hostname + hardware model for unique identification
- **Session persistence** — JSON file at `~/.fieldmid/session.json`
- **Write queue guard** — Actively monitors and blocks any local modification attempts

## Related Repos

| Repo | Purpose |
|------|---------|
| [supabase-repo](../supabase-repo) | Backend (provides `edge_critical_feed` PowerSync stream) |
| [core-repo](../core-repo) | Web dashboard for supervisors and admins |
| [mobile-app-repo](../mobile-app-repo) | Mobile app for field incident reporting |
| [mastra-agents-repo](../mastra-agents-repo) | AI agents for triage and escalation |
| [landing-page-repo](../landing-page-repo) | Marketing landing page |
