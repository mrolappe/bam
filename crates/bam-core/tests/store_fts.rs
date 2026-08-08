//! P4.6: the FTS5 full-text index over `package.description` and readme
//! text, matching the phase doc's five test bullets exactly.

use std::collections::HashMap;

use bam_core::query::ir::Predicate;
use bam_core::query::registry::FieldRegistry;
use bam_core::store::compile::compile;
use bam_core::store::fts::rebuild_fts;
use bam_core::store::normalize::normalize;
use bam_core::store::tables::{self, LandingIndexLine, LandingReadme, Package};
use rusqlite::Connection;

fn insert_package(conn: &Connection, landing_id: i64, dir: &str, description: Option<&str>) -> i64 {
    tables::insert_package(
        conn,
        &Package {
            id: 0,
            dir: dir.to_string(),
            file: "pkg.lha".to_string(),
            name: "pkg".to_string(),
            version: None,
            size_bytes: None,
            uploaded_on: None,
            date_precision: "exact".to_string(),
            description: description.map(str::to_string),
            landing_id,
        },
    )
    .unwrap()
}

fn insert_readme(conn: &Connection, package_id: i64, url: &str, text: &str) {
    tables::insert_landing_readme(
        conn,
        &LandingReadme {
            id: 0,
            package_id,
            url: url.to_string(),
            fetched_at: "2026-01-01T00:00:00Z".to_string(),
            raw: text.as_bytes().to_vec(),
            detected_encoding: "UTF-8".to_string(),
        },
    )
    .unwrap();
}

fn search_ids(conn: &Connection, term: &str) -> Vec<i64> {
    let reg = FieldRegistry::new(bam_core::query::registry::package_fields());
    let query = compile(&Predicate::FullText(term.to_string()), &reg, None, &HashMap::new()).unwrap();
    let mut stmt = conn.prepare(&query.sql).unwrap();
    let mut ids: Vec<i64> = stmt
        .query_map(rusqlite::params_from_iter(&query.params), |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    ids.sort();
    ids
}

#[test]
fn a_distinctive_word_from_a_readme_finds_exactly_that_package() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing_id = tables::insert_landing_index_line(
        &conn,
        &LandingIndexLine {
            id: 0,
            fetched_at: "2026-01-01T00:00:00Z".into(),
            source_url: "test://fixture".into(),
            line_no: 1,
            raw: vec![],
        },
    )
    .unwrap();

    let a = insert_package(&conn, landing_id, "util/a", Some("a nice utility"));
    let b = insert_package(&conn, landing_id, "util/b", Some("another tool"));
    insert_readme(
        &conn,
        a,
        "util/a/pkg.readme",
        "This tool has a flibbertigibbet mode.",
    );

    rebuild_fts(&conn).unwrap();

    assert_eq!(search_ids(&conn, "flibbertigibbet"), vec![a]);
    let _ = b;
}

#[test]
fn dropping_and_rebuilding_restores_identical_results() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing_id = tables::insert_landing_index_line(
        &conn,
        &LandingIndexLine {
            id: 0,
            fetched_at: "2026-01-01T00:00:00Z".into(),
            source_url: "test://fixture".into(),
            line_no: 1,
            raw: vec![],
        },
    )
    .unwrap();
    let a = insert_package(&conn, landing_id, "util/a", Some("a searchable widget"));
    rebuild_fts(&conn).unwrap();
    let before = search_ids(&conn, "widget");

    conn.execute("DROP TABLE package_fts", []).unwrap();
    rebuild_fts(&conn).unwrap();
    let after = search_ids(&conn, "widget");

    assert_eq!(before, vec![a]);
    assert_eq!(before, after);
}

#[test]
fn bulk_renormalize_followed_by_rebuild_leaves_the_index_consistent() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let raw = b"foo.lha                        util/utils  10K   1 a searchable gizmo\n";
    tables::insert_landing_index_line(
        &conn,
        &LandingIndexLine {
            id: 0,
            fetched_at: "2026-01-01T00:00:00Z".into(),
            source_url: "test://fixture".into(),
            line_no: 1,
            raw: raw.to_vec(),
        },
    )
    .unwrap();

    normalize(&conn).unwrap();
    rebuild_fts(&conn).unwrap();

    let id: i64 = conn
        .query_row("SELECT id FROM package WHERE dir = 'util/utils'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(search_ids(&conn, "gizmo"), vec![id]);

    // A bulk re-normalize resets `package`'s rowids (full DELETE + reinsert)
    // — proving the index still tracks the new ids, not the stale ones.
    normalize(&conn).unwrap();
    rebuild_fts(&conn).unwrap();

    let id_after: i64 = conn
        .query_row("SELECT id FROM package WHERE dir = 'util/utils'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(search_ids(&conn, "gizmo"), vec![id_after]);
}

#[test]
fn fulltext_compiles_to_an_fts5_match_not_like() {
    let reg = FieldRegistry::new(bam_core::query::registry::package_fields());
    let query = compile(
        &Predicate::FullText("tracker module editor".to_string()),
        &reg,
        None,
        &HashMap::new(),
    )
    .unwrap();

    assert!(query.sql.contains("package_fts"));
    assert!(query.sql.contains("MATCH"));
    assert!(!query.sql.contains("LIKE"));
}

#[test]
fn a_term_present_only_in_a_readme_is_found() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing_id = tables::insert_landing_index_line(
        &conn,
        &LandingIndexLine {
            id: 0,
            fetched_at: "2026-01-01T00:00:00Z".into(),
            source_url: "test://fixture".into(),
            line_no: 1,
            raw: vec![],
        },
    )
    .unwrap();

    let a = insert_package(&conn, landing_id, "util/a", Some("plain description"));
    insert_readme(
        &conn,
        a,
        "util/a/pkg.readme",
        "Full instructions for the quibblesnort feature.",
    );

    rebuild_fts(&conn).unwrap();

    assert_eq!(search_ids(&conn, "quibblesnort"), vec![a]);
}
