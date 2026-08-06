//! P3.8's six test bullets: a rule omitting `lang` uses the configured
//! default, a rule naming an explicit language uses it, a rule naming an
//! unregistered language errors (naming it), a rule whose `when` fails to
//! compile is reported and skipped without disabling the others, editing
//! the file while running updates highlighting without a restart, and the
//! watcher's debounce coalesces two rapid writes into one reload.
//!
//! The first four drive a real [`Session`]/[`SessionStore`] — genuine
//! parser/registry/compiler wiring, same convention as
//! `tui_selection.rs`'s save/load round trip — since what's under test is
//! whether `bam_core`'s language registry actually behaves as configured,
//! not a fake's own hardcoded logic. The last two need deterministic call
//! counting instead, so they use a `FakeStore`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bam_core::api::Session;
use bam_core::query::ir::Predicate;
use bam_core::query::lang::ParseError;
use bam_core::store::tables::{self, LandingIndexLine, Package};
use bam_tui::app::{App, all_packages};
use bam_tui::store::{PackageStore, SessionStore, StoreError, WindowResult};

fn temp_path(label: &str, ext: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "bam-tui-highlight-test-{label}-{}-{n}.{ext}",
        std::process::id()
    ))
}

/// Seeds one package (`games/action/pkg0.lha`) so a rule matching
/// `dir:games/*` has a real row to hit.
fn seeded_session(db_path: &std::path::Path) -> Session {
    let conn = bam_core::store::open(db_path).unwrap();
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
    tables::insert_package(
        &conn,
        &Package {
            id: 0,
            dir: "games/action".into(),
            file: "pkg0.lha".into(),
            name: "pkg0".into(),
            version: None,
            size_bytes: Some(1),
            uploaded_on: Some("2026-01-01".into()),
            date_precision: "exact".into(),
            description: None,
            landing_id,
        },
    )
    .unwrap();
    Session::open(db_path).unwrap()
}

#[test]
fn rule_omitting_lang_uses_the_configured_default() {
    let db_path = temp_path("default-lang", "sqlite");
    let cfg_path = temp_path("default-lang", "toml");
    std::fs::write(
        &cfg_path,
        r#"
[[highlight]]
name = "action games"
when = "dir:games/*"
gutter = "g"
priority = 1
"#,
    )
    .unwrap();

    let session = seeded_session(&db_path);
    let mut app = App::new(SessionStore::new(session), all_packages(), 5).unwrap();
    app.set_highlight_config(&cfg_path).unwrap();

    assert!(app.highlight_errors().is_empty());
    assert_eq!(app.row_tokens(0).gutters, vec!["g".to_string()]);

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(&cfg_path);
}

#[test]
fn rule_naming_an_explicit_language_uses_it() {
    let db_path = temp_path("explicit-lang", "sqlite");
    let cfg_path = temp_path("explicit-lang", "toml");
    std::fs::write(
        &cfg_path,
        r#"
[[highlight]]
name = "action games"
lang = "bam-dsl"
when = "dir:games/*"
gutter = "g"
priority = 1
"#,
    )
    .unwrap();

    let session = seeded_session(&db_path);
    let mut app = App::new(SessionStore::new(session), all_packages(), 5).unwrap();
    app.set_highlight_config(&cfg_path).unwrap();

    assert!(app.highlight_errors().is_empty());
    assert_eq!(app.row_tokens(0).gutters, vec!["g".to_string()]);

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(&cfg_path);
}

#[test]
fn rule_naming_an_unregistered_language_errors_naming_it() {
    let db_path = temp_path("bad-lang", "sqlite");
    let cfg_path = temp_path("bad-lang", "toml");
    std::fs::write(
        &cfg_path,
        r#"
[[highlight]]
name = "action games"
lang = "sql"
when = "dir:games/*"
gutter = "g"
priority = 1
"#,
    )
    .unwrap();

    let session = seeded_session(&db_path);
    let mut app = App::new(SessionStore::new(session), all_packages(), 5).unwrap();
    app.set_highlight_config(&cfg_path).unwrap();

    assert_eq!(app.highlight_errors().len(), 1);
    let err = &app.highlight_errors()[0];
    assert!(err.contains("action games"), "must name the rule: {err}");
    assert!(
        err.contains("sql"),
        "must name the unregistered language: {err}"
    );
    // A rule that failed to resolve contributes no decoration.
    assert!(app.row_tokens(0).gutters.is_empty());

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(&cfg_path);
}

#[test]
fn a_rule_that_fails_to_compile_is_reported_and_skipped_not_fatal() {
    let db_path = temp_path("bad-when", "sqlite");
    let cfg_path = temp_path("bad-when", "toml");
    std::fs::write(
        &cfg_path,
        r#"
[[highlight]]
name = "broken"
when = "size:~'foo'"

[[highlight]]
name = "action games"
when = "dir:games/*"
gutter = "g"
priority = 1
"#,
    )
    .unwrap();

    let session = seeded_session(&db_path);
    let mut app = App::new(SessionStore::new(session), all_packages(), 5).unwrap();
    app.set_highlight_config(&cfg_path).unwrap();

    assert_eq!(app.highlight_errors().len(), 1);
    assert!(app.highlight_errors()[0].contains("broken"));
    // The other rule still evaluates — one bad rule doesn't take down the
    // app or disable the rest.
    assert_eq!(app.row_tokens(0).gutters, vec!["g".to_string()]);

    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(&cfg_path);
}

/// Echoes `when` back as a `FullText` predicate and records every
/// `parse_lang` call — deterministic and fast, so the hot-reload and
/// debounce tests can control timing exactly via synthetic `Instant`s
/// rather than real file-system event timing.
struct FakeStore {
    total: usize,
    parse_calls: Rc<RefCell<usize>>,
}

impl PackageStore for FakeStore {
    fn window(
        &mut self,
        _pred: &Predicate,
        offset: usize,
        limit: usize,
    ) -> Result<WindowResult, StoreError> {
        let end = (offset + limit).min(self.total);
        Ok(WindowResult {
            packages: (offset..end)
                .map(|i| Package {
                    id: i as i64,
                    dir: "games/action".into(),
                    file: format!("pkg{i}.lha"),
                    name: format!("pkg{i}"),
                    version: None,
                    size_bytes: Some(1),
                    uploaded_on: Some("2026-01-01".into()),
                    date_precision: "exact".into(),
                    description: None,
                    landing_id: 0,
                })
                .collect(),
            total: self.total,
        })
    }

    fn parse(&self, src: &str) -> Result<Predicate, ParseError> {
        Ok(Predicate::FullText(src.to_string()))
    }

    fn parse_lang(&self, _lang: Option<&str>, src: &str) -> Result<Predicate, StoreError> {
        *self.parse_calls.borrow_mut() += 1;
        Ok(Predicate::FullText(src.to_string()))
    }

    fn matching_ids(&self, _pred: &Predicate, ids: &[i64]) -> Result<Vec<i64>, StoreError> {
        Ok(ids.to_vec())
    }

    fn toggle(&self, _package_id: i64) -> Result<bool, StoreError> {
        unimplemented!("not exercised by this test")
    }
    fn is_marked(&self, _package_id: i64) -> Result<bool, StoreError> {
        Ok(false)
    }
    fn mark(&self, _package_id: i64) -> Result<(), StoreError> {
        unimplemented!("not exercised by this test")
    }
    fn select_by_query(
        &self,
        _pred: &Predicate,
        _mode: bam_core::api::SelectionMode,
    ) -> Result<usize, StoreError> {
        unimplemented!("not exercised by this test")
    }
    fn save_as(&self, _name: &str) -> Result<(), StoreError> {
        unimplemented!("not exercised by this test")
    }
    fn load(&self, _name: &str) -> Result<(), StoreError> {
        unimplemented!("not exercised by this test")
    }
    fn list_selections(&self) -> Result<Vec<bam_core::api::SelectionSummary>, StoreError> {
        unimplemented!("not exercised by this test")
    }
}

fn write_one_rule(path: &std::path::Path, name: &str, gutter: &str) {
    std::fs::write(
        path,
        format!(
            r#"
[[highlight]]
name = "{name}"
when = "dir:games/*"
gutter = "{gutter}"
priority = 1
"#
        ),
    )
    .unwrap();
}

#[test]
fn editing_the_file_while_running_updates_highlighting_without_a_restart() {
    let cfg_path = temp_path("hot-reload", "toml");
    write_one_rule(&cfg_path, "first", "g1");

    let calls = Rc::new(RefCell::new(0));
    let store = FakeStore {
        total: 3,
        parse_calls: calls.clone(),
    };
    let mut app = App::new(store, all_packages(), 3).unwrap();
    app.set_highlight_config(&cfg_path).unwrap();
    assert_eq!(app.row_tokens(0).gutters, vec!["g1".to_string()]);

    // Edit the file "while running" — no restart, just further `tick`s.
    write_one_rule(&cfg_path, "second", "g2");
    let t0 = Instant::now();
    // First tick observes the change and starts the debounce timer; it must
    // not have reloaded yet.
    app.tick(t0).unwrap();
    assert_eq!(app.row_tokens(0).gutters, vec!["g1".to_string()]);
    // Once the debounce settles, the new content takes effect.
    app.tick(t0 + Duration::from_millis(400)).unwrap();
    assert_eq!(app.row_tokens(0).gutters, vec!["g2".to_string()]);

    let _ = std::fs::remove_file(&cfg_path);
}

#[test]
fn the_watcher_debounces_two_rapid_writes_into_one_reload() {
    let cfg_path = temp_path("debounce", "toml");
    write_one_rule(&cfg_path, "first", "g1");

    let calls = Rc::new(RefCell::new(0));
    let store = FakeStore {
        total: 3,
        parse_calls: calls.clone(),
    };
    let mut app = App::new(store, all_packages(), 3).unwrap();
    app.set_highlight_config(&cfg_path).unwrap();
    let calls_after_load = *calls.borrow();
    assert_eq!(calls_after_load, 1, "one rule, one parse at initial load");

    let t0 = Instant::now();
    // Two rapid writes ("editors write twice"), both well inside the
    // debounce window.
    write_one_rule(&cfg_path, "second", "g2");
    app.tick(t0).unwrap();
    write_one_rule(&cfg_path, "third", "g3");
    app.tick(t0 + Duration::from_millis(50)).unwrap();
    assert_eq!(
        *calls.borrow(),
        calls_after_load,
        "no reload yet — still inside the debounce window"
    );

    // Only once the *final* content has held steady for the full debounce
    // window does exactly one reload happen.
    app.tick(t0 + Duration::from_millis(450)).unwrap();
    assert_eq!(
        *calls.borrow(),
        calls_after_load + 1,
        "exactly one reload, not two, for two rapid writes"
    );
    assert_eq!(
        app.row_tokens(0).gutters,
        vec!["g3".to_string()],
        "the reload must reflect the final, settled content"
    );

    let _ = std::fs::remove_file(&cfg_path);
}
