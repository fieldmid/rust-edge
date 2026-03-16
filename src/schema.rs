use powersync::schema::{Column, Schema, Table};

pub fn app_schema() -> Schema {
    Schema {
        tables: vec![
            sites_table(),
            incidents_table(),
            escalations_table(),
            sync_logs_table(),
        ],
        raw_tables: Vec::new(),
    }
}

fn sites_table() -> Table {
    Table::create(
        "sites",
        vec![Column::text("name"), Column::text("created_at")],
        |_| {},
    )
}

fn incidents_table() -> Table {
    Table::create(
        "incidents",
        vec![
            Column::text("worker_id"),
            Column::text("site_id"),
            Column::text("title"),
            Column::text("description"),
            Column::text("severity"),
            Column::text("ai_severity"),
            Column::text("status"),
            Column::text("created_at"),
            Column::text("synced_at"),
        ],
        |_| {},
    )
}

fn escalations_table() -> Table {
    Table::create(
        "escalations",
        vec![
            Column::text("incident_id"),
            Column::integer("level"),
            Column::text("created_at"),
        ],
        |_| {},
    )
}

fn sync_logs_table() -> Table {
    Table::create(
        "sync_logs",
        vec![
            Column::text("worker_id"),
            Column::text("last_synced_at"),
            Column::integer("pending_uploads"),
            Column::text("sync_status"),
        ],
        |table| {
            table.options.local_only = true;
        },
    )
}
