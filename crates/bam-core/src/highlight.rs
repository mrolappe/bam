//! Semantic decoration tokens (`bam-handoff.md` §11.1, invariant I7's
//! "selections render through this path too"). A `Decoration` is one rule's
//! or provider's suggestion for a row; `resolve` combines every decoration
//! that applies to a row into the tokens actually rendered. Pure data and
//! logic — no database driver dependency — so every frontend (TUI now, a
//! future web/GUI client) shares one conflict-resolution implementation
//! instead of each re-deriving it.
//!
//! Frontends map the resulting token strings (e.g. `"user"`, `"XL"`) to
//! their own presentation (a ratatui `Style` in the TUI, a CSS class in a
//! future GUI) — that mapping is frontend-specific and lives there, not here.

use serde::{Deserialize, Serialize};

/// One rule's or provider's decoration suggestion for a single row. Multiple
/// decorations from different rules/providers may apply to the same row;
/// [`resolve`] combines them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decoration {
    pub gutter: Option<String>,
    pub badge: Option<String>,
    pub background: Option<String>,
    pub priority: i32,
}

/// Priority reserved for marked-state decorations, so a marked row's gutter
/// always outranks ordinary highlight rules when gutters stack.
pub const MARKED_PRIORITY: i32 = i32::MAX;
pub const MARKED_GUTTER: &str = "marked";

/// Conflict-resolved tokens for one row: `background` is exclusive (highest
/// `priority` wins; a strict `>` comparison means the first decoration to
/// reach a given priority keeps it, so equal priorities resolve to whichever
/// came first in `decorations` — stable, not hash-order-dependent).
/// `gutters`/`badges` stack, ordered highest priority first, capped at
/// [`STACK_CAP`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowTokens {
    pub background: Option<String>,
    pub gutters: Vec<String>,
    pub badges: Vec<String>,
}

const STACK_CAP: usize = 3;

pub fn resolve(decorations: &[Decoration]) -> RowTokens {
    let mut background: Option<(&str, i32)> = None;
    let mut gutters: Vec<(&str, i32)> = Vec::new();
    let mut badges: Vec<(&str, i32)> = Vec::new();

    for d in decorations {
        if let Some(bg) = &d.background {
            if background.is_none_or(|(_, p)| d.priority > p) {
                background = Some((bg, d.priority));
            }
        }
        if let Some(g) = &d.gutter {
            gutters.push((g, d.priority));
        }
        if let Some(b) = &d.badge {
            badges.push((b, d.priority));
        }
    }

    // `sort_by` is stable, so equal-priority entries keep their relative
    // input order rather than depending on sort/hash order.
    gutters.sort_by_key(|(_, p)| std::cmp::Reverse(*p));
    badges.sort_by_key(|(_, p)| std::cmp::Reverse(*p));

    RowTokens {
        background: background.map(|(t, _)| t.to_string()),
        gutters: gutters
            .into_iter()
            .take(STACK_CAP)
            .map(|(t, _)| t.to_string())
            .collect(),
        badges: badges
            .into_iter()
            .take(STACK_CAP)
            .map(|(t, _)| t.to_string())
            .collect(),
    }
}
