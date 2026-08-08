//! P7.1 — `LlmProvider` trait + `OpenAiCompatibleProvider`. A fake client
//! drives every scenario except the `#[ignore]`d real-server test, so the
//! default run makes no network calls (invariant I8).

use std::sync::Mutex;

use bam_core::http::{HttpClient, HttpError, HttpPostRequest, HttpRequest, HttpResponse};
use bam_core::llm::{
    CompletionRequest, GrammarSupport, LlmError, LlmProvider, OpenAiCompatibleConfig,
    OpenAiCompatibleProvider,
};

/// Scripted client: replays queued POST responses in request order,
/// recording every request it received.
struct FakeClient {
    responses: Mutex<Vec<Result<HttpResponse, HttpError>>>,
    requests: Mutex<Vec<HttpPostRequest>>,
}

impl FakeClient {
    fn new(responses: Vec<Result<HttpResponse, HttpError>>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().rev().collect()),
            requests: Mutex::new(Vec::new()),
        }
    }
}

impl HttpClient for FakeClient {
    async fn get(&self, _req: HttpRequest) -> Result<HttpResponse, HttpError> {
        unimplemented!("llm provider only issues POST requests")
    }

    async fn post(&self, req: HttpPostRequest) -> Result<HttpResponse, HttpError> {
        self.requests.lock().unwrap().push(req);
        self.responses
            .lock()
            .unwrap()
            .pop()
            .expect("no scripted response left")
    }
}

fn ok_json(body: &str) -> Result<HttpResponse, HttpError> {
    Ok(HttpResponse {
        status: 200,
        body: body.as_bytes().to_vec(),
        etag: None,
    })
}

fn llama_cpp_config() -> OpenAiCompatibleConfig {
    OpenAiCompatibleConfig {
        base_url: "http://localhost:8080".into(),
        model: "local-model".into(),
        api_key: None,
        grammar: GrammarSupport::Gbnf,
        context_size: Some(8192),
    }
}

fn cloud_config() -> OpenAiCompatibleConfig {
    OpenAiCompatibleConfig {
        base_url: "https://api.example.com".into(),
        model: "gpt-4o-mini".into(),
        api_key: Some("sk-test".into()),
        grammar: GrammarSupport::JsonSchema,
        context_size: Some(128_000),
    }
}

#[tokio::test]
async fn same_code_path_completes_against_llama_cpp_and_cloud_fakes() {
    for config in [llama_cpp_config(), cloud_config()] {
        let client = FakeClient::new(vec![ok_json(
            r#"{"choices":[{"message":{"content":"hello from the model"}}]}"#,
        )]);
        let provider = OpenAiCompatibleProvider::new(&client, config);

        let out = provider
            .complete(CompletionRequest {
                prompt: "say hello".into(),
                grammar: None,
                json_schema: None,
                max_tokens: None,
            })
            .await
            .unwrap();

        assert_eq!(out, "hello from the model");
    }
}

#[tokio::test]
async fn capabilities_report_gbnf_for_llama_cpp_and_json_schema_for_cloud() {
    let client = FakeClient::new(vec![]);

    let llama = OpenAiCompatibleProvider::new(&client, llama_cpp_config());
    assert_eq!(llama.capabilities().grammar, GrammarSupport::Gbnf);

    let cloud = OpenAiCompatibleProvider::new(&client, cloud_config());
    assert_eq!(cloud.capabilities().grammar, GrammarSupport::JsonSchema);
}

#[tokio::test]
async fn json_schema_is_wired_into_response_format_for_cloud_only() {
    let schema = r#"{"type":"object"}"#;

    for (config, should_wire) in [(llama_cpp_config(), false), (cloud_config(), true)] {
        let client = FakeClient::new(vec![ok_json(
            r#"{"choices":[{"message":{"content":"ok"}}]}"#,
        )]);
        let provider = OpenAiCompatibleProvider::new(&client, config);

        provider
            .complete(CompletionRequest {
                prompt: "generate a query".into(),
                grammar: None,
                json_schema: Some(schema.to_string()),
                max_tokens: None,
            })
            .await
            .unwrap();

        let sent = client.requests.lock().unwrap().pop().unwrap();
        let body: serde_json::Value = serde_json::from_slice(&sent.body).unwrap();
        assert_eq!(
            body.get("response_format").is_some(),
            should_wire,
            "response_format presence for {should_wire}"
        );
    }
}

#[tokio::test]
async fn connection_failure_names_the_configured_url() {
    let client = FakeClient::new(vec![Err(HttpError::Request("connection refused".into()))]);
    let provider = OpenAiCompatibleProvider::new(&client, llama_cpp_config());

    let err = provider
        .complete(CompletionRequest {
            prompt: "say hello".into(),
            grammar: None,
            json_schema: None,
            max_tokens: None,
        })
        .await
        .unwrap_err();

    assert!(err.to_string().contains("running"));
    match err {
        LlmError::ConnectionFailed { url } => {
            assert_eq!(url, "http://localhost:8080/v1/chat/completions")
        }
        other => panic!("expected ConnectionFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn embedding_batch_returns_vectors_in_input_order() {
    let client = FakeClient::new(vec![ok_json(
        r#"{"data":[
            {"index":1,"embedding":[0.4,0.5]},
            {"index":0,"embedding":[0.1,0.2]},
            {"index":2,"embedding":[0.6,0.7]}
        ]}"#,
    )]);
    let provider = OpenAiCompatibleProvider::new(&client, llama_cpp_config());

    let texts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
    let vectors = provider.embed(&texts).await.unwrap();

    assert_eq!(
        vectors,
        vec![vec![0.1, 0.2], vec![0.4, 0.5], vec![0.6, 0.7]]
    );
}

#[tokio::test]
#[ignore = "hits a real local llama.cpp server; run explicitly, never in CI"]
async fn real_llama_cpp_completion() {
    use bam_core::http::ReqwestClient;

    let client = ReqwestClient::new();
    let provider = OpenAiCompatibleProvider::new(&client, llama_cpp_config());

    let out = provider
        .complete(CompletionRequest {
            prompt: "reply with exactly the word: pong".into(),
            grammar: None,
            json_schema: None,
            max_tokens: Some(16),
        })
        .await
        .unwrap();

    assert!(out.to_lowercase().contains("pong"));
}
