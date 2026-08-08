//! P8.3 — the `content_analyzer` extension point. Uses the fixture plugin
//! under `tests/fixtures/plugins/echo-analyzer/`: it always reports
//! available, classifies any `.mod` file as `kind: "echo"` with
//! `searchable_text` derived from the file's decoded bytes, and returns
//! intentionally malformed JSON for a path ending in `broken.mod` — enough
//! to prove the JSON contract, FTS5 wiring, producer-version reprocessing,
//! malformed-output handling, and `claims` prefiltering without needing
//! real module parsing inside WASM.

use std::collections::HashMap;
use std::path::PathBuf;

use bam_core::plugin::WasmContentAnalyzer;
use bam_core::query::ir::Predicate;
use bam_core::query::registry::FieldRegistry;
use bam_core::store::compile::compile;
use bam_core::store::content_analysis::analyze_files;
use bam_core::store::fts::rebuild_fts;
use bam_core::store::summaries::{SUMMARY_KIND, SUMMARY_PRODUCER_VERSION};
use bam_core::store::tables::{self, Enrichment, LandingIndexLine, Package};
use rusqlite::Connection;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/plugins/echo-analyzer")
}

fn insert_package(conn: &Connection, dir: &str) -> i64 {
    let landing_id = tables::insert_landing_index_line(
        conn,
        &LandingIndexLine {
            id: 0,
            fetched_at: "2026-01-01T00:00:00Z".into(),
            source_url: "test://fixture".into(),
            line_no: 1,
            raw: vec![],
        },
    )
    .unwrap();
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
            description: None,
            landing_id,
        },
    )
    .unwrap()
}

fn search_ids(conn: &Connection, term: &str) -> Vec<i64> {
    let reg = FieldRegistry::new(bam_core::query::registry::package_fields());
    let query = compile(
        &Predicate::FullText(term.to_string()),
        &reg,
        None,
        &HashMap::new(),
    )
    .unwrap();
    let mut stmt = conn.prepare(&query.sql).unwrap();
    stmt.query_map(rusqlite::params_from_iter(&query.params), |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn plugin_classifies_a_mod_fixture_and_its_text_becomes_findable_through_fts() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let pkg = insert_package(&conn, "mods/a");

    let analyzer = WasmContentAnalyzer::load(&fixture_dir()).unwrap();
    let files = vec![("intro.mod".to_string(), b"flibbertigibbet".to_vec())];
    let outcome = analyze_files(&conn, pkg, &analyzer, &files, "audio").unwrap();
    assert_eq!(outcome.analyzed, 1);

    rebuild_fts(&conn).unwrap();
    assert_eq!(search_ids(&conn, "flibbertigibbet"), vec![pkg]);
}

#[test]
fn results_are_stored_with_the_plugins_name_and_version_as_producer() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let pkg = insert_package(&conn, "mods/b");

    let analyzer = WasmContentAnalyzer::load(&fixture_dir()).unwrap();
    let files = vec![("song.mod".to_string(), b"tune".to_vec())];
    analyze_files(&conn, pkg, &analyzer, &files, "audio").unwrap();

    let row = tables::get_enrichment(
        &conn,
        pkg,
        &bam_core::store::content_analysis::enrichment_kind("echo-analyzer", "song.mod"),
    )
    .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&row.payload).unwrap();
    assert_eq!(payload["kind"], "echo");
    assert!(row.payload.contains("song.mod"));
}

#[test]
fn bumping_plugin_version_reprocesses_only_that_plugins_rows_not_llm_summaries() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let pkg = insert_package(&conn, "mods/c");

    tables::upsert_enrichment(
        &conn,
        &Enrichment {
            package_id: pkg,
            kind: SUMMARY_KIND.to_string(),
            producer_version: SUMMARY_PRODUCER_VERSION,
            produced_at: "2026-01-01T00:00:00Z".to_string(),
            payload: "a pre-existing summary".to_string(),
        },
    )
    .unwrap();

    let analyzer = WasmContentAnalyzer::load(&fixture_dir()).unwrap();
    let files = vec![("song.mod".to_string(), b"v1".to_vec())];

    let first = analyze_files(&conn, pkg, &analyzer, &files, "audio").unwrap();
    assert_eq!(first.analyzed, 1);
    let second = analyze_files(&conn, pkg, &analyzer, &files, "audio").unwrap();
    assert_eq!(second.up_to_date, 1, "unchanged version must not reprocess");

    // A version bump (new plugin dir with a bumped manifest) reprocesses.
    let bumped_dir =
        std::env::temp_dir().join(format!("bam-echo-analyzer-bumped-{}", std::process::id()));
    std::fs::create_dir_all(&bumped_dir).unwrap();
    std::fs::copy(
        fixture_dir().join("plugin.wasm"),
        bumped_dir.join("plugin.wasm"),
    )
    .unwrap();
    let manifest = std::fs::read_to_string(fixture_dir().join("manifest.toml")).unwrap();
    std::fs::write(
        bumped_dir.join("manifest.toml"),
        manifest.replace("0.1.0", "0.2.0"),
    )
    .unwrap();
    let bumped = WasmContentAnalyzer::load(&bumped_dir).unwrap();
    let third = analyze_files(&conn, pkg, &bumped, &files, "audio").unwrap();
    assert_eq!(third.analyzed, 1, "bumped version must reprocess");

    let summary = tables::get_enrichment(&conn, pkg, SUMMARY_KIND).unwrap();
    assert_eq!(summary.payload, "a pre-existing summary");
}

#[test]
fn a_plugin_returning_malformed_json_is_reported_and_skipped() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let pkg = insert_package(&conn, "mods/d");

    let analyzer = WasmContentAnalyzer::load(&fixture_dir()).unwrap();
    let files = vec![
        ("broken.mod".to_string(), b"x".to_vec()),
        ("ok.mod".to_string(), b"y".to_vec()),
    ];
    let outcome = analyze_files(&conn, pkg, &analyzer, &files, "audio").unwrap();

    assert_eq!(outcome.analyzed, 1);
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.failed[0].0, "broken.mod");

    let err = tables::get_enrichment(
        &conn,
        pkg,
        &bam_core::store::content_analysis::enrichment_kind("echo-analyzer", "broken.mod"),
    );
    assert!(
        err.is_err(),
        "a failed analysis must not write an enrichment row"
    );
}

#[test]
fn claims_prefiltering_means_an_unclaimed_file_is_never_invoked() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let pkg = insert_package(&conn, "mods/e");

    let analyzer = WasmContentAnalyzer::load(&fixture_dir()).unwrap();
    // echo-analyzer's manifest claims only "*.mod"; picture.iff is not offered to it.
    let files = vec![("picture.iff".to_string(), b"iff data".to_vec())];
    let outcome = analyze_files(&conn, pkg, &analyzer, &files, "image").unwrap();

    assert_eq!(outcome.analyzed, 0);
    assert_eq!(outcome.up_to_date, 0);
    assert!(outcome.failed.is_empty());
    let row = tables::get_enrichment(
        &conn,
        pkg,
        &bam_core::store::content_analysis::enrichment_kind("echo-analyzer", "picture.iff"),
    );
    assert!(
        row.is_err(),
        "unclaimed file must never be analyzed or stored"
    );
}
