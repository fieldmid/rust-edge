use anyhow::Result;

pub async fn insert_demo_critical() -> Result<()> {
    anyhow::bail!(
        "insert-demo-critical is disabled; rust-edge is strict read-only for the deadline build"
    )
}
