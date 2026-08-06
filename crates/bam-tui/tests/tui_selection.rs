//! P3.6's four test bullets: `space` toggles membership and rendering
//! updates, Visual mode over N rows marks exactly N, `:mark <query>` unions
//! into the working selection, and `:save`/`:load` round-trip across a
//! fresh session. The fifth bullet ("no selection state in the TUI") is a
//! diff-review check, not a test — see PROGRESS.md.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use bam_core::api::{SelectionMode, SelectionSummary, Session};
use bam_core::query::ir::Predicate;
use bam_core::query::lang::ParseError;
use bam_core::store::tables::{self, LandingIndexLine, Package};
use bam_tui::app::{App, CommandOutcome, all_packages};
use bam_tui::store::{PackageStore, SessionStore, StoreError, WindowResult};
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

/// A store over `total` synthetic packages (ids `0..total`) whose membership
/// lives in a shared `HashSet`, standing in for the working selection —
/// `select_by_query` only recognizes one fake predicate, `dir:mus/*`
/// (matching ids 2 and 3), enough to prove the union without reimplementing
/// the real compiler for a fake.
struct FakeStore {
    total: usize,
    marked: Rc<RefCell<HashSet<i64>>>,
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
            packages: (offset..end).map(fake_package).collect(),
            total: self.total,
        })
    }

    fn parse(&self, src: &str) -> Result<Predicate, ParseError> {
        Ok(Predicate::FullText(src.to_string()))
    }

    // P3.8 grew the trait with highlight-rule operations (see
    // tui_highlight.rs); unused placeholders here since these tests never
    // wire a highlight config in.
    fn parse_lang(&self, _lang: Option<&str>, src: &str) -> Result<Predicate, StoreError> {
        Ok(Predicate::FullText(src.to_string()))
    }
    fn matching_ids(&self, _pred: &Predicate, _ids: &[i64]) -> Result<Vec<i64>, StoreError> {
        Ok(Vec::new())
    }

    fn toggle(&self, package_id: i64) -> Result<bool, StoreError> {
        let mut marked = self.marked.borrow_mut();
        if marked.remove(&package_id) {
            Ok(false)
        } else {
            marked.insert(package_id);
            Ok(true)
        }
    }

    fn is_marked(&self, package_id: i64) -> Result<bool, StoreError> {
        Ok(self.marked.borrow().contains(&package_id))
    }

    fn mark(&self, package_id: i64) -> Result<(), StoreError> {
        self.marked.borrow_mut().insert(package_id);
        Ok(())
    }

    fn select_by_query(&self, pred: &Predicate, mode: SelectionMode) -> Result<usize, StoreError> {
        assert_eq!(mode, SelectionMode::Union, "the `:mark` command unions");
        let Predicate::FullText(src) = pred else {
            panic!("unexpected predicate shape: {pred:?}");
        };
        assert_eq!(src, "dir:mus/*");
        let mut marked = self.marked.borrow_mut();
        marked.insert(2);
        marked.insert(3);
        Ok(marked.len())
    }

    fn save_as(&self, _name: &str) -> Result<(), StoreError> {
        unimplemented!("not exercised by the fake-store tests")
    }
    fn load(&self, _name: &str) -> Result<(), StoreError> {
        unimplemented!("not exercised by the fake-store tests")
    }
    fn list_selections(&self) -> Result<Vec<SelectionSummary>, StoreError> {
        unimplemented!("not exercised by the fake-store tests")
    }
}

#[test]
fn space_toggles_membership_and_rendering_updates() {
    let marked = Rc::new(RefCell::new(HashSet::new()));
    let store = FakeStore {
        total: 5,
        marked: marked.clone(),
    };
    let mut app = App::new(store, all_packages(), 5).unwrap();

    assert!(!app.visible_marked()[0]);
    app.toggle_mark().unwrap();
    assert!(app.visible_marked()[0]);
    assert!(marked.borrow().contains(&0));

    let backend = TestBackend::new(40, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(&app, frame)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let rendered: String = buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(
        rendered.contains('*'),
        "a marked row must render its gutter marker"
    );

    app.toggle_mark().unwrap();
    assert!(!app.visible_marked()[0]);
    assert!(!marked.borrow().contains(&0));
}

#[test]
fn visual_mode_over_n_rows_marks_exactly_n() {
    let marked = Rc::new(RefCell::new(HashSet::new()));
    let store = FakeStore {
        total: 10,
        marked: marked.clone(),
    };
    let mut app = App::new(store, all_packages(), 10).unwrap();

    app.enter_visual();
    app.move_down(3).unwrap(); // cursor 0 -> 3: anchor..=cursor spans 4 rows
    app.toggle_mark().unwrap(); // confirms the visual mark

    assert_eq!(
        marked.borrow().len(),
        4,
        "exactly N (4) rows must be marked"
    );
    for id in 0..4 {
        assert!(marked.borrow().contains(&id), "row {id} must be marked");
    }
    assert!(
        !marked.borrow().contains(&4),
        "row past the visual range must stay unmarked"
    );
}

#[test]
fn colon_mark_query_unions_into_the_working_selection() {
    let marked = Rc::new(RefCell::new(HashSet::new()));
    let store = FakeStore {
        total: 5,
        marked: marked.clone(),
    };
    let mut app = App::new(store, all_packages(), 5).unwrap();

    let outcome = app.run_command("mark dir:mus/*").unwrap();
    assert_eq!(outcome, CommandOutcome::MemberCount(2));
    assert_eq!(*marked.borrow(), HashSet::from([2, 3]));
}

fn temp_db_path(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "bam-tui-selection-test-{label}-{}-{n}.sqlite",
        std::process::id()
    ))
}

/// Seeds one package directly through the store layer (P1.2), independent
/// of any query language, so this test only exercises P3.6's own plumbing.
fn seed_one_package(path: &std::path::Path) -> i64 {
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
    tables::insert_package(
        &conn,
        &Package {
            id: 0,
            dir: "mods/a".into(),
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
    .unwrap()
}

#[test]
fn save_and_load_round_trip_across_a_fresh_session() {
    let path = temp_db_path("save-load");
    seed_one_package(&path);

    {
        let session = Session::open(&path).unwrap();
        let mut app = App::new(SessionStore::new(session), all_packages(), 5).unwrap();
        app.toggle_mark().unwrap(); // marks the one package under the cursor
        assert_eq!(
            app.run_command(r#"save "tracker candidates""#).unwrap(),
            CommandOutcome::Saved
        );
    }
    // `app`/`session` dropped: the ephemeral working selection is gone, but
    // the named one must survive for a fresh session to load.

    let session = Session::open(&path).unwrap();
    let mut app = App::new(SessionStore::new(session), all_packages(), 5).unwrap();
    assert!(!app.visible_marked()[0], "a fresh session starts unmarked");
    assert_eq!(
        app.run_command(r#"load "tracker candidates""#).unwrap(),
        CommandOutcome::Loaded
    );
    assert!(
        app.visible_marked()[0],
        "load must restore the saved membership"
    );

    let _ = std::fs::remove_file(&path);
}
