//! P2.5: the IR → SQL compiler's six test groups, against an in-memory
//! Phase 1 schema. `docs/lang-bam-dsl.md`'s worked examples are reused
//! verbatim for the first group.

use bam_core::query::bam_dsl::BamDsl;
use bam_core::query::ir::{FieldId, Pattern, Predicate};
use bam_core::query::lang::QueryLanguage;
use bam_core::query::registry::{FieldRegistry, package_fields};
use bam_core::store::compile::{CompileError, compile};
use bam_core::store::tables::{self, LandingIndexLine, Package, Selection, SelectionMember};
use rusqlite::Connection;

struct Fixture {
    conn: Connection,
    marked: i64,
    ids: [i64; 9],
}

/// Nine packages, a named selection `"tracker candidates"` (ids[1], ids[7]),
/// and an ephemeral "marked" selection (ids[0], ids[5], ids[6]) — enough
/// spread across `dir`/`size`/`date`/`version`/`description` to give each of
/// the fourteen executable worked examples a distinct, hand-checkable
/// result set. Row 8 (`ids[8]`) exists only to prove `GLOB` is
/// case-sensitive: its `dir` would match `util/*` under a case-insensitive
/// match but must not under `GLOB`.
fn fixture() -> Fixture {
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

    let row = |dir: &str,
               file: &str,
               name: &str,
               version: Option<&str>,
               size_bytes: Option<i64>,
               uploaded_on: Option<&str>,
               description: Option<&str>|
     -> i64 {
        tables::insert_package(
            &conn,
            &Package {
                id: 0,
                dir: dir.to_string(),
                file: file.to_string(),
                name: name.to_string(),
                version: version.map(str::to_string),
                size_bytes,
                uploaded_on: uploaded_on.map(str::to_string),
                date_precision: "exact".to_string(),
                description: description.map(str::to_string),
                landing_id,
            },
        )
        .unwrap()
    };

    let ids = [
        row(
            "util/utils",
            "foo.lha",
            "foo",
            None,
            Some(200_000),
            Some("2005-06-01"),
            Some("a nice utility"),
        ),
        row(
            "mus/mod",
            "bar.lha",
            "bar",
            Some("1.2"),
            Some(50_000),
            Some("1999-01-01"),
            Some("tracker module editor"),
        ),
        row(
            "mus/cla",
            "baz.lha",
            "baz",
            Some("1.0"),
            Some(5_000),
            Some("2001-05-01"),
            None,
        ),
        row(
            "util/mod",
            "Deluxe1.lha",
            "Deluxe1",
            None,
            Some(10_000),
            Some("2010-01-01"),
            Some("demo pack"),
        ),
        row(
            "util/mod",
            "Deluxe2.lha",
            "Deluxe",
            Some("1.2"),
            Some(10_000),
            Some("2010-01-01"),
            None,
        ),
        row(
            "game/adv",
            "quest.lha",
            "quest",
            Some("2.0"),
            Some(300_000),
            Some("2021-03-01"),
            None,
        ),
        row(
            "pix/art",
            "pic.lha",
            "pic",
            Some("1.0"),
            Some(2_000),
            Some("1994-01-01"),
            None,
        ),
        row(
            "mus/smp",
            "sample.lha",
            "sample",
            None,
            Some(1_000),
            Some("2000-01-01"),
            Some("demo"),
        ),
        row(
            "UTIL/mod",
            "upper.lha",
            "upper",
            None,
            Some(500),
            Some("2010-01-01"),
            None,
        ),
    ];

    let named = tables::insert_selection(
        &conn,
        &Selection {
            id: 0,
            name: Some("tracker candidates".into()),
            created_at: "2026-01-01T00:00:00Z".into(),
            ephemeral: false,
        },
    )
    .unwrap();
    for pid in [ids[1], ids[7]] {
        tables::insert_selection_member(
            &conn,
            &SelectionMember {
                selection_id: named,
                package_id: pid,
            },
        )
        .unwrap();
    }

    let marked = tables::insert_selection(
        &conn,
        &Selection {
            id: 0,
            name: None,
            created_at: "2026-01-01T00:00:00Z".into(),
            ephemeral: true,
        },
    )
    .unwrap();
    for pid in [ids[0], ids[5], ids[6]] {
        tables::insert_selection_member(
            &conn,
            &SelectionMember {
                selection_id: marked,
                package_id: pid,
            },
        )
        .unwrap();
    }

    Fixture { conn, marked, ids }
}

fn run(conn: &Connection, reg: &FieldRegistry, pred: &Predicate, marked: Option<i64>) -> Vec<i64> {
    let compiled = compile(pred, reg, marked).unwrap_or_else(|e| panic!("compile failed: {e}"));
    let mut stmt = conn
        .prepare(&compiled.sql)
        .unwrap_or_else(|e| panic!("bad SQL {:?}: {e}", compiled.sql));
    let mut ids: Vec<i64> = stmt
        .query_map(rusqlite::params_from_iter(compiled.params.iter()), |r| {
            r.get(0)
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    ids.sort_unstable();
    ids
}

#[test]
fn worked_examples_compile_and_return_expected_ids() {
    let f = fixture();
    let reg = FieldRegistry::new(package_fields());
    let lang = BamDsl;
    let [a, b, c, d, e, g, h, i, j] = f.ids;

    // (source, expected ids). Example 9 (`similar:...`) is compile-rejected
    // by design (P2.1/P7.4) — covered by `similar_is_rejected` below instead.
    // `j` (row 9, `UTIL/mod`, uploaded 2010) exists to prove GLOB is
    // case-sensitive (`glob_is_case_sensitive`, separately) — it's still a
    // real row here, so it shows up in any case that doesn't test `dir`.
    let cases: Vec<(&str, Vec<i64>)> = vec![
        ("dir:util/*", vec![a, d, e]),
        ("size>100k", vec![a, g]),
        ("year>2000", vec![a, c, d, e, g, j]),
        ("dir:util/* !name:mod OR year>2000", vec![a, c, d, e, g, j]),
        ("tracker module editor", vec![b]),
        ("name:Deluxe* version:1.2", vec![e]),
        ("in:'tracker candidates'", vec![b, i]),
        ("marked !size<10k", vec![a, g]),
        ("dir:mus/* (year<1995 OR year>2000)", vec![c]),
        ("!(dir:mus/* OR dir:cla/*)", vec![a, d, e, g, h, j]),
        ("date>2020-01-01", vec![g]),
        ("description:~'demo'", vec![d, i]),
        ("version!=1.0", vec![b, e, g]),
        ("file:*.lha", {
            let mut all = f.ids.to_vec();
            all.sort_unstable();
            all
        }),
    ];

    for (src, mut expected) in cases {
        expected.sort_unstable();
        let pred = lang
            .parse(src, &reg)
            .unwrap_or_else(|e| panic!("{src}: {e}"));
        let got = run(&f.conn, &reg, &pred, Some(f.marked));
        assert_eq!(got, expected, "source: {src}");
    }
}

#[test]
fn literal_containing_sql_is_bound_not_interpolated() {
    let f = fixture();
    let reg = FieldRegistry::new(package_fields());
    let malicious = "'; DROP TABLE package; --";
    let pred = Predicate::Compare {
        field: FieldId::new("name"),
        op: bam_core::query::ir::CmpOp::Eq,
        value: bam_core::query::ir::Value::Text(malicious.to_string()),
    };
    let compiled = compile(&pred, &reg, None).unwrap();
    assert!(
        !compiled.sql.contains("DROP TABLE"),
        "the literal must not reach the SQL text: {}",
        compiled.sql
    );
    assert_eq!(
        compiled.params,
        vec![rusqlite::types::Value::Text(malicious.to_string())]
    );

    let got = run(&f.conn, &reg, &pred, None);
    assert!(got.is_empty());

    let count: i64 = f
        .conn
        .query_row("SELECT count(*) FROM package", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 9, "package table must still exist and hold all rows");
}

#[test]
fn glob_is_case_sensitive() {
    let f = fixture();
    let reg = FieldRegistry::new(package_fields());
    let pred = Predicate::Match {
        field: FieldId::new("dir"),
        pattern: Pattern::Glob("util/*".into()),
    };
    let got = run(&f.conn, &reg, &pred, None);
    assert!(
        !got.contains(&f.ids[8]),
        "GLOB must not case-fold 'UTIL/mod' into 'util/*'"
    );
}

#[test]
fn year_compare_excludes_week_precision_rows_straddling_the_boundary() {
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
    let row = |file: &str, uploaded_on: &str, date_precision: &str| -> i64 {
        tables::insert_package(
            &conn,
            &Package {
                id: 0,
                dir: "d".into(),
                file: file.into(),
                name: file.into(),
                version: None,
                size_bytes: None,
                uploaded_on: Some(uploaded_on.to_string()),
                date_precision: date_precision.to_string(),
                description: None,
                landing_id,
            },
        )
        .unwrap()
    };

    // ±7-day window straddles the 2000/2001 boundary: true year unknowable.
    let straddling = row("straddle.lha", "2001-01-02", "week");
    // Window sits entirely inside 2005: safe to assert year>2000.
    let safe_week = row("safe.lha", "2005-06-15", "week");
    // Exact precision at the same knife's-edge date: no fuzz applied.
    let exact_edge = row("exact.lha", "2001-01-01", "exact");

    let reg = FieldRegistry::new(package_fields());
    let lang = BamDsl;
    let pred = lang.parse("year>2000", &reg).unwrap();
    let got = run(&conn, &reg, &pred, None);

    assert!(
        !got.contains(&straddling),
        "a week-precision row whose window straddles the boundary must be excluded"
    );
    assert!(got.contains(&safe_week));
    assert!(got.contains(&exact_edge));
}

#[test]
fn nested_not_or_parenthesizes_correctly() {
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
    let row = |dir: &str| -> i64 {
        tables::insert_package(
            &conn,
            &Package {
                id: 0,
                dir: dir.into(),
                file: format!("{dir}.lha").replace('/', "_"),
                name: dir.into(),
                version: None,
                size_bytes: None,
                uploaded_on: None,
                date_precision: "exact".into(),
                description: None,
                landing_id,
            },
        )
        .unwrap()
    };
    let a_only = row("a/1");
    let b_only = row("b/1");
    let neither = row("c/1");

    let reg = FieldRegistry::new(package_fields());
    let a = Predicate::Match {
        field: FieldId::new("dir"),
        pattern: Pattern::Glob("a/*".into()),
    };
    let b = Predicate::Match {
        field: FieldId::new("dir"),
        pattern: Pattern::Glob("b/*".into()),
    };

    // `!(a OR b)`
    let not_or = Predicate::Not(Box::new(Predicate::Or(vec![a.clone(), b.clone()])));
    let got_not_or = run(&conn, &reg, &not_or, None);
    assert_eq!(got_not_or, vec![neither]);

    // `!a OR b` — a different tree, and must give a different result.
    let wrong = Predicate::Or(vec![Predicate::Not(Box::new(a)), b]);
    let mut got_wrong = run(&conn, &reg, &wrong, None);
    got_wrong.sort_unstable();
    let mut expected_wrong = vec![b_only, neither];
    expected_wrong.sort_unstable();
    assert_eq!(got_wrong, expected_wrong);

    assert_ne!(
        got_not_or, got_wrong,
        "!(a OR b) must not compile the same as !a OR b"
    );
    let _ = a_only; // only relevant as the row that both queries correctly exclude/include differently
}

#[test]
fn similar_is_rejected_as_not_yet_supported() {
    let reg = FieldRegistry::new(package_fields());
    let pred = Predicate::Similar {
        text: "tracker module editor".into(),
        threshold: 0.82,
    };
    let err = compile(&pred, &reg, None).unwrap_err();
    assert!(
        matches!(err, CompileError::SimilarNotSupported),
        "got: {err:?}"
    );
}
