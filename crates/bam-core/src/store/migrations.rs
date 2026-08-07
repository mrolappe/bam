use rusqlite::{Connection, Result};

struct Migration {
    version: i64,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("../../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("../../migrations/0002_http_cache.sql"),
    },
    Migration {
        version: 3,
        sql: include_str!("../../migrations/0003_fetch_queue.sql"),
    },
    Migration {
        version: 4,
        sql: include_str!("../../migrations/0004_landing_readme.sql"),
    },
];

/// Runs every migration newer than the DB's `user_version`, in order, and
/// advances `user_version` to match. No down-migrations: schema only moves
/// forward, so re-running against an up-to-date DB is a no-op.
pub fn apply_migrations(conn: &Connection) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    for migration in MIGRATIONS {
        if migration.version > current {
            conn.execute_batch(migration.sql)?;
            conn.pragma_update(None, "user_version", migration.version)?;
        }
    }
    Ok(())
}
