use bam_core::store::{self, tables::*};
use rusqlite::Connection;

fn fresh_db() -> Connection {
    store::open(":memory:").unwrap()
}

fn insert_pkg(conn: &Connection) -> i64 {
    let landing_id = insert_landing_index_line(
        conn,
        &LandingIndexLine {
            id: 0,
            fetched_at: "2026-08-07T00:00:00Z".into(),
            source_url: "https://ftp.fau.de/aminet/INDEX".into(),
            line_no: 1,
            raw: b"util/misc  Foo-1.2.lha  10K  Foo utility".to_vec(),
        },
    )
    .unwrap();
    insert_package(
        conn,
        &Package {
            id: 0,
            dir: "util/misc".into(),
            file: "Foo-1.2.lha".into(),
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
fn readme_round_trips_exact_bytes() {
    let conn = fresh_db();
    let package_id = insert_pkg(&conn);
    let raw = b"Short: Foo\nAuthor: Someone\n\xffinvalid utf8".to_vec();

    let id = insert_landing_readme(
        &conn,
        &LandingReadme {
            id: 0,
            package_id,
            url: "https://ftp.fau.de/aminet/util/misc/Foo-1.2.readme".into(),
            fetched_at: "2026-08-07T00:00:00Z".into(),
            raw: raw.clone(),
            detected_encoding: "windows-1252".into(),
        },
    )
    .unwrap();

    let readme =
        get_landing_readme(&conn, "https://ftp.fau.de/aminet/util/misc/Foo-1.2.readme").unwrap();
    assert_eq!(readme.id, id);
    assert_eq!(readme.package_id, package_id);
    assert_eq!(readme.raw, raw);
}

#[test]
fn detected_encoding_is_stored_and_readable() {
    let conn = fresh_db();
    let package_id = insert_pkg(&conn);

    insert_landing_readme(
        &conn,
        &LandingReadme {
            id: 0,
            package_id,
            url: "https://ftp.fau.de/aminet/util/misc/Foo-1.2.readme".into(),
            fetched_at: "2026-08-07T00:00:00Z".into(),
            raw: b"Short: Foo".to_vec(),
            detected_encoding: "UTF-8".into(),
        },
    )
    .unwrap();

    let readme =
        get_landing_readme(&conn, "https://ftp.fau.de/aminet/util/misc/Foo-1.2.readme").unwrap();
    assert_eq!(readme.detected_encoding, "UTF-8");
}

#[test]
fn refetching_the_same_url_updates_rather_than_duplicates() {
    let conn = fresh_db();
    let package_id = insert_pkg(&conn);
    let url = "https://ftp.fau.de/aminet/util/misc/Foo-1.2.readme";

    let first_id = insert_landing_readme(
        &conn,
        &LandingReadme {
            id: 0,
            package_id,
            url: url.into(),
            fetched_at: "2026-08-07T00:00:00Z".into(),
            raw: b"old contents".to_vec(),
            detected_encoding: "UTF-8".into(),
        },
    )
    .unwrap();

    let second_id = insert_landing_readme(
        &conn,
        &LandingReadme {
            id: 0,
            package_id,
            url: url.into(),
            fetched_at: "2026-08-07T01:00:00Z".into(),
            raw: b"new contents".to_vec(),
            detected_encoding: "UTF-8".into(),
        },
    )
    .unwrap();

    assert_eq!(first_id, second_id);

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM landing_readme WHERE url = ?1",
            [url],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);

    let readme = get_landing_readme(&conn, url).unwrap();
    assert_eq!(readme.raw, b"new contents");
    assert_eq!(readme.fetched_at, "2026-08-07T01:00:00Z");
}
