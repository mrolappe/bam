//! P7.4: batched, resumable readme embedding plus the `Similar` predicate
//! it feeds. A fake provider drives every scenario (invariant I8: no
//! network in the default run).

use std::collections::HashMap;
use std::sync::Mutex;

use bam_core::llm::{Capabilities, CompletionRequest, GrammarSupport, LlmError, LlmProvider};
use bam_core::query::ir::Predicate;
use bam_core::query::registry::{FieldRegistry, package_fields};
use bam_core::store::compile::compile;
use bam_core::store::embeddings::{EmbedError, run_batch};
use bam_core::store::fts::rebuild_fts;
use bam_core::store::tables::{self, LandingReadme, Package};
use rusqlite::Connection;

/// Embeds any text present in `exact` to its hand-assigned vector; any
/// other text to a small deterministic vector derived from its length, so
/// tests that only care about call counts don't need to hand-craft one.
struct FakeProvider {
    calls: Mutex<usize>,
    dim: usize,
    exact: HashMap<String, Vec<f32>>,
}

impl FakeProvider {
    fn new(dim: usize) -> Self {
        Self {
            calls: Mutex::new(0),
            dim,
            exact: HashMap::new(),
        }
    }

    fn with_vector(mut self, text: &str, vector: Vec<f32>) -> Self {
        self.exact.insert(text.to_string(), vector);
        self
    }

    fn embed_one(&self, text: &str) -> Vec<f32> {
        if let Some(v) = self.exact.get(text) {
            return v.clone();
        }
        let seed = text.len() as f32;
        (0..self.dim).map(|i| seed + i as f32).collect()
    }

    fn call_count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl LlmProvider for FakeProvider {
    async fn complete(&self, _req: CompletionRequest) -> Result<String, LlmError> {
        unimplemented!("embedding only calls embed")
    }

    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, LlmError> {
        *self.calls.lock().unwrap() += 1;
        Ok(texts.iter().map(|t| self.embed_one(t)).collect())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            grammar: GrammarSupport::None,
            context_size: None,
        }
    }
}

fn package_with_readme(conn: &Connection, landing_id: i64, dir: &str, readme_text: &str) -> i64 {
    let id = tables::insert_package(
        conn,
        &Package {
            id: 0,
            dir: dir.to_string(),
            file: "pkg.lha".to_string(),
            name: "pkg".to_string(),
            version: None,
            size_bytes: None,
            uploaded_on: None,
            date_precision: "exact".to_string(),
            description: None,
            landing_id,
        },
    )
    .unwrap();
    tables::insert_landing_readme(
        conn,
        &LandingReadme {
            id: 0,
            package_id: id,
            url: format!("test://{dir}"),
            fetched_at: "2026-01-01T00:00:00Z".to_string(),
            raw: readme_text.as_bytes().to_vec(),
            detected_encoding: "UTF-8".to_string(),
        },
    )
    .unwrap();
    id
}

fn landing_id(conn: &Connection) -> i64 {
    tables::insert_landing_index_line(
        conn,
        &tables::LandingIndexLine {
            id: 0,
            fetched_at: "2026-01-01T00:00:00Z".into(),
            source_url: "test://fixture".into(),
            line_no: 1,
            raw: vec![],
        },
    )
    .unwrap()
}

#[tokio::test]
async fn embedding_is_batched_far_fewer_calls_than_packages() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing = landing_id(&conn);
    for i in 0..100 {
        package_with_readme(
            &conn,
            landing,
            &format!("dir/pkg{i}"),
            &format!("readme text {i}"),
        );
    }

    let provider = FakeProvider::new(4);
    let mut total_embedded = 0;
    loop {
        let outcome = run_batch(&conn, &provider, "fake-model", 20).await.unwrap();
        if outcome.embedded == 0 {
            break;
        }
        total_embedded += outcome.embedded;
    }

    assert_eq!(total_embedded, 100);
    assert_eq!(provider.call_count(), 5, "100 packages / batch of 20");
    assert!(provider.call_count() < 100);
}

#[tokio::test]
async fn an_interrupted_run_resumes_without_re_embedding() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing = landing_id(&conn);
    let ids: Vec<i64> = (0..10)
        .map(|i| {
            package_with_readme(
                &conn,
                landing,
                &format!("dir/pkg{i}"),
                &format!("readme {i}"),
            )
        })
        .collect();

    // "Interrupted": only one batch runs, covering half the backlog.
    let provider = FakeProvider::new(4);
    let first = run_batch(&conn, &provider, "fake-model", 5).await.unwrap();
    assert_eq!(first.embedded, 5);
    let embedded_after_first: usize = ids
        .iter()
        .filter(|id| {
            tables::get_package_embedding(&conn, **id)
                .unwrap()
                .is_some()
        })
        .count();
    assert_eq!(embedded_after_first, 5);

    // "Resumed": a fresh call only picks up the remainder, never the five
    // already-embedded packages (a re-embed would double the call's work,
    // which a call-count assertion can't see but a completeness one can).
    let second = run_batch(&conn, &provider, "fake-model", 5).await.unwrap();
    assert_eq!(second.embedded, 5);
    let third = run_batch(&conn, &provider, "fake-model", 5).await.unwrap();
    assert_eq!(third.embedded, 0, "nothing left to embed");

    for id in &ids {
        assert!(tables::get_package_embedding(&conn, *id).unwrap().is_some());
    }
    assert_eq!(provider.call_count(), 2);
}

#[tokio::test]
async fn a_dimension_change_is_reported_not_silently_stored() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing = landing_id(&conn);
    package_with_readme(&conn, landing, "dir/a", "first readme");
    package_with_readme(&conn, landing, "dir/b", "second readme");

    let small = FakeProvider::new(3);
    run_batch(&conn, &small, "model-v1", 1).await.unwrap();

    // A "model switch": the second package is embedded with a different
    // dimensionality than what's already stored.
    let big = FakeProvider::new(4);
    let err = run_batch(&conn, &big, "model-v2", 1).await.unwrap_err();
    assert!(
        matches!(
            err,
            EmbedError::DimensionMismatch {
                expected: 3,
                actual: 4
            }
        ),
        "got: {err:?}"
    );
}

#[tokio::test]
async fn similar_finds_a_semantic_match_that_keyword_search_misses() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing = landing_id(&conn);

    let query = "amiga chip graphics showcase";
    let target_text = "a demo coded for the boing ball effect using blitter tricks";
    let decoy_text = "amiga chip graphics showcase parody sketch, nothing technical";
    let unrelated_text = "a text editor for AmigaDOS scripts";

    let target = package_with_readme(&conn, landing, "demo/boing", target_text);
    let decoy = package_with_readme(&conn, landing, "demo/decoy", decoy_text);
    let _unrelated = package_with_readme(&conn, landing, "util/editor", unrelated_text);

    let provider = FakeProvider::new(3)
        .with_vector(query, vec![1.0, 0.0, 0.0])
        .with_vector(target_text, vec![0.99, 0.01, 0.0])
        .with_vector(decoy_text, vec![0.0, 1.0, 0.0])
        .with_vector(unrelated_text, vec![0.0, 0.0, 1.0]);

    let mut embedded = 0;
    loop {
        let outcome = run_batch(&conn, &provider, "fake-model", 10).await.unwrap();
        if outcome.embedded == 0 {
            break;
        }
        embedded += outcome.embedded;
    }
    assert_eq!(embedded, 3);

    // A literal keyword search for the query phrase misses the semantic
    // match: `target_text` shares no words with `query`.
    rebuild_fts(&conn).unwrap();
    let reg = FieldRegistry::new(package_fields());
    let keyword = compile(
        &Predicate::FullText(query.to_string()),
        &reg,
        None,
        &HashMap::new(),
    )
    .unwrap();
    let mut stmt = conn.prepare(&keyword.sql).unwrap();
    let keyword_ids: Vec<i64> = stmt
        .query_map(rusqlite::params_from_iter(keyword.params.iter()), |r| {
            r.get(0)
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(
        !keyword_ids.contains(&target),
        "keyword search for the exact query phrase should miss the semantic-only match"
    );

    // `Similar` finds it, and excludes the keyword-matching decoy whose
    // embedding is far away.
    let pred = Predicate::Similar {
        text: query.to_string(),
        threshold: 0.9,
    };
    let mut vectors = HashMap::new();
    vectors.insert(query.to_string(), vec![1.0, 0.0, 0.0]);
    let compiled = compile(&pred, &reg, None, &vectors).unwrap();
    let mut stmt = conn.prepare(&compiled.sql).unwrap();
    let similar_ids: Vec<i64> = stmt
        .query_map(rusqlite::params_from_iter(compiled.params.iter()), |r| {
            r.get(0)
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();

    assert_eq!(similar_ids, vec![target]);
    assert!(!similar_ids.contains(&decoy));
}
