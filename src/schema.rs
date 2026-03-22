use powersync::schema::{Column, Schema, Table};

pub fn app_schema() -> Schema {
    Schema {
        tables: vec![
            sites_table(),
            incidents_table(),
            escalations_table(),
            sync_logs_table(),
            user_profiles_table(),
            org_join_requests_table(),
            notifications_table(),
        ],
        raw_tables: Vec::new(),
    }
}

fn sites_table() -> Table {
    Table::create(
        "sites",
        vec![
            Column::text("org_id"),
            Column::text("name"),
            Column::text("location"),
            Column::text("site_type"),
            Column::text("lat"),
            Column::text("lng"),
            Column::integer("active"),
            Column::text("created_at"),
        ],
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
            Column::text("supervisor_id"),
            Column::integer("escalation_level"),
            Column::text("message"),
            Column::text("channel"),
            Column::integer("acknowledged"),
            Column::text("acknowledged_at"),
            Column::text("auto_escalate_at"),
            Column::text("escalated_at"),
            Column::text("created_at"),
            Column::text("updated_at"),
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
            Column::integer("pending_downloads"),
            Column::text("sync_status"),
            Column::text("updated_at"),
        ],
        |table| {
            table.options.local_only = true;
        },
    )
}

fn user_profiles_table() -> Table {
    Table::create(
        "user_profiles",
        vec![
            Column::text("email"),
            Column::text("full_name"),
            Column::text("role"),
            Column::text("org_id"),
            Column::text("site_id"),
            Column::text("membership_status"),
            Column::text("created_at"),
        ],
        |_| {},
    )
}

fn org_join_requests_table() -> Table {
    Table::create(
        "org_join_requests",
        vec![
            Column::text("org_id"),
            Column::text("requester_user_id"),
            Column::text("requested_role"),
            Column::text("status"),
            Column::text("message"),
            Column::text("decided_by"),
            Column::text("decided_at"),
            Column::text("created_at"),
        ],
        |_| {},
    )
}

fn notifications_table() -> Table {
    Table::create(
        "notifications",
        vec![
            Column::text("user_id"),
            Column::text("title"),
            Column::text("body"),
            Column::text("notification_type"),
            Column::integer("read"),
            Column::text("created_at"),
        ],
        |_| {},
    )
}
