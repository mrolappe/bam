//! The `QueryLanguage` trait (invariant I4's reference registry): a surface
//! syntax is anything that parses to the IR (`super::ir::Predicate`) and,
//! optionally, renders back to text or emits a grammar for constrained LLM
//! generation. `render` and `grammar` return `Option` because not every
//! language can round-trip or be constrained — that's a capability, not a
//! failure.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::ir::Predicate;
use super::registry::FieldRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrammarKind {
    Gbnf,
    JsonSchema,
}

/// Carries a byte span from the start: P2.4's bam-dsl parser requires one so
/// a generated query can be shown to the user and corrected (§11), and
/// retrofitting a span onto every implementor after the trait is registered
/// would be a breaking change to the one thing pluggability was meant to
/// avoid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("{message}")]
pub struct ParseError {
    pub message: String,
    /// Byte offsets `[start, end)` into the source, when the error can be
    /// pinned to a span.
    pub span: Option<(usize, usize)>,
}

pub trait QueryLanguage {
    fn id(&self) -> &str;
    fn parse(&self, src: &str, reg: &FieldRegistry) -> Result<Predicate, ParseError>;
    fn render(&self, p: &Predicate) -> Option<String>;
    fn grammar(&self, kind: GrammarKind) -> Option<String>;
}

#[derive(Debug, Error)]
pub enum LanguageError {
    #[error("unknown query language '{requested}'; available: {available}")]
    UnknownLanguage {
        requested: String,
        available: String,
    },
}

pub struct LanguageRegistry {
    languages: Vec<Box<dyn QueryLanguage>>,
    default_id: String,
}

impl LanguageRegistry {
    pub fn new(default_id: impl Into<String>) -> Self {
        Self {
            languages: Vec::new(),
            default_id: default_id.into(),
        }
    }

    pub fn register(&mut self, lang: Box<dyn QueryLanguage>) {
        self.languages.push(lang);
    }

    /// Resolves `id`, falling back to the configured default when `None`.
    pub fn get(&self, id: Option<&str>) -> Result<&dyn QueryLanguage, LanguageError> {
        let wanted = id.unwrap_or(&self.default_id);
        self.languages
            .iter()
            .find(|l| l.id() == wanted)
            .map(|l| l.as_ref())
            .ok_or_else(|| LanguageError::UnknownLanguage {
                requested: wanted.to_string(),
                available: self
                    .languages
                    .iter()
                    .map(|l| l.id())
                    .collect::<Vec<_>>()
                    .join(", "),
            })
    }
}
