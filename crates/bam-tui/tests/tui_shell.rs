//! P3.4's three test bullets: visible-window-only querying, flat memory
//! against result-set size, and a buffer snapshot for a small fixture.

use std::cell::RefCell;
use std::rc::Rc;

use bam_core::query::ir::Predicate;
use bam_core::query::lang::ParseError;
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

/// A store with `total` synthetic packages, never touching a database, that
/// records every `(offset, limit)` it was asked for.
struct FakeStore {
    total: usize,
    calls: Rc<RefCell<Vec<(usize, usize)>>>,
}

impl PackageStore for FakeStore {
    fn window(
        &mut self,
        _pred: &Predicate,
        offset: usize,
        limit: usize,
    ) -> Result<WindowResult, StoreError> {
        self.calls.borrow_mut().push((offset, limit));
        let end = (offset + limit).min(self.total);
        let packages = (offset..end).map(fake_package).collect();
        Ok(WindowResult {
            packages,
            total: self.total,
        })
    }

    /// Unused by these tests (P3.4 predates the query line) — a placeholder
    /// so `FakeStore` satisfies the trait.
    fn parse(&self, src: &str) -> Result<Predicate, ParseError> {
        Ok(Predicate::FullText(src.to_string()))
    }

    // P3.8 grew the trait with highlight-rule operations (see
    // tui_highlight.rs); unused placeholders here, same as `parse` above —
    // `App::new` never calls them for an app with no highlight config wired.
    fn parse_lang(&self, _lang: Option<&str>, src: &str) -> Result<Predicate, StoreError> {
        Ok(Predicate::FullText(src.to_string()))
    }
    fn matching_ids(&self, _pred: &Predicate, _ids: &[i64]) -> Result<Vec<i64>, StoreError> {
        Ok(Vec::new())
    }

    // P3.6 grew the trait with selection operations (see tui_selection.rs);
    // unused placeholders here, same as `parse` above. `is_marked` must
    // still return `Ok` rather than panic — `App::new` calls it for every
    // loaded row regardless of which test is running.
    fn toggle(&self, _package_id: i64) -> Result<bool, StoreError> {
        unimplemented!()
    }
    fn is_marked(&self, _package_id: i64) -> Result<bool, StoreError> {
        Ok(false)
    }
    fn mark(&self, _package_id: i64) -> Result<(), StoreError> {
        unimplemented!()
    }
    fn select_by_query(
        &self,
        _pred: &Predicate,
        _mode: bam_core::api::SelectionMode,
    ) -> Result<usize, StoreError> {
        unimplemented!()
    }
    fn save_as(&self, _name: &str) -> Result<(), StoreError> {
        unimplemented!()
    }
    fn load(&self, _name: &str) -> Result<(), StoreError> {
        unimplemented!()
    }
    fn list_selections(&self) -> Result<Vec<bam_core::api::SelectionSummary>, StoreError> {
        unimplemented!()
    }
}

#[test]
fn scrolling_by_one_row_does_not_requery_the_whole_set() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let store = FakeStore {
        total: 84_000,
        calls: calls.clone(),
    };
    let mut app = App::new(store, all_packages(), 20).unwrap();
    assert_eq!(*calls.borrow(), vec![(0, 20)]);

    // Scrolling within the loaded page (cursor 0 -> 19, all inside the
    // initial [0, 20) window) issues no further query.
    for _ in 0..19 {
        app.move_down(1).unwrap();
    }
    assert_eq!(calls.borrow().len(), 1);

    // The 20th single-row scroll steps past the loaded page and must
    // re-query — but only for another viewport-sized page, never the whole
    // 84,000-row result set.
    app.move_down(1).unwrap();
    let seen = calls.borrow().clone();
    assert_eq!(seen.len(), 2, "expected exactly one re-query, got {seen:?}");
    for (_, limit) in &seen {
        assert_eq!(
            *limit, 20,
            "a query must never ask for more than the viewport"
        );
    }
}

#[test]
fn memory_does_not_scale_with_result_set_size() {
    let small = App::new(
        FakeStore {
            total: 100,
            calls: Rc::new(RefCell::new(Vec::new())),
        },
        all_packages(),
        20,
    )
    .unwrap();
    let large = App::new(
        FakeStore {
            total: 84_000,
            calls: Rc::new(RefCell::new(Vec::new())),
        },
        all_packages(),
        20,
    )
    .unwrap();

    assert_eq!(small.visible().len(), 20);
    assert_eq!(large.visible().len(), 20);
    assert_eq!(small.visible().len(), large.visible().len());
}

#[test]
fn buffer_snapshot_for_a_small_fixture() {
    let store = FakeStore {
        total: 3,
        calls: Rc::new(RefCell::new(Vec::new())),
    };
    let app = App::new(store, all_packages(), 3).unwrap();

    let backend = TestBackend::new(40, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(&app, frame)).unwrap();

    let buffer = terminal.backend().buffer().clone();
    let rendered: String = buffer.content().iter().map(|cell| cell.symbol()).collect();

    assert!(rendered.contains("search:"));
    assert!(rendered.contains("games/action/pkg0.lha"));
    assert!(rendered.contains("packages (1/3)"));
    assert!(rendered.contains("detail"));
}
