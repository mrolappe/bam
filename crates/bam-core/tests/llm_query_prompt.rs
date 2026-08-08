//! P7.3 — the natural-language query prompt and its round trip back into
//! `bam-dsl` text. A fake provider drives every scenario except the
//! `#[ignore]`d real-model test (invariant I8: no network in the default
//! run).

use std::fs;
use std::sync::Mutex;

use bam_core::llm::{Capabilities, CompletionRequest, GrammarSupport, LlmError, LlmProvider};
use bam_core::query::bam_dsl::BamDsl;
use bam_core::query::lang::QueryLanguage;
use bam_core::query::registry::{FieldRegistry, package_fields};

/// Replays scripted completions in call order; records every prompt it
/// received so tests can assert on prompt content.
struct FakeProvider {
    grammar: GrammarSupport,
    completions: Mutex<Vec<String>>,
    prompts: Mutex<Vec<String>>,
}

impl FakeProvider {
    fn new(grammar: GrammarSupport, completions: Vec<&str>) -> Self {
        Self {
            grammar,
            completions: Mutex::new(completions.into_iter().rev().map(str::to_string).collect()),
            prompts: Mutex::new(Vec::new()),
        }
    }
}

impl LlmProvider for FakeProvider {
    async fn complete(&self, req: CompletionRequest) -> Result<String, LlmError> {
        self.prompts.lock().unwrap().push(req.prompt);
        Ok(self.completions.lock().unwrap().pop().expect("no scripted completion left"))
    }

    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, LlmError> {
        unimplemented!("query generation only calls complete")
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            grammar: self.grammar,
            context_size: Some(8192),
        }
    }
}

/// Parses `tree_sample.txt` (P1.1's real `TREE` fixture) into `(id,
/// description)` pairs the way a runtime TREE-ingest would, without adding
/// one just for this test.
fn tree_categories() -> Vec<(String, String)> {
    fs::read_to_string("tests/fixtures/tree_sample.txt")
        .unwrap()
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, char::is_whitespace);
            let id = parts.next()?.trim();
            let desc = parts.next()?.trim();
            (!id.is_empty()).then(|| (id.to_string(), desc.to_string()))
        })
        .collect()
}

fn registry() -> FieldRegistry {
    FieldRegistry::new(package_fields())
}

const NL_TO_DSL_CASES: &[(&str, &str)] = &[
    ("music from the nineties", "dir:mus/* year<2000"),
    ("large demos over 5 megabytes", "dir:demo/* size>5M"),
    ("utilities uploaded after 2015", "dir:util/* year>2015"),
    ("everything I've marked", "marked"),
    ("mentions Mustermann in the description", "description:~Mustermann"),
    ("games directory", "dir:game/*"),
    ("business software", "dir:biz/*"),
    ("anything not util", "!dir:util/*"),
    ("small files under 100k", "size<100k"),
    ("communications software from 1998", "dir:comm/* year:1998"),
];

#[tokio::test]
async fn ten_natural_language_queries_produce_valid_parseable_dsl() {
    let reg = registry();
    let lang = BamDsl;
    let cats = tree_categories();

    for (nl, canned_dsl) in NL_TO_DSL_CASES {
        let provider = FakeProvider::new(GrammarSupport::Gbnf, vec![canned_dsl]);
        let out = bam_core::llm::generate_query(&provider, &lang, &reg, &cats, nl)
            .await
            .unwrap_or_else(|e| panic!("query {nl:?} failed to generate: {e}"));

        // Round-trips: the rendered text parses again under the same
        // language and registry.
        lang.parse(&out, &reg)
            .unwrap_or_else(|e| panic!("rendered dsl {out:?} for {nl:?} failed to parse: {e}"));
    }
}

#[tokio::test]
async fn json_schema_path_round_trips_through_predicate() {
    let reg = registry();
    let lang = BamDsl;
    let cats = tree_categories();

    let json = serde_json::to_string(&bam_core::query::ir::Predicate::Compare {
        field: bam_core::query::ir::FieldId::new("year"),
        op: bam_core::query::ir::CmpOp::Lt,
        value: bam_core::query::ir::Value::Int(2000),
    })
    .unwrap();
    let provider = FakeProvider::new(GrammarSupport::JsonSchema, vec![&json]);

    let out = bam_core::llm::generate_query(&provider, &lang, &reg, &cats, "music from the nineties")
        .await
        .unwrap();

    assert_eq!(out, "year<2000");
}

#[tokio::test]
async fn prompt_includes_the_tree_category_vocabulary() {
    let reg = registry();
    let lang = BamDsl;
    let cats = tree_categories();
    let provider = FakeProvider::new(GrammarSupport::Gbnf, vec!["marked"]);

    bam_core::llm::generate_query(&provider, &lang, &reg, &cats, "anything")
        .await
        .unwrap();

    let prompt = provider.prompts.lock().unwrap()[0].clone();
    // A handful of real TREE categories, spot-checked rather than every one.
    assert!(prompt.contains("comm/irc"));
    assert!(prompt.contains("demo/aga"));
    assert!(prompt.contains("Internet Relay Chat"));
}

#[tokio::test]
async fn unparseable_model_output_is_a_clear_error_not_a_panic() {
    let reg = registry();
    let lang = BamDsl;
    let cats = tree_categories();
    let provider = FakeProvider::new(GrammarSupport::Gbnf, vec!["dir:( unbalanced"]);

    let err = bam_core::llm::generate_query(&provider, &lang, &reg, &cats, "anything")
        .await
        .unwrap_err();

    assert!(err.to_string().contains("could not be parsed"));
}

#[tokio::test]
#[ignore = "hits a real local llama.cpp/7B server; run explicitly, never in CI"]
async fn real_model_answers_ten_natural_language_queries() {
    use bam_core::http::ReqwestClient;
    use bam_core::llm::{OpenAiCompatibleConfig, OpenAiCompatibleProvider};

    let client = ReqwestClient::new();
    let provider = OpenAiCompatibleProvider::new(
        &client,
        OpenAiCompatibleConfig {
            base_url: "http://localhost:8080".into(),
            model: "local-model".into(),
            api_key: None,
            grammar: GrammarSupport::Gbnf,
            context_size: Some(8192),
        },
    );
    let reg = registry();
    let lang = BamDsl;
    let cats = tree_categories();

    for (nl, _) in NL_TO_DSL_CASES {
        let out = bam_core::llm::generate_query(&provider, &lang, &reg, &cats, nl)
            .await
            .unwrap_or_else(|e| panic!("query {nl:?} failed: {e}"));
        lang.parse(&out, &reg)
            .unwrap_or_else(|e| panic!("model output {out:?} for {nl:?} failed to parse: {e}"));
    }
}
