# FieldMid Rust Edge Daemon

Read-only PowerSync daemon for edge monitoring of critical incidents.

## Setup

```bash
cp .env.example .env
cargo run -- check-connectivity
```

## Required env vars

- `POWERSYNC_URL`
- `POWERSYNC_TOKEN`

Optional:
- `POWERSYNC_STREAM=edge_critical_feed`
- `POWERSYNC_STREAM_PARAMS={"site_id":"<site-uuid>"}`
- `DEVICE_ID`
- `FIELDMID_DB_PATH`

## Commands

```bash
cargo run -- run
cargo run -- check-connectivity
cargo run -- latest-incidents
cargo run -- install-hint
```

## Planned curl install flow

Use the helper command to print the installer command that users will run:

```bash
cargo run -- install-hint
```

Default output:

```bash
curl -fsSL https://downloads.fieldmid.dev/install.sh | sh
```

You can override the installer URL for staging/tests:

```bash
FIELDMID_INSTALL_SCRIPT_URL=https://staging-downloads.fieldmid.dev/install.sh cargo run -- install-hint
```
