use serde_json::{Value, json};

use super::{Capabilities, CompletionRequest, GrammarSupport, LlmError, LlmProvider};
use crate::http::{HttpClient, HttpError, HttpPostRequest};

/// Default targets a local llama.cpp server; pointing at Ollama or a cloud
/// endpoint is a config change, nothing else (§10).
#[derive(Debug, Clone)]
pub struct OpenAiCompatibleConfig {
    pub base_url: String,
    pub model: String,
    pub api_key: Option<String>,
    pub grammar: GrammarSupport,
    pub context_size: Option<u32>,
}

pub struct OpenAiCompatibleProvider<'a, C: HttpClient> {
    client: &'a C,
    config: OpenAiCompatibleConfig,
}

impl<'a, C: HttpClient> OpenAiCompatibleProvider<'a, C> {
    pub fn new(client: &'a C, config: OpenAiCompatibleConfig) -> Self {
        Self { client, config }
    }

    fn headers(&self) -> Vec<(String, String)> {
        let mut headers = vec![(
            "Content-Type".to_string(),
            "application/json".to_string(),
        )];
        if let Some(key) = &self.config.api_key {
            headers.push(("Authorization".to_string(), format!("Bearer {key}")));
        }
        headers
    }

    async fn post(&self, path: &str, body: Value) -> Result<Value, LlmError> {
        let url = format!("{}{path}", self.config.base_url);
        let resp = self
            .client
            .post(HttpPostRequest {
                url: url.clone(),
                body: serde_json::to_vec(&body)
                    .map_err(|e| LlmError::InvalidResponse(e.to_string()))?,
                headers: self.headers(),
            })
            .await
            .map_err(|e| match e {
                // A response with a status code means the server is up;
                // only a transport-level failure looks like "not running".
                HttpError::Request(_) => LlmError::ConnectionFailed { url: url.clone() },
                HttpError::Status(code) => LlmError::Http(format!("unexpected status {code}")),
            })?;
        serde_json::from_slice(&resp.body).map_err(|e| LlmError::InvalidResponse(e.to_string()))
    }
}

impl<'a, C: HttpClient> LlmProvider for OpenAiCompatibleProvider<'a, C> {
    async fn complete(&self, req: CompletionRequest) -> Result<String, LlmError> {
        let mut body = json!({
            "model": self.config.model,
            "messages": [{"role": "user", "content": req.prompt}],
        });
        if let Some(max_tokens) = req.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        // JSON Schema constraining lands in P7.2; llama.cpp's GBNF is a
        // plain top-level field, so it can be wired through today.
        if let (Some(grammar), GrammarSupport::Gbnf) = (&req.grammar, self.config.grammar) {
            body["grammar"] = json!(grammar);
        }

        let resp = self.post("/v1/chat/completions", body).await?;
        resp["choices"][0]["message"]["content"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| {
                LlmError::InvalidResponse("missing choices[0].message.content".into())
            })
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, LlmError> {
        let body = json!({
            "model": self.config.model,
            "input": texts,
        });
        let resp = self.post("/v1/embeddings", body).await?;
        let data = resp["data"]
            .as_array()
            .ok_or_else(|| LlmError::InvalidResponse("missing data array".into()))?;

        let mut indexed: Vec<(usize, Vec<f32>)> = Vec::with_capacity(data.len());
        for (i, item) in data.iter().enumerate() {
            let index = item["index"].as_u64().map_or(i, |n| n as usize);
            let embedding = item["embedding"]
                .as_array()
                .ok_or_else(|| LlmError::InvalidResponse("missing embedding array".into()))?
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();
            indexed.push((index, embedding));
        }
        indexed.sort_by_key(|(index, _)| *index);
        Ok(indexed.into_iter().map(|(_, v)| v).collect())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            grammar: self.config.grammar,
            context_size: self.config.context_size,
        }
    }
}
