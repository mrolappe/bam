use std::path::Path;

use rusqlite::Connection;

pub mod compile;
pub mod fetch;
pub mod ingest;
pub mod land;
mod migrations;
pub mod normalize;
pub mod recent;
pub mod session;
pub mod tables;

pub use migrations::apply_migrations;

pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    apply_migrations(&conn)?;
    Ok(conn)
}
