use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

use super::SCHEMA;

/// Open a clean graph database at `path`, replacing whatever was there.
/// `analyze` always does a full rebuild (see module docs on `schema.sql`),
/// so "open" here really means "recreate."
pub fn open_fresh(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if path.exists() {
        std::fs::remove_file(path)
            .with_context(|| format!("failed to remove stale graph at {}", path.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open graph database at {}", path.display()))?;
    conn.execute_batch(SCHEMA)
        .context("failed to apply graph schema")?;
    Ok(conn)
}

/// An in-memory graph with the schema applied. Used by tests so they don't
/// touch the filesystem.
// DECISION: only called from #[cfg(test)] today; kept #[allow] rather than
// cfg-gating the fn itself since it's a normal public building block, not
// test-only infrastructure.
#[allow(dead_code)]
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory().context("failed to open in-memory graph database")?;
    conn.execute_batch(SCHEMA)
        .context("failed to apply graph schema")?;
    Ok(conn)
}
