use bam_core::store::{self, tables::*};
use rusqlite::Connection;

fn fresh_db() -> Connection {
    store::open(":memory:").unwrap()
}

fn insert_landing(conn: &Connection, raw: &[u8]) -> i64 {
    insert_landing_index_line(
        conn,
        &LandingIndexLine {
            id: 0,
            fetched_at: "2026-08-06T00:00:00Z".into(),
            source_url: "https://ftp.fau.de/aminet/INDEX".into(),
            line_no: 1,
            raw: raw.to_vec(),
        },
    )
    .unwrap()
}

fn insert_pkg(conn: &Connection, dir: &str, file: &str, landing_id: i64) -> i64 {
    insert_package(
        conn,
        &Package {
            id: 0,
            dir: dir.into(),
            file: file.into(),
            name: "Foo".into(),
            version: Some("1.2".into()),
            size_bytes: Some(1024),
            uploaded_on: Some("2026-08-01".into()),
            date_precision: "week".into(),
            description: Some("A test package".into()),
            landing_id,
        },
    )
    .unwrap()
}

#[test]
fn round_trip_all_tables() {
    let conn = fresh_db();

    let landing_id = insert_landing(&conn, b"util/misc  Foo-1.2.lha  10K  Foo utility");
    let landing = get_landing_index_line(&conn, landing_id).unwrap();
    assert_eq!(landing.id, landing_id);
    assert_eq!(landing.raw, b"util/misc  Foo-1.2.lha  10K  Foo utility");

    let package_id = insert_pkg(&conn, "util/misc", "Foo-1.2.lha", landing_id);
    let package = get_package(&conn, package_id).unwrap();
    assert_eq!(package.id, package_id);
    assert_eq!(package.dir, "util/misc");
    assert_eq!(package.landing_id, landing_id);

    insert_enrichment(
        &conn,
        &Enrichment {
            package_id,
            kind: "readme_header".into(),
            producer_version: 1,
            produced_at: "2026-08-06T00:00:00Z".into(),
            payload: "{}".into(),
        },
    )
    .unwrap();
    let enrichment = get_enrichment(&conn, package_id, "readme_header").unwrap();
    assert_eq!(enrichment.package_id, package_id);
    assert_eq!(enrichment.payload, "{}");

    let selection_id = insert_selection(
        &conn,
        &Selection {
            id: 0,
            name: Some("my-picks".into()),
            created_at: "2026-08-06T00:00:00Z".into(),
            ephemeral: false,
        },
    )
    .unwrap();
    let selection = get_selection(&conn, selection_id).unwrap();
    assert_eq!(selection.id, selection_id);
    assert_eq!(selection.name.as_deref(), Some("my-picks"));

    insert_selection_member(
        &conn,
        &SelectionMember {
            selection_id,
            package_id,
        },
    )
    .unwrap();
    let member = get_selection_member(&conn, selection_id, package_id).unwrap();
    assert_eq!(member.selection_id, selection_id);
    assert_eq!(member.package_id, package_id);
}

#[test]
fn unique_dir_file_rejects_duplicate() {
    let conn = fresh_db();
    let landing_id = insert_landing(&conn, b"line one");

    insert_pkg(&conn, "util/misc", "Foo-1.2.lha", landing_id);
    let result = insert_package(
        &conn,
        &Package {
            id: 0,
            dir: "util/misc".into(),
            file: "Foo-1.2.lha".into(),
            name: "Foo".into(),
            version: None,
            size_bytes: None,
            uploaded_on: None,
            date_precision: "week".into(),
            description: None,
            landing_id,
        },
    );

    assert!(result.is_err());
}

#[test]
fn cascade_delete_package() {
    let conn = fresh_db();
    let landing_id = insert_landing(&conn, b"line one");
    let package_id = insert_pkg(&conn, "util/misc", "Foo-1.2.lha", landing_id);

    insert_enrichment(
        &conn,
        &Enrichment {
            package_id,
            kind: "readme_header".into(),
            producer_version: 1,
            produced_at: "2026-08-06T00:00:00Z".into(),
            payload: "{}".into(),
        },
    )
    .unwrap();
    let selection_id = insert_selection(
        &conn,
        &Selection {
            id: 0,
            name: None,
            created_at: "2026-08-06T00:00:00Z".into(),
            ephemeral: true,
        },
    )
    .unwrap();
    insert_selection_member(
        &conn,
        &SelectionMember {
            selection_id,
            package_id,
        },
    )
    .unwrap();

    conn.execute("DELETE FROM package WHERE id = ?1", [package_id])
        .unwrap();

    assert!(get_enrichment(&conn, package_id, "readme_header").is_err());
    assert!(get_selection_member(&conn, selection_id, package_id).is_err());
}

#[test]
fn drop_recreate_package_leaves_landing_untouched() {
    let conn = fresh_db();
    let landing_id = insert_landing(&conn, b"line one");
    insert_pkg(&conn, "util/misc", "Foo-1.2.lha", landing_id);

    conn.execute("DROP TABLE package", []).unwrap();
    conn.execute_batch(
        "CREATE TABLE package (
           id             INTEGER PRIMARY KEY,
           dir            TEXT NOT NULL,
           file           TEXT NOT NULL,
           name           TEXT NOT NULL,
           version        TEXT,
           size_bytes     INTEGER,
           uploaded_on    TEXT,
           date_precision TEXT NOT NULL,
           description    TEXT,
           landing_id     INTEGER NOT NULL REFERENCES landing_index_line(id),
           UNIQUE(dir, file)
         )",
    )
    .unwrap();

    let landing = get_landing_index_line(&conn, landing_id).unwrap();
    assert_eq!(landing.raw, b"line one");

    let package_id = insert_pkg(&conn, "util/misc", "Foo-1.2.lha", landing_id);
    assert_eq!(
        get_package(&conn, package_id).unwrap().landing_id,
        landing_id
    );
}

#[test]
fn blob_roundtrips_invalid_utf8() {
    let conn = fresh_db();
    let invalid_utf8 = [0xFFu8, 0xFE, 0x00, 0x80, b'F', b'o', b'o'];

    let landing_id = insert_landing(&conn, &invalid_utf8);
    let landing = get_landing_index_line(&conn, landing_id).unwrap();

    assert_eq!(landing.raw, invalid_utf8);
}
