//! `[[highlight]]` rules from `bam.toml` (P3.8, invariant I3): each rule's
//! `when` is compiled through the query language named by its own `lang`
//! (falling back to the registry default) into a [`bam_core::query::ir::Predicate`],
//! evaluated per-window against only the currently visible rows, and fed
//! into `bam_core::highlight::resolve` alongside marked-state (Round 17) —
//! through `App::row_tokens`, the same conflict-resolution path, not a
//! parallel one.
//!
//! The file is polled, not watched via a filesystem-event crate — nothing
//! here needs more than "did the content change." A content change starts a
//! timer on the same cadence as P3.5's query-line debounce; only a change
//! that has held steady for [`RELOAD_DEBOUNCE`] triggers a reload, so an
//! editor's two rapid writes from one save collapse into a single reload.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use bam_core::highlight::Decoration;
use bam_core::query::ir::Predicate;

use crate::store::PackageStore;

const RELOAD_DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct FileConfig {
    #[serde(default)]
    highlight: Vec<RuleConfig>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RuleConfig {
    name: String,
    #[serde(default)]
    lang: Option<String>,
    when: String,
    #[serde(default)]
    gutter: Option<String>,
    #[serde(default)]
    badge: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    priority: i32,
}

/// One `[[highlight]]` block whose `when` compiled successfully.
pub struct Rule {
    pub name: String,
    pub predicate: Predicate,
    pub decoration: Decoration,
}

/// Rules loaded from `bam.toml`, plus one message per rule that failed to
/// resolve — an unregistered `lang` or a `when` the language rejected
/// (syntax, an unknown/mistyped field, or a predicate shape the compiler
/// doesn't support) — reported by the caller rather than taking down the
/// app or disabling the other rules.
pub struct HighlightRules {
    path: Option<PathBuf>,
    last_content: Option<String>,
    pending: Option<(Option<String>, Instant)>,
    rules: Vec<Rule>,
    errors: Vec<String>,
}

impl HighlightRules {
    /// No config wired in — the state every [`crate::app::App`] starts with
    /// until [`crate::app::App::set_highlight_config`] is called. `poll` is
    /// then a no-op, at the cost of one `Option` check.
    pub fn empty() -> Self {
        Self {
            path: None,
            last_content: None,
            pending: None,
            rules: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn load<P: PackageStore>(path: impl Into<PathBuf>, store: &P) -> Self {
        let mut me = Self {
            path: Some(path.into()),
            last_content: None,
            pending: None,
            rules: Vec::new(),
            errors: Vec::new(),
        };
        me.reload(store);
        me
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    fn read(&self) -> Option<String> {
        self.path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
    }

    /// Call on every app tick. Returns whether a reload just happened, so
    /// the caller knows to re-evaluate rules against the visible window.
    pub fn poll<P: PackageStore>(&mut self, now: Instant, store: &P) -> bool {
        if self.path.is_none() {
            return false;
        }
        let current = self.read();
        if current == self.last_content {
            self.pending = None;
            return false;
        }
        match &self.pending {
            Some((seen, since)) if *seen == current => {
                if now.duration_since(*since) >= RELOAD_DEBOUNCE {
                    self.pending = None;
                    self.reload(store);
                    return true;
                }
            }
            _ => self.pending = Some((current, now)),
        }
        false
    }

    /// Parses (and, with an empty trial id list, compiles) every rule's
    /// `when`. A rule that fails either step is dropped from [`Self::rules`]
    /// and its message recorded in [`Self::errors`] instead of aborting the
    /// whole reload — one bad rule cannot disable the others.
    fn reload<P: PackageStore>(&mut self, store: &P) {
        let content = self.read();
        let config: FileConfig = content
            .as_deref()
            .and_then(|s| toml::from_str(s).ok())
            .unwrap_or_default();
        let mut rules = Vec::new();
        let mut errors = Vec::new();
        for rc in config.highlight {
            let compiled = store
                .parse_lang(rc.lang.as_deref(), &rc.when)
                .map_err(|e| e.to_string())
                .and_then(|pred| {
                    store
                        .matching_ids(&pred, &[])
                        .map(|_| pred)
                        .map_err(|e| e.to_string())
                });
            match compiled {
                Ok(predicate) => rules.push(Rule {
                    name: rc.name,
                    predicate,
                    decoration: Decoration {
                        gutter: rc.gutter,
                        badge: rc.badge,
                        background: rc.background,
                        priority: rc.priority,
                    },
                }),
                Err(msg) => errors.push(format!("{}: {msg}", rc.name)),
            }
        }
        self.last_content = content;
        self.rules = rules;
        self.errors = errors;
    }
}
