//! P7.3, §11: assembles the natural-language-to-`bam-dsl` prompt and routes
//! a provider's completion back through [`QueryLanguage`] into rendered,
//! editable DSL text.
//!
//! [`generate_query`] never touches `bam-core::store` or `bam-core::api` —
//! it returns a `String` for the caller to show and let the user edit
//! before anything runs, per §11's "always shown, always editable" rule.

use crate::query::ir::Predicate;
use crate::query::lang::{GrammarKind, QueryLanguage};
use crate::query::registry::FieldRegistry;

use super::{CompletionRequest, GrammarSupport, LlmError, LlmProvider};

/// A small fixed dictionary of Aminet's common archive/document extensions
/// — unlike categories, this doesn't vary per mirror snapshot, so it's
/// hardcoded rather than threaded in as configuration.
const FILE_TYPES: &[(&str, &str)] = &[
    ("lha", "LHA-compressed archive, the most common Aminet format"),
    ("lzx", "LZX-compressed archive"),
    ("zip", "ZIP archive"),
    ("readme", "plain-text description of the package"),
    ("txt", "plain text file"),
    ("dms", "DiskMasher disk image"),
    ("info", "Amiga Workbench icon"),
];

/// Few-shot examples, written against `registry::package_fields`'s actual
/// field set — not `bam-handoff.md`'s illustrative-only `type`/`author`
/// examples, which have no backing column yet.
const EXAMPLES: &[(&str, &str)] = &[
    ("music software from the nineties", "dir:mus/* year<2000"),
    ("large demos, over 5 megabytes", "dir:demo/* size>5M"),
    ("utilities uploaded after 2015", "dir:util/* year>2015"),
    ("everything I've marked", "marked"),
    (
        "anything mentioning Mustermann in the description",
        "description:~Mustermann",
    ),
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum QueryGenError {
    #[error("llm request failed: {0}")]
    Provider(#[from] LlmError),
    /// The model's output didn't parse as `bam-dsl` text or, on the
    /// JSON-Schema path, deserialize to a `Predicate` — a clear error
    /// instead of a panic, per P7.3's test list.
    #[error("model output could not be parsed as a query: {0}")]
    Unparseable(String),
}

/// Builds the prompt text: task instructions, the TREE category vocabulary,
/// the file-type dictionary, few-shot examples, then the user's request.
pub fn build_prompt(nl_query: &str, categories: &[(String, String)]) -> String {
    let mut prompt = String::from(
        "Translate the user's request into a single bam-dsl query. \
         Reply with only the query, no explanation.\n\n",
    );

    prompt.push_str("Categories (dir: values):\n");
    for (id, desc) in categories {
        prompt.push_str(&format!("  {id} - {desc}\n"));
    }

    prompt.push_str("\nFile types:\n");
    for (ext, desc) in FILE_TYPES {
        prompt.push_str(&format!("  {ext} - {desc}\n"));
    }

    prompt.push_str("\nExamples:\n");
    for (nl, dsl) in EXAMPLES {
        prompt.push_str(&format!("  \"{nl}\" -> {dsl}\n"));
    }

    prompt.push_str(&format!("\nRequest: \"{nl_query}\"\nQuery:"));
    prompt
}

/// Generates a `bam-dsl` query from a natural-language request. Returns
/// rendered, editable DSL text — the caller is responsible for showing it
/// to the user and running it only on confirmation; this function has no
/// access to anything that could run a query itself.
pub async fn generate_query<P: LlmProvider>(
    provider: &P,
    lang: &dyn QueryLanguage,
    reg: &FieldRegistry,
    categories: &[(String, String)],
    nl_query: &str,
) -> Result<String, QueryGenError> {
    let caps = provider.capabilities();
    let mut req = CompletionRequest {
        prompt: build_prompt(nl_query, categories),
        grammar: None,
        json_schema: None,
        max_tokens: None,
    };
    match caps.grammar {
        GrammarSupport::Gbnf => req.grammar = lang.grammar(GrammarKind::Gbnf),
        GrammarSupport::JsonSchema => req.json_schema = lang.grammar(GrammarKind::JsonSchema),
        GrammarSupport::None => {}
    }

    let completion = provider.complete(req).await?;

    let predicate = if caps.grammar == GrammarSupport::JsonSchema {
        serde_json::from_str::<Predicate>(&completion)
            .map_err(|e| QueryGenError::Unparseable(e.to_string()))?
    } else {
        lang.parse(&completion, reg)
            .map_err(|e| QueryGenError::Unparseable(e.to_string()))?
    };

    Ok(lang.render(&predicate).unwrap_or(completion))
}
