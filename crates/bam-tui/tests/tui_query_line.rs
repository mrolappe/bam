//! P3.5's four test bullets: an inline error marker under the offending
//! span, previous results kept while the query is invalid, debounce
//! coalescing rapid keystrokes into one query, and a valid query replacing
//! results.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use bam_core::query::bam_dsl::BamDsl;
use bam_core::query::ir::Predicate;
use bam_core::query::lang::{ParseError, QueryLanguage};
use bam_core::query::registry::{FieldRegistry, package_fields};
use bam_core::store::tables::Package;
use bam_tui::app::{App, all_packages};
use bam_tui::store::{PackageStore, StoreError, WindowResult};
use bam_tui::ui::render;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn fake_package(i: usize) -> Package {
    Package {
        id: i as i64,
        dir: "games/action".to_string(),
        file: format!("pkg{i}.lha"),
        name: format!("pkg{i}"),
        version: None,
        size_bytes: Some(1024),
        uploaded_on: Some("2026-01-01".into()),
        date_precision: "exact".into(),
        description: Some(format!("package number {i}")),
        landing_id: 0,
    }
}

/// Parses through the real `bam-dsl` grammar (pure, no database) so parse
/// errors and spans in these tests are genuine, not hand-rolled. `window`
/// distinguishes only the initial "match everything" predicate from any
/// other — enough to prove a valid edit changes what's shown, without
/// reimplementing glob/compare semantics just for a fake.
struct FakeStore {
    window_calls: Rc<RefCell<usize>>,
}

impl PackageStore for FakeStore {
    fn window(
        &mut self,
        pred: &Predicate,
        offset: usize,
        limit: usize,
    ) -> Result<WindowResult, StoreError> {
        *self.window_calls.borrow_mut() += 1;
        let packages: Vec<Package> = if *pred == all_packages() {
            vec![fake_package(0)]
        } else {
            vec![fake_package(1), fake_package(2)]
        };
        let total = packages.len();
        Ok(WindowResult {
            packages: packages.into_iter().skip(offset).take(limit).collect(),
            total,
        })
    }

    fn parse(&self, src: &str) -> Result<Predicate, ParseError> {
        let reg = FieldRegistry::new(package_fields());
        BamDsl.parse(src, &reg)
    }
}

fn new_app() -> (App<FakeStore>, Rc<RefCell<usize>>) {
    let calls = Rc::new(RefCell::new(0));
    let store = FakeStore {
        window_calls: calls.clone(),
    };
    (App::new(store, all_packages(), 20).unwrap(), calls)
}

#[test]
fn invalid_query_keeps_previous_results_and_reports_the_error() {
    let (mut app, calls) = new_app();
    let before = app.visible().to_vec();
    assert_eq!(*calls.borrow(), 1);

    let t0 = Instant::now();
    app.edit_query("dir:util/* size>".to_string(), t0);
    app.tick(t0 + Duration::from_millis(200)).unwrap();

    assert!(app.query_error().is_some());
    assert_eq!(app.visible(), before.as_slice());
    assert_eq!(*calls.borrow(), 1, "an invalid edit must not requery");
}

#[test]
fn debounce_coalesces_rapid_keystrokes_into_one_query() {
    let (mut app, calls) = new_app();
    assert_eq!(*calls.borrow(), 1); // the initial query from App::new

    let t0 = Instant::now();
    let text = "dir:util/*";
    for i in 1..=text.len() {
        let partial = &text[..i];
        let edit_at = t0 + Duration::from_millis((i as u64 - 1) * 10);
        app.edit_query(partial.to_string(), edit_at);
        // Ticking shortly after each keystroke, well inside the 150ms
        // debounce window, must not issue a query yet.
        app.tick(edit_at + Duration::from_millis(5)).unwrap();
    }
    assert_eq!(
        *calls.borrow(),
        1,
        "no query while keystrokes are still arriving"
    );

    let last_edit = t0 + Duration::from_millis((text.len() as u64 - 1) * 10);
    app.tick(last_edit + Duration::from_millis(200)).unwrap();
    assert_eq!(
        *calls.borrow(),
        2,
        "exactly one query once the debounce settles"
    );
}

#[test]
fn valid_query_replaces_results() {
    let (mut app, _calls) = new_app();
    let before = app.visible().to_vec();

    let t0 = Instant::now();
    app.edit_query("dir:util/*".to_string(), t0);
    app.tick(t0 + Duration::from_millis(200)).unwrap();

    assert!(app.query_error().is_none());
    assert_ne!(app.visible(), before.as_slice());
}

#[test]
fn error_marker_renders_under_the_offending_span() {
    let (mut app, _calls) = new_app();
    let t0 = Instant::now();
    let text = "dir:util/* size>".to_string();
    app.edit_query(text, t0);
    app.tick(t0 + Duration::from_millis(200)).unwrap();
    assert!(app.query_error().is_some());

    let backend = TestBackend::new(40, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(&app, frame)).unwrap();
    let buffer = terminal.backend().buffer();

    // "dir:util/* size>" is 16 bytes; the trailing '>' (the offending
    // operator with nothing after it) is its last character, at index 15.
    let prefix_len = "search: ".len() as u16;
    let col = prefix_len + 15;
    assert_eq!(buffer[(col, 1)].symbol(), "^");
}
