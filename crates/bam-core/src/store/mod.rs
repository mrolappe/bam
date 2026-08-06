use std::path::Path;

use rusqlite::Connection;

mod migrations;
pub mod tables;

pub use migrations::apply_migrations;

pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    apply_migrations(&conn)?;
    Ok(conn)
}
