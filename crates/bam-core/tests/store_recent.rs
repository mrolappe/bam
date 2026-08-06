//! P1.8 — RECENT-based incremental update: upsert-by-`(dir, file)` on top of
//! P1.4's parser and P1.6's normalizer, preserving existing `package.id` and
//! reporting exactly what changed.

use bam_core::store::normalize::normalize;
use bam_core::store::recent::upsert_recent;
use bam_core::store::{self, land::land_lines};
use rusqlite::Connection;

const INDEX_URL: &str = "https://ftp.fau.de/aminet/INDEX";
const RECENT_URL: &str = "https://ftp.fau.de/aminet/RECENT";
const FETCHED_AT: &str = "2026-08-06T00:00:00Z";

fn read_fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(path).unwrap()
}

fn seed_from_index(conn: &Connection) {
    land_lines(
        conn,
        INDEX_URL,
        FETCHED_AT,
        &read_fixture("index_sample.txt"),
    )
    .unwrap();
    normalize(conn).unwrap();
}

fn package_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM package", [], |row| row.get(0))
        .unwrap()
}

fn package_id(conn: &Connection, dir: &str, file: &str) -> i64 {
    conn.query_row(
        "SELECT id FROM package WHERE dir = ?1 AND file = ?2",
        [dir, file],
        |row| row.get(0),
    )
    .unwrap()
}

// Located by content, not line number (Round 3's convention).
fn find_line(fixture: &str, needle: &str) -> Vec<u8> {
    read_fixture(fixture)
        .split(|&b| b == b'\n')
        .find(|line| line.windows(needle.len()).any(|w| w == needle.as_bytes()))
        .expect("fixture line present")
        .to_vec()
}

#[test]
fn recent_after_index_adds_only_new_rows() {
    let conn = store::open(":memory:").unwrap();
    seed_from_index(&conn);
    let before = package_count(&conn);

    let recent_body = read_fixture("recent_sample.txt");
    let changed = upsert_recent(&conn, RECENT_URL, FETCHED_AT, &recent_body).unwrap();

    // The fixtures don't overlap (confirmed 2026-08-06): every RECENT line is
    // genuinely new, so every one of them lands as an insert.
    let after = package_count(&conn);
    assert_eq!(after, before + changed.len() as i64);
    assert!(!changed.is_empty());
}

#[test]
fn existing_rows_untouched_including_id() {
    let conn = store::open(":memory:").unwrap();
    seed_from_index(&conn);

    let recent_body = read_fixture("recent_sample.txt");
    let first_run = upsert_recent(&conn, RECENT_URL, FETCHED_AT, &recent_body).unwrap();
    let ids_after_first: Vec<i64> = first_run
        .iter()
        .map(|c| package_id(&conn, &c.dir, &c.file))
        .collect();
    let count_after_first = package_count(&conn);

    // Re-running the identical RECENT body must be a total no-op: nothing
    // reported as changed, same row count, same ids for the rows it already
    // added — none of them get rewritten just because they're re-listed.
    let second_run = upsert_recent(&conn, RECENT_URL, FETCHED_AT, &recent_body).unwrap();
    assert!(second_run.is_empty());
    assert_eq!(package_count(&conn), count_after_first);
    let ids_after_second: Vec<i64> = first_run
        .iter()
        .map(|c| package_id(&conn, &c.dir, &c.file))
        .collect();
    assert_eq!(ids_after_first, ids_after_second);
}

#[test]
fn changed_list_matches_exactly_what_was_added_or_updated() {
    let conn = store::open(":memory:").unwrap();
    seed_from_index(&conn);

    let unchanged_line = find_line("index_sample.txt", "A2KDeck.lha");
    let id_before = package_id(&conn, "biz/dbase", "A2KDeck.lha");

    // Replace the size token ("671K") with a different value — the parser is
    // token-position based, so this doesn't disturb column alignment.
    let updated_line = String::from_utf8(unchanged_line.clone())
        .unwrap()
        .replacen("671K", "999K", 1)
        .into_bytes();

    let new_line = find_line("recent_sample.txt", "Weather13.lha"); // absent from INDEX

    let mut body = Vec::new();
    body.extend_from_slice(&unchanged_line);
    body.push(b'\n');
    body.extend_from_slice(&updated_line);
    body.push(b'\n');
    body.extend_from_slice(&new_line);
    body.push(b'\n');

    // `updated_line` re-lists A2KDeck.lha with a different size, and reuses
    // the same (dir, file) as the unchanged line right above it in the same
    // body — upsert_recent must still end up applying the *last* processed
    // state for that key, which is the updated one.
    let changed = upsert_recent(&conn, RECENT_URL, FETCHED_AT, &body).unwrap();

    let changed_files: std::collections::HashSet<_> =
        changed.iter().map(|c| c.file.as_str()).collect();
    assert_eq!(
        changed_files,
        ["A2KDeck.lha", "Weather13.lha"].into_iter().collect()
    );

    // id preserved across the update, only the size changed.
    let id_after = package_id(&conn, "biz/dbase", "A2KDeck.lha");
    assert_eq!(id_before, id_after);
    let size_bytes: i64 = conn
        .query_row(
            "SELECT size_bytes FROM package WHERE id = ?1",
            [id_after],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(size_bytes, 999 * 1024);
}
