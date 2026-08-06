//! Virtualized list state (P3.4): tracks a cursor over the *whole* result
//! set but only ever holds `viewport_len` [`Package`] records in memory,
//! re-querying [`PackageStore`] only when the cursor would move outside the
//! currently loaded window.

use std::time::{Duration, Instant};

use bam_core::api::{SelectionMode, SelectionSummary};
use bam_core::query::ir::{FieldId, Pattern, Predicate};
use bam_core::query::lang::ParseError;
use bam_core::store::tables::Package;

use crate::store::{PackageStore, StoreError, WindowResult};

/// P3.5's debounce window: rapid keystrokes reset this deadline instead of
/// each querying immediately, so a query fires once typing settles.
const DEBOUNCE: Duration = Duration::from_millis(150);

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
    /// Marked state for each row in `window`, same order — a rendering
    /// cache refreshed from the store's own membership data after every
    /// window change, never an independent record of membership (I7: the
    /// working selection is the source of truth, kept in `store::session`).
    marked: Vec<bool>,
    /// Anchor row set by [`Self::enter_visual`]; `Some` for the duration of
    /// Visual mode. Movement while set just moves `cursor` as usual (no
    /// special-casing needed); [`Self::toggle_mark`] reads it to mark the
    /// `[anchor, cursor]` range instead of the single row under the cursor.
    visual_anchor: Option<usize>,
    query_text: String,
    query_error: Option<ParseError>,
    debounce_deadline: Option<Instant>,
    command_text: String,
    status: Option<String>,
}

impl<S: PackageStore> App<S> {
    pub fn new(
        mut store: S,
        predicate: Predicate,
        viewport_len: usize,
    ) -> Result<Self, StoreError> {
        let viewport_len = viewport_len.max(1);
        let window = store.window(&predicate, 0, viewport_len)?;
        let marked = Self::fetch_marked(&store, &window)?;
        Ok(Self {
            store,
            predicate,
            viewport_len,
            cursor: 0,
            top: 0,
            window,
            marked,
            visual_anchor: None,
            query_text: String::new(),
            query_error: None,
            debounce_deadline: None,
            command_text: String::new(),
            status: None,
        })
    }

    fn fetch_marked(store: &S, window: &WindowResult) -> Result<Vec<bool>, StoreError> {
        window
            .packages
            .iter()
            .map(|p| store.is_marked(p.id))
            .collect()
    }

    fn refresh_marked(&mut self) -> Result<(), StoreError> {
        self.marked = Self::fetch_marked(&self.store, &self.window)?;
        Ok(())
    }

    pub fn query_text(&self) -> &str {
        &self.query_text
    }

    pub fn query_error(&self) -> Option<&ParseError> {
        self.query_error.as_ref()
    }

    /// Updates the query line's text and (re)starts the debounce window.
    /// Does not query yet — [`Self::tick`] applies the edit once the
    /// debounce settles, so rapid keystrokes coalesce into one query.
    pub fn edit_query(&mut self, text: String, now: Instant) {
        self.query_text = text;
        self.debounce_deadline = Some(now + DEBOUNCE);
    }

    /// Applies a pending debounced edit once its deadline has passed; a
    /// no-op otherwise. A parse error leaves `predicate`/`window` untouched
    /// (the "keep last valid result set" rule) and is recorded for the UI
    /// to render instead.
    pub fn tick(&mut self, now: Instant) -> Result<(), StoreError> {
        let Some(deadline) = self.debounce_deadline else {
            return Ok(());
        };
        if now < deadline {
            return Ok(());
        }
        self.debounce_deadline = None;
        match self.store.parse(&self.query_text) {
            Ok(predicate) => {
                self.query_error = None;
                self.predicate = predicate;
                self.cursor = 0;
                self.top = 0;
                self.window = self.store.window(&self.predicate, 0, self.viewport_len)?;
                self.refresh_marked()?;
            }
            Err(e) => self.query_error = Some(e),
        }
        Ok(())
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
        self.refresh_marked()
    }

    pub fn visible_marked(&self) -> &[bool] {
        &self.marked
    }

    pub fn enter_visual(&mut self) {
        self.visual_anchor = Some(self.cursor);
    }

    pub fn leave_visual(&mut self) {
        self.visual_anchor = None;
    }

    /// Outside Visual mode, toggles the row under the cursor. Inside it
    /// (an anchor is set), marks every row from the anchor to the cursor —
    /// vim's Visual-then-space semantics — and leaves Visual mode.
    pub fn toggle_mark(&mut self) -> Result<(), StoreError> {
        if let Some(anchor) = self.visual_anchor.take() {
            return self.mark_range(anchor, self.cursor);
        }
        let idx = self.cursor - self.top;
        let Some(pkg) = self.window.packages.get(idx) else {
            return Ok(());
        };
        let marked = self.store.toggle(pkg.id)?;
        self.marked[idx] = marked;
        Ok(())
    }

    /// Marks every package in `[a, b]` (inclusive, either order) by
    /// fetching exactly that range through [`PackageStore::window`] — the
    /// range is usually the current viewport, but re-fetching by predicate
    /// keeps this correct even if Visual mode scrolled past it.
    fn mark_range(&mut self, a: usize, b: usize) -> Result<(), StoreError> {
        let start = a.min(b);
        let len = a.max(b) - start + 1;
        let range = self.store.window(&self.predicate, start, len)?;
        for pkg in &range.packages {
            self.store.mark(pkg.id)?;
        }
        self.refresh_marked()
    }

    pub fn command_text(&self) -> &str {
        &self.command_text
    }

    pub fn edit_command(&mut self, text: String) {
        self.command_text = text;
    }

    pub fn clear_command(&mut self) {
        self.command_text.clear();
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn set_status(&mut self, message: String) {
        self.status = Some(message);
    }

    /// `:mark <query>`, `:unmark <query>`, `:save <name>`, `:load <name>`,
    /// `:selections` (P3.6) — the command line's whole vocabulary. `save`/
    /// `load` accept an optionally double-quoted name so a selection can
    /// contain spaces (`:save "tracker candidates"`).
    pub fn run_command(&mut self, cmd: &str) -> Result<CommandOutcome, StoreError> {
        let cmd = cmd.trim();
        let (word, rest) = cmd.split_once(char::is_whitespace).unwrap_or((cmd, ""));
        let rest = unquote(rest.trim());
        match word {
            "mark" => self.select_by_query_command(rest, SelectionMode::Union),
            "unmark" => self.select_by_query_command(rest, SelectionMode::Subtract),
            "save" => {
                self.store.save_as(rest)?;
                Ok(CommandOutcome::Saved)
            }
            "load" => {
                self.store.load(rest)?;
                self.refresh_marked()?;
                Ok(CommandOutcome::Loaded)
            }
            "selections" => Ok(CommandOutcome::Selections(self.store.list_selections()?)),
            other => Ok(CommandOutcome::Unknown(other.to_string())),
        }
    }

    fn select_by_query_command(
        &mut self,
        query_src: &str,
        mode: SelectionMode,
    ) -> Result<CommandOutcome, StoreError> {
        let pred = self
            .store
            .parse(query_src)
            .map_err(|e| StoreError(e.message))?;
        let count = self.store.select_by_query(&pred, mode)?;
        self.refresh_marked()?;
        Ok(CommandOutcome::MemberCount(count))
    }
}

fn unquote(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s)
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandOutcome {
    MemberCount(usize),
    Saved,
    Loaded,
    Selections(Vec<SelectionSummary>),
    Unknown(String),
}
