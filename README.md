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
```
