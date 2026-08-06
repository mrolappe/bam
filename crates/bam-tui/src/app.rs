//! Virtualized list state (P3.4): tracks a cursor over the *whole* result
//! set but only ever holds `viewport_len` [`Package`] records in memory,
//! re-querying [`PackageStore`] only when the cursor would move outside the
//! currently loaded window.

use bam_core::query::ir::{FieldId, Pattern, Predicate};
use bam_core::store::tables::Package;

use crate::store::{PackageStore, StoreError, WindowResult};

/// Matches every package with a non-null `dir` — true of every row, `dir`
/// being `NOT NULL` (P1.2's schema) — since v1's query line (P3.5) doesn't
/// exist yet to supply a real one.
pub fn all_packages() -> Predicate {
    Predicate::Match {
        field: FieldId::new("dir"),
        pattern: Pattern::Glob("*".to_string()),
    }
}

pub struct App<S: PackageStore> {
    store: S,
    predicate: Predicate,
    viewport_len: usize,
    cursor: usize,
    top: usize,
    window: WindowResult,
}

impl<S: PackageStore> App<S> {
    pub fn new(
        mut store: S,
        predicate: Predicate,
        viewport_len: usize,
    ) -> Result<Self, StoreError> {
        let viewport_len = viewport_len.max(1);
        let window = store.window(&predicate, 0, viewport_len)?;
        Ok(Self {
            store,
            predicate,
            viewport_len,
            cursor: 0,
            top: 0,
            window,
        })
    }

    pub fn visible(&self) -> &[Package] {
        &self.window.packages
    }

    pub fn total(&self) -> usize {
        self.window.total
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn window_start(&self) -> usize {
        self.top
    }

    pub fn selected(&self) -> Option<&Package> {
        self.window.packages.get(self.cursor - self.top)
    }

    pub fn move_down(&mut self, count: usize) -> Result<(), StoreError> {
        let last = self.window.total.saturating_sub(1);
        self.cursor = (self.cursor + count).min(last);
        self.sync_window()
    }

    pub fn move_up(&mut self, count: usize) -> Result<(), StoreError> {
        self.cursor = self.cursor.saturating_sub(count);
        self.sync_window()
    }

    pub fn go_top(&mut self) -> Result<(), StoreError> {
        self.cursor = 0;
        self.sync_window()
    }

    pub fn go_bottom(&mut self) -> Result<(), StoreError> {
        self.cursor = self.window.total.saturating_sub(1);
        self.sync_window()
    }

    /// Re-fetches the window only if `cursor` fell outside it — the
    /// virtualization requirement: a scroll that stays inside the loaded
    /// page costs nothing.
    fn sync_window(&mut self) -> Result<(), StoreError> {
        let new_top = if self.cursor < self.top {
            self.cursor
        } else if self.cursor >= self.top + self.viewport_len {
            self.cursor + 1 - self.viewport_len
        } else {
            return Ok(());
        };
        self.top = new_top;
        self.window = self
            .store
            .window(&self.predicate, self.top, self.viewport_len)?;
        Ok(())
    }
}
