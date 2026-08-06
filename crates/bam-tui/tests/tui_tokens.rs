//! P3.7's fourth test bullet: a marked row must render its token through
//! the exact same [`bam_core::highlight::resolve`] path a DSL highlight
//! rule's decoration would (P3.8 doesn't exist yet to produce a real one,
//! so this builds an equivalent `Decoration` by hand and asserts `App`
//! produces the same resolved tokens). The fifth bullet (unknown token →
//! unstyled, not a panic) is covered by `bam_tui::tokens`'s own inline test.

use std::cell::RefCell;
use std::collections::HashSet;

use bam_core::api::{SelectionMode, SelectionSummary};
use bam_core::highlight::{Decoration, MARKED_GUTTER, MARKED_PRIORITY, resolve};
use bam_core::query::ir::Predicate;
use bam_core::query::lang::ParseError;
use bam_core::store::tables::Package;
use bam_tui::app::{App, all_packages};
use bam_tui::store::{PackageStore, StoreError, WindowResult};

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
        description: None,
        landing_id: 0,
    }
}

struct FakeStore {
    marked: RefCell<HashSet<i64>>,
}

impl PackageStore for FakeStore {
    fn window(
        &mut self,
        _pred: &Predicate,
        offset: usize,
        limit: usize,
    ) -> Result<WindowResult, StoreError> {
        let end = (offset + limit).min(3);
        Ok(WindowResult {
            packages: (offset..end).map(fake_package).collect(),
            total: 3,
        })
    }

    fn parse(&self, src: &str) -> Result<Predicate, ParseError> {
        Ok(Predicate::FullText(src.to_string()))
    }

    // P3.8 grew the trait with highlight-rule operations (see
    // tui_highlight.rs); unused placeholders here since this test never
    // wires a highlight config in.
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

    fn select_by_query(
        &self,
        _pred: &Predicate,
        _mode: SelectionMode,
    ) -> Result<usize, StoreError> {
        unimplemented!("not exercised by this test")
    }
    fn save_as(&self, _name: &str) -> Result<(), StoreError> {
        unimplemented!("not exercised by this test")
    }
    fn load(&self, _name: &str) -> Result<(), StoreError> {
        unimplemented!("not exercised by this test")
    }
    fn list_selections(&self) -> Result<Vec<SelectionSummary>, StoreError> {
        unimplemented!("not exercised by this test")
    }
}

#[test]
fn a_marked_row_resolves_through_the_same_path_as_a_highlight_rule() {
    let store = FakeStore {
        marked: RefCell::new(HashSet::new()),
    };
    let mut app = App::new(store, all_packages(), 3).unwrap();
    app.toggle_mark().unwrap();

    // The tokens App::row_tokens produces for the now-marked row 0 must be
    // exactly what resolve() returns for a hand-built Decoration carrying
    // the same marked-state gutter/priority — the same function, not a
    // parallel special case for marked rows.
    let expected = resolve(&[Decoration {
        gutter: Some(MARKED_GUTTER.to_string()),
        badge: None,
        background: None,
        priority: MARKED_PRIORITY,
    }]);
    assert_eq!(app.row_tokens(0), expected);

    // An unmarked row resolves to no tokens at all, through the same path.
    assert_eq!(app.row_tokens(1), resolve(&[]));
}
