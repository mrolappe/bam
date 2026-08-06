//! `[count] [operator] {motion | object | command}` resolver — invariant I6.
//!
//! v1 registers modes and motions only. The operator/object grammar slot is
//! named in `docs/input-model.md` but nothing here parses one yet.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Normal,
    Insert,
    Visual,
    Command,
}

/// A physical keypress. Decoupled from any terminal backend's key type so
/// this module doesn't need one to be tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Esc,
}

impl Key {
    /// The canonical token used both in `bam.toml` and to match a pressed
    /// sequence against the keymap, e.g. `Ctrl('d')` -> `"ctrl-d"`.
    fn token(self) -> String {
        match self {
            Key::Char(c) => c.to_string(),
            Key::Ctrl(c) => format!("ctrl-{c}"),
            Key::Esc => "esc".to_string(),
        }
    }
}

/// What a key sequence names, before a pending count is applied. Mirrors
/// [`Action`] one-for-one except for the variants a count changes the shape
/// of (`GoToRowOrBottom` resolves to either `Action::GoToRow` or
/// `Action::GoBottom` depending on whether a count preceded it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    MoveDown,
    MoveUp,
    LineStart,
    LineEnd,
    GoTop,
    GoToRowOrBottom,
    PageDown,
    PageUp,
    HalfPageDown,
    HalfPageUp,
    ScreenTop,
    ScreenMiddle,
    ScreenBottom,
    NextMatch,
    PrevMatch,
    ToggleMark,
    EnterVisual,
    EnterCommand,
    EnterSearch,
    LeaveMode,
    Quit,
}

/// A resolved, ready-to-execute instruction — counts already applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    MoveDown(usize),
    MoveUp(usize),
    LineStart,
    LineEnd,
    GoTop,
    GoBottom,
    GoToRow(usize),
    PageDown(usize),
    PageUp(usize),
    HalfPageDown(usize),
    HalfPageUp(usize),
    ScreenTop,
    ScreenMiddle,
    ScreenBottom,
    NextMatch(usize),
    PrevMatch(usize),
    ToggleMark,
    EnterMode(Mode),
    LeaveMode,
    Quit,
}

fn resolve_action(kind: ActionKind, count: Option<usize>) -> Action {
    match kind {
        ActionKind::MoveDown => Action::MoveDown(count.unwrap_or(1)),
        ActionKind::MoveUp => Action::MoveUp(count.unwrap_or(1)),
        ActionKind::LineStart => Action::LineStart,
        ActionKind::LineEnd => Action::LineEnd,
        ActionKind::GoTop => Action::GoTop,
        ActionKind::GoToRowOrBottom => match count {
            Some(n) => Action::GoToRow(n),
            None => Action::GoBottom,
        },
        ActionKind::PageDown => Action::PageDown(count.unwrap_or(1)),
        ActionKind::PageUp => Action::PageUp(count.unwrap_or(1)),
        ActionKind::HalfPageDown => Action::HalfPageDown(count.unwrap_or(1)),
        ActionKind::HalfPageUp => Action::HalfPageUp(count.unwrap_or(1)),
        ActionKind::ScreenTop => Action::ScreenTop,
        ActionKind::ScreenMiddle => Action::ScreenMiddle,
        ActionKind::ScreenBottom => Action::ScreenBottom,
        ActionKind::NextMatch => Action::NextMatch(count.unwrap_or(1)),
        ActionKind::PrevMatch => Action::PrevMatch(count.unwrap_or(1)),
        ActionKind::ToggleMark => Action::ToggleMark,
        ActionKind::EnterVisual => Action::EnterMode(Mode::Visual),
        ActionKind::EnterCommand => Action::EnterMode(Mode::Command),
        ActionKind::EnterSearch => Action::EnterMode(Mode::Insert),
        ActionKind::LeaveMode => Action::LeaveMode,
        ActionKind::Quit => Action::Quit,
    }
}

/// Bindings as they round-trip through `bam.toml`: key-sequence token
/// (`"j"`, `"gg"`, `"ctrl-d"`) to the action it names. Defaults (P3.3) and
/// user-override merging are layered on top of this type, not part of it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Keymap(pub HashMap<String, ActionKind>);

enum Lookup {
    Match(ActionKind),
    Prefix,
    None,
}

impl Keymap {
    fn lookup(&self, seq: &str) -> Lookup {
        if let Some(kind) = self.0.get(seq) {
            return Lookup::Match(*kind);
        }
        if self.0.keys().any(|bound| bound.starts_with(seq)) {
            Lookup::Prefix
        } else {
            Lookup::None
        }
    }
}

/// Why a key sequence didn't resolve. v1 has exactly one cause; kept as a
/// named type rather than a bare `String` since `docs/input-model.md` and
/// P3.2 both refer to it as `Reason`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reason(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    Pending,
    Resolved(Action),
    Rejected(Reason),
}

/// Accumulates a pending count and key sequence across calls to
/// [`Resolver::handle_key`] until a binding matches, a sequence rejects, or
/// `clear` is called (e.g. on `Esc`).
pub struct Resolver {
    keymap: Keymap,
    pending_count: Option<usize>,
    pending_keys: Vec<Key>,
}

impl Resolver {
    pub fn new(keymap: Keymap) -> Self {
        Resolver {
            keymap,
            pending_count: None,
            pending_keys: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.pending_count = None;
        self.pending_keys.clear();
    }

    pub fn handle_key(&mut self, key: Key) -> Resolution {
        if let Key::Char(c) = key {
            if c.is_ascii_digit() && (c != '0' || self.pending_count.is_some()) {
                let digit = c.to_digit(10).unwrap() as usize;
                self.pending_count = Some(self.pending_count.unwrap_or(0) * 10 + digit);
                return Resolution::Pending;
            }
        }

        self.pending_keys.push(key);
        let seq: String = self.pending_keys.iter().map(|k| k.token()).collect();

        match self.keymap.lookup(&seq) {
            Lookup::Match(kind) => {
                let count = self.pending_count.take();
                self.pending_keys.clear();
                Resolution::Resolved(resolve_action(kind, count))
            }
            Lookup::Prefix => Resolution::Pending,
            Lookup::None => {
                self.clear();
                Resolution::Rejected(Reason(seq))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keymap() -> Keymap {
        Keymap(HashMap::from([
            ("j".to_string(), ActionKind::MoveDown),
            ("k".to_string(), ActionKind::MoveUp),
            ("gg".to_string(), ActionKind::GoTop),
        ]))
    }

    #[test]
    fn mode_action_keymap_round_trip_serde() {
        let mode = Mode::Visual;
        let mode2: Mode = serde_json::from_str(&serde_json::to_string(&mode).unwrap()).unwrap();
        assert_eq!(mode, mode2);

        let action = Action::GoToRow(12);
        let action2: Action =
            serde_json::from_str(&serde_json::to_string(&action).unwrap()).unwrap();
        assert_eq!(action, action2);

        let keymap = test_keymap();
        let keymap2: Keymap =
            serde_json::from_str(&serde_json::to_string(&keymap).unwrap()).unwrap();
        assert_eq!(keymap.0, keymap2.0);
    }

    #[test]
    fn count_prefix_parses() {
        let mut resolver = Resolver::new(test_keymap());
        assert_eq!(resolver.handle_key(Key::Char('5')), Resolution::Pending);
        assert_eq!(
            resolver.handle_key(Key::Char('j')),
            Resolution::Resolved(Action::MoveDown(5))
        );
    }

    #[test]
    fn prefix_key_yields_pending() {
        let mut resolver = Resolver::new(test_keymap());
        assert_eq!(resolver.handle_key(Key::Char('g')), Resolution::Pending);
    }

    #[test]
    fn unmatched_sequence_rejected_and_clears_pending() {
        let mut resolver = Resolver::new(test_keymap());
        assert_eq!(resolver.handle_key(Key::Char('g')), Resolution::Pending);
        assert_eq!(
            resolver.handle_key(Key::Char('x')),
            Resolution::Rejected(Reason("gx".to_string()))
        );
        // pending state was cleared by the rejection, not carried forward
        assert_eq!(
            resolver.handle_key(Key::Char('j')),
            Resolution::Resolved(Action::MoveDown(1))
        );
    }

    // P3.2 — resolver state machine, against the five test groups in
    // docs/plan/phase-3-tui.md. P3.1 already implemented `Resolver` in full;
    // these are the additional cases its own four tests didn't cover.

    fn motion_keymap() -> Keymap {
        Keymap(HashMap::from([
            ("j".to_string(), ActionKind::MoveDown),
            ("gg".to_string(), ActionKind::GoTop),
            ("G".to_string(), ActionKind::GoToRowOrBottom),
            ("esc".to_string(), ActionKind::LeaveMode),
        ]))
    }

    fn mode_keymap() -> Keymap {
        Keymap(HashMap::from([
            ("v".to_string(), ActionKind::EnterVisual),
            ("esc".to_string(), ActionKind::LeaveMode),
            (":".to_string(), ActionKind::EnterCommand),
            ("/".to_string(), ActionKind::EnterSearch),
        ]))
    }

    #[test]
    fn count_prefix_motions_resolve() {
        let mut resolver = Resolver::new(motion_keymap());
        assert_eq!(
            resolver.handle_key(Key::Char('j')),
            Resolution::Resolved(Action::MoveDown(1))
        );
        assert_eq!(resolver.handle_key(Key::Char('5')), Resolution::Pending);
        assert_eq!(
            resolver.handle_key(Key::Char('j')),
            Resolution::Resolved(Action::MoveDown(5))
        );
        assert_eq!(resolver.handle_key(Key::Char('1')), Resolution::Pending);
        assert_eq!(resolver.handle_key(Key::Char('2')), Resolution::Pending);
        assert_eq!(
            resolver.handle_key(Key::Char('G')),
            Resolution::Resolved(Action::GoToRow(12))
        );
    }

    #[test]
    fn g_prefix_state_machine() {
        let mut resolver = Resolver::new(motion_keymap());
        assert_eq!(resolver.handle_key(Key::Char('g')), Resolution::Pending);
        assert_eq!(
            resolver.handle_key(Key::Char('g')),
            Resolution::Resolved(Action::GoTop)
        );
        assert_eq!(resolver.handle_key(Key::Char('g')), Resolution::Pending);
        assert_eq!(
            resolver.handle_key(Key::Char('x')),
            Resolution::Rejected(Reason("gx".to_string()))
        );
    }

    #[test]
    fn esc_clears_pending_state_from_any_partial_sequence() {
        let mut resolver = Resolver::new(motion_keymap());

        // partial count
        assert_eq!(resolver.handle_key(Key::Char('5')), Resolution::Pending);
        assert_eq!(
            resolver.handle_key(Key::Esc),
            Resolution::Resolved(Action::LeaveMode)
        );
        assert_eq!(
            resolver.handle_key(Key::Char('j')),
            Resolution::Resolved(Action::MoveDown(1)),
            "the count from before Esc must not survive"
        );

        // partial key-sequence prefix
        assert_eq!(resolver.handle_key(Key::Char('g')), Resolution::Pending);
        resolver.handle_key(Key::Esc); // "gesc" is unbound -> Rejected, but clears either way
        assert_eq!(
            resolver.handle_key(Key::Char('j')),
            Resolution::Resolved(Action::MoveDown(1)),
            "the g prefix from before Esc must not survive"
        );
    }

    #[test]
    fn mode_transitions() {
        let mut resolver = Resolver::new(mode_keymap());
        assert_eq!(
            resolver.handle_key(Key::Char('v')),
            Resolution::Resolved(Action::EnterMode(Mode::Visual))
        );
        assert_eq!(
            resolver.handle_key(Key::Esc),
            Resolution::Resolved(Action::LeaveMode)
        );
        assert_eq!(
            resolver.handle_key(Key::Char(':')),
            Resolution::Resolved(Action::EnterMode(Mode::Command))
        );
        assert_eq!(
            resolver.handle_key(Key::Char('/')),
            Resolution::Resolved(Action::EnterMode(Mode::Insert))
        );
    }

    #[test]
    fn count_with_no_following_key_remains_pending_indefinitely() {
        let mut resolver = Resolver::new(motion_keymap());
        assert_eq!(resolver.handle_key(Key::Char('1')), Resolution::Pending);
        assert_eq!(resolver.handle_key(Key::Char('2')), Resolution::Pending);
        assert_eq!(resolver.handle_key(Key::Char('3')), Resolution::Pending);
        assert_eq!(
            resolver.handle_key(Key::Char('j')),
            Resolution::Resolved(Action::MoveDown(123)),
            "the accumulated count survives arbitrarily many pending digits"
        );
    }
}
