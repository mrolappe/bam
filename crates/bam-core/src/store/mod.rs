use std::path::Path;
use std::sync::Once;
use std::time::Duration;

use rusqlite::Connection;
use rusqlite::ffi::{sqlite3, sqlite3_api_routines, sqlite3_auto_extension};
use std::os::raw::c_char;

pub mod blob_cache;
pub mod compile;
pub mod embeddings;
pub mod fetch;
pub mod fetch_queue;
pub mod fetch_worker;
pub mod fts;
pub mod ingest;
pub mod inventory;
pub mod land;
mod migrations;
pub mod normalize;
pub mod recent;
pub mod session;
pub mod summaries;
pub mod tables;

pub use migrations::apply_migrations;

static VEC_EXTENSION: Once = Once::new();

/// Registers sqlite-vec (P7.4) as an auto-extension, once per process:
/// `sqlite3_auto_extension` applies to every `Connection` opened afterwards,
/// so every future `open()` call gets `vec_distance_cosine` for free without
/// re-registering per connection.
fn register_vec_extension() {
    VEC_EXTENSION.call_once(|| unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute::<
            *const (),
            unsafe extern "C" fn(
                *mut sqlite3,
                *mut *mut c_char,
                *const sqlite3_api_routines,
            ) -> i32,
        >(sqlite_vec::sqlite3_vec_init as *const ())));
    });
}

pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Connection> {
    register_vec_extension();
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    apply_migrations(&conn)?;
    Ok(conn)
}
