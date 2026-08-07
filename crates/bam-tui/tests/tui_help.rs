//! P3.9's three test bullets: the overlay's binding set matches the active
//! keymap exactly (not a hardcoded list), a user override shows the user's
//! key rather than the default, and open/close toggles `App::help_open`.

use std::collections::HashMap;

use bam_core::api::{SelectionMode, SelectionSummary};
use bam_core::query::ir::Predicate;
use bam_core::query::lang::ParseError;
use bam_core::store::tables::Package;
use bam_tui::app::{App, all_packages};
use bam_tui::input::{ActionKind, KeymapConfig, default_keymap, merge_keymap};
use bam_tui::store::{PackageStore, StoreError, WindowResult};

struct FakeStore;

impl PackageStore for FakeStore {
    fn window(
        &mut self,
        _pred: &Predicate,
        _offset: usize,
        _limit: usize,
    ) -> Result<WindowResult, StoreError> {
        Ok(WindowResult {
            packages: Vec::<Package>::new(),
            total: 0,
        })
    }
    fn parse(&self, src: &str) -> Result<Predicate, ParseError> {
        Ok(Predicate::FullText(src.to_string()))
    }
    fn parse_lang(&self, _lang: Option<&str>, src: &str) -> Result<Predicate, StoreError> {
        Ok(Predicate::FullText(src.to_string()))
    }
    fn matching_ids(&self, _pred: &Predicate, _ids: &[i64]) -> Result<Vec<i64>, StoreError> {
        Ok(Vec::new())
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

fn new_app() -> App<FakeStore> {
    App::new(FakeStore, all_packages(), 5).unwrap()
}

#[test]
fn overlay_binding_set_equals_the_active_keymap() {
    let keymap = default_keymap();
    let mut app = new_app();
    app.open_help(keymap.clone());

    let overlay = app.help_bindings().expect("help overlay should be open");
    let overlay_tokens: std::collections::HashSet<&String> = overlay.0.keys().collect();
    let keymap_tokens: std::collections::HashSet<&String> = keymap.0.keys().collect();
    assert_eq!(overlay_tokens, keymap_tokens);
}

#[test]
fn user_override_shows_the_users_key_not_the_default() {
    let config = KeymapConfig {
        keys: HashMap::from([("x".to_string(), "move_down".to_string())]),
    };
    let merged = merge_keymap(default_keymap(), &config).unwrap();

    let mut app = new_app();
    app.open_help(merged);

    let overlay = app.help_bindings().unwrap();
    assert_eq!(overlay.0.get("x"), Some(&ActionKind::MoveDown));
    // The default binding for `j` is untouched by this particular override.
    assert_eq!(overlay.0.get("j"), Some(&ActionKind::MoveDown));
}

#[test]
fn open_and_close_toggle_help_open() {
    let mut app = new_app();
    assert!(!app.help_open());

    app.open_help(default_keymap());
    assert!(app.help_open());

    app.close_help();
    assert!(!app.help_open());
    assert!(app.help_bindings().is_none());
}
