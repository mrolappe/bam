//! P2.7's six test groups, against [`Session`]'s selection operations
//! (invariant I7) — `mark`/`unmark`/`toggle`/`clear`/`select_by_query`/
//! `save_as`/`load`/`list_selections`/`delete_selection`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::query::ir::{CmpOp, FieldId, Predicate, Value};
use bam_core::store::session::{SelectionMode, Session};
use bam_core::store::tables::{self, LandingIndexLine, Package};

fn temp_db_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "bam-api-selection-test-{label}-{}-{n}.sqlite",
        std::process::id()
    ))
}

/// Seeds `n` packages under `dir`, tagging every other one with `dir_b`
/// instead, so a `dir:` predicate can select a known subset. Returns all
/// ids in insertion order.
fn seed(path: &std::path::Path, n: usize, dir_a: &str, dir_b: &str) -> Vec<i64> {
    let conn = bam_core::store::open(path).unwrap();
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
    (0..n)
        .map(|i| {
            let dir = if i % 2 == 0 { dir_a } else { dir_b };
            tables::insert_package(
                &conn,
                &Package {
                    id: 0,
                    dir: dir.to_string(),
                    file: format!("pkg{i}.lha"),
                    name: format!("pkg{i}"),
                    version: None,
                    size_bytes: Some(1),
                    uploaded_on: Some("2026-01-01".into()),
                    date_precision: "exact".into(),
                    description: None,
                    landing_id,
                },
            )
            .unwrap()
        })
        .collect()
}

fn dir_eq(dir: &str) -> Predicate {
    Predicate::Compare {
        field: FieldId::new("dir"),
        op: CmpOp::Eq,
        value: Value::Text(dir.to_string()),
    }
}

#[test]
fn mark_is_idempotent_and_toggle_twice_returns_to_original_state() {
    let path = temp_db_path("mark");
    let ids = seed(&path, 1, "mods/a", "mods/b");
    let session = Session::open(&path).unwrap();

    session.mark(ids[0]).unwrap();
    session.mark(ids[0]).unwrap();
    assert!(session.is_marked(ids[0]).unwrap());

    let was_marked_before = session.is_marked(ids[0]).unwrap();
    session.toggle(ids[0]).unwrap();
    session.toggle(ids[0]).unwrap();
    assert_eq!(session.is_marked(ids[0]).unwrap(), was_marked_before);
}

#[test]
fn each_selection_mode_produces_the_expected_membership() {
    let path = temp_db_path("modes");
    // ids[0,2,4] in mods/a, ids[1,3] in mods/b.
    let ids = seed(&path, 5, "mods/a", "mods/b");
    let session = Session::open(&path).unwrap();

    // Replace: starts from nothing marked, ends with exactly mods/a.
    session.mark(ids[1]).unwrap();
    let count = session
        .select_by_query(&dir_eq("mods/a"), SelectionMode::Replace)
        .unwrap();
    assert_eq!(count, 3);
    assert!(!session.is_marked(ids[1]).unwrap());
    for &id in &[ids[0], ids[2], ids[4]] {
        assert!(session.is_marked(id).unwrap());
    }

    // Union: mods/b added on top of the existing mods/a working set.
    let count = session
        .select_by_query(&dir_eq("mods/b"), SelectionMode::Union)
        .unwrap();
    assert_eq!(count, 5);

    // Intersect: keep only members that are also in mods/a -> back to 3.
    let count = session
        .select_by_query(&dir_eq("mods/a"), SelectionMode::Intersect)
        .unwrap();
    assert_eq!(count, 3);
    for &id in &[ids[1], ids[3]] {
        assert!(!session.is_marked(id).unwrap());
    }

    // Subtract: remove mods/a from the working set -> empty.
    let count = session
        .select_by_query(&dir_eq("mods/a"), SelectionMode::Subtract)
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn save_as_then_load_in_a_fresh_session_returns_the_same_members() {
    let path = temp_db_path("save-load");
    let ids = seed(&path, 3, "mods/a", "mods/b");

    {
        let session = Session::open(&path).unwrap();
        session.mark(ids[0]).unwrap();
        session.mark(ids[2]).unwrap();
        session.save_as("tracker candidates").unwrap();
    }

    let session = Session::open(&path).unwrap();
    session.load("tracker candidates").unwrap();
    assert!(session.is_marked(ids[0]).unwrap());
    assert!(session.is_marked(ids[2]).unwrap());
    assert!(!session.is_marked(ids[1]).unwrap());

    let _ = std::fs::remove_file(&path);
}

#[test]
fn deleting_a_package_removes_its_membership() {
    let path = temp_db_path("cascade");
    let ids = seed(&path, 2, "mods/a", "mods/b");
    let session = Session::open(&path).unwrap();
    session.mark(ids[0]).unwrap();
    session.save_as("has a deleted member").unwrap();

    let conn = bam_core::store::open(&path).unwrap();
    conn.execute("DELETE FROM package WHERE id = ?1", [ids[0]])
        .unwrap();

    let selections = session.list_selections().unwrap();
    let (_, _, count) = selections
        .into_iter()
        .find(|(name, ..)| name == "has a deleted member")
        .unwrap();
    assert_eq!(
        count, 0,
        "cascade delete must remove the membership row too"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn an_ephemeral_selection_is_cleaned_up_on_session_end_a_named_one_is_not() {
    let path = temp_db_path("cleanup");
    let ids = seed(&path, 1, "mods/a", "mods/b");

    {
        let session = Session::open(&path).unwrap();
        session.mark(ids[0]).unwrap();
        session.save_as("kept").unwrap();
    }
    // The `Session` above is dropped; its working (ephemeral) selection
    // must be gone, but "kept" must survive.

    let conn = bam_core::store::open(&path).unwrap();
    let ephemeral_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM selection WHERE ephemeral = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ephemeral_rows, 0);

    let named_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM selection WHERE name = 'kept'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(named_rows, 1);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn two_sessions_each_with_a_working_selection_do_not_interfere() {
    let path = temp_db_path("two-sessions");
    let ids = seed(&path, 2, "mods/a", "mods/b");

    let a = Session::open(&path).unwrap();
    let b = Session::open(&path).unwrap();

    a.mark(ids[0]).unwrap();
    b.mark(ids[1]).unwrap();

    assert!(a.is_marked(ids[0]).unwrap());
    assert!(!a.is_marked(ids[1]).unwrap());
    assert!(b.is_marked(ids[1]).unwrap());
    assert!(!b.is_marked(ids[0]).unwrap());

    drop(a);
    drop(b);
    let _ = std::fs::remove_file(&path);
}
