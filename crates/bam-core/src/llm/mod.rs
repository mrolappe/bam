//! `LlmProvider` trait (P7.1, §10). One implementation covers llama.cpp,
//! Ollama, and cloud OpenAI-compatible endpoints — they share a wire format,
//! and differ only in [`Capabilities`]. Plain, ungated code (invariant I1)
//! so it compiles to wasm32; it drives HTTP through P1.9's [`HttpClient`]
//! rather than a concrete client.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LlmError {
    /// The most common local-model failure: nothing listening at the
    /// configured URL. Named explicitly per P7.1's test list.
    #[error("could not reach {url} — is the server running?")]
    ConnectionFailed { url: String },
    #[error("llm request failed: {0}")]
    Http(String),
    #[error("llm returned an unparseable response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub prompt: String,
    /// GBNF grammar text, used only when [`Capabilities::grammar`] is
    /// [`GrammarSupport::Gbnf`]; ignored otherwise until P7.2 adds JSON
    /// Schema constraining.
    pub grammar: Option<String>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrammarSupport {
    Gbnf,
    JsonSchema,
    None,
}

#[derive(Debug, Clone, Copy)]
pub struct Capabilities {
    pub grammar: GrammarSupport,
    pub context_size: Option<u32>,
}

#[allow(async_fn_in_trait)]
pub trait LlmProvider {
    async fn complete(&self, req: CompletionRequest) -> Result<String, LlmError>;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, LlmError>;
    fn capabilities(&self) -> Capabilities;
}

mod openai_compatible;
pub use openai_compatible::{OpenAiCompatibleConfig, OpenAiCompatibleProvider};
