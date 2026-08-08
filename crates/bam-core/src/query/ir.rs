//! The query IR (invariant I2): the typed predicate tree every surface
//! query language compiles down to, and the SQL compiler (P2.5) compiles
//! from. Pure data — no database driver dependency, so ungated and
//! wasm32-safe — so the highlight engine, selections, and a future LLM
//! grammar generator can all consume it without pulling in a database.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A field name resolved through the [`crate::query::registry::FieldRegistry`].
/// An owned string, not a `&'static str`: languages parse field names from
/// arbitrary user input, and a `Predicate` must round-trip through serde
/// without borrowing from whatever produced it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct FieldId(pub String);

impl FieldId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Value {
    Text(String),
    Int(i64),
    /// ISO 8601 `YYYY-MM-DD`. Compared lexicographically, which is why the
    /// format is fixed rather than left to a language's own rendering.
    Date(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Pattern {
    Glob(String),
    Prefix(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SelectionRef {
    Named(String),
    /// The current working selection (`marked` in the DSL).
    Marked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Predicate {
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
    Not(Box<Predicate>),
    Compare {
        field: FieldId,
        op: CmpOp,
        value: Value,
    },
    /// Glob or prefix match — only permitted on `Text`-typed fields, checked
    /// by the registry at resolve time (see `registry::FieldRegistry`).
    Match {
        field: FieldId,
        pattern: Pattern,
    },
    /// A bareword: not resolved against any field, compiled to a
    /// `description LIKE` fallback until P4.6's FTS5 table exists.
    FullText(String),
    InSelection(SelectionRef),
    /// `threshold` is a minimum cosine similarity (P7.4). Compiles only if
    /// the caller resolves `text` to an embedding first — the compiler
    /// stays synchronous, so it can't call an `LlmProvider` itself
    /// (`store::compile::SimilarVectors`, `CompileError::MissingEmbedding`).
    Similar {
        text: String,
        threshold: f32,
    },
}
