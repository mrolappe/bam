//! P7.5: batched, resumable `llm_summary` enrichment. A fake provider
//! drives every scenario (invariant I8: no network in the default run).

use bam_core::llm::{Capabilities, CompletionRequest, GrammarSupport, LlmError, LlmProvider};
use bam_core::store::summaries::{SummaryError, estimate_run, run_batch};
use bam_core::store::tables::{self, Enrichment, LandingReadme, Package};
use rusqlite::Connection;
use std::sync::Mutex;

/// Summarizes to a canned string, or errors for texts marked to fail.
struct FakeProvider {
    calls: Mutex<usize>,
    fail_on: Vec<String>,
}

impl FakeProvider {
    fn new() -> Self {
        Self {
            calls: Mutex::new(0),
            fail_on: Vec::new(),
        }
    }

    fn failing_on(mut self, prompt_substring: &str) -> Self {
        self.fail_on.push(prompt_substring.to_string());
        self
    }

    fn call_count(&self) -> usize {
        *self.calls.lock().unwrap()
    }
}

impl LlmProvider for FakeProvider {
    async fn complete(&self, req: CompletionRequest) -> Result<String, LlmError> {
        *self.calls.lock().unwrap() += 1;
        if self.fail_on.iter().any(|s| req.prompt.contains(s)) {
            return Err(LlmError::Http("simulated failure".to_string()));
        }
        Ok(format!("summary of: {}", req.prompt.len()))
    }

    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, LlmError> {
        unimplemented!("summaries only call complete")
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
async fn an_interrupted_run_resumes_without_re_summarizing() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing = landing_id(&conn);
    let ids: Vec<i64> = (0..100)
        .map(|i| {
            package_with_readme(
                &conn,
                landing,
                &format!("dir/pkg{i}"),
                &format!("readme {i}"),
            )
        })
        .collect();

    let provider = FakeProvider::new();
    let first = run_batch(&conn, &provider, None, 40, true).await.unwrap();
    assert_eq!(first.summarized, 40);

    let second = run_batch(&conn, &provider, None, 40, true).await.unwrap();
    assert_eq!(second.summarized, 40);
    let third = run_batch(&conn, &provider, None, 40, true).await.unwrap();
    assert_eq!(
        third.summarized, 20,
        "only the remainder, not re-summarized"
    );
    let fourth = run_batch(&conn, &provider, None, 40, true).await.unwrap();
    assert_eq!(fourth.summarized, 0, "nothing left pending");

    for id in &ids {
        assert!(tables::get_enrichment(&conn, *id, "llm_summary").is_ok());
    }
}

#[tokio::test]
async fn bumping_producer_version_reprocesses_leaving_it_does_not() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing = landing_id(&conn);
    let id = package_with_readme(&conn, landing, "dir/a", "readme text");

    let provider = FakeProvider::new();
    run_batch(&conn, &provider, None, 10, true).await.unwrap();
    assert_eq!(provider.call_count(), 1);

    // Leaving producer_version alone: already up to date, no reprocessing.
    let again = run_batch(&conn, &provider, None, 10, true).await.unwrap();
    assert_eq!(again.summarized, 0);
    assert_eq!(provider.call_count(), 1);

    // Bumping producer_version: stale, gets reprocessed.
    tables::upsert_enrichment(
        &conn,
        &Enrichment {
            package_id: id,
            kind: "llm_summary".to_string(),
            producer_version: 0, // older than SUMMARY_PRODUCER_VERSION (1)
            produced_at: "2026-01-01T00:00:00Z".to_string(),
            payload: "stale".to_string(),
        },
    )
    .unwrap();
    let reprocessed = run_batch(&conn, &provider, None, 10, true).await.unwrap();
    assert_eq!(reprocessed.summarized, 1);
    assert_eq!(provider.call_count(), 2);
}

#[tokio::test]
async fn estimate_reports_before_starting_and_run_requires_confirmation() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing = landing_id(&conn);
    package_with_readme(&conn, landing, "dir/a", "some readme text");
    package_with_readme(&conn, landing, "dir/b", "some other readme text");

    let estimate = estimate_run(&conn, None, Some(2.0)).unwrap();
    assert_eq!(estimate.packages, 2);
    assert!(estimate.estimated_tokens > 0);
    assert_eq!(
        estimate.estimated_cost,
        Some(estimate.estimated_tokens as f64 / 1000.0 * 2.0)
    );

    // Local/free providers: no price given, no cost estimate.
    let free_estimate = estimate_run(&conn, None, None).unwrap();
    assert_eq!(free_estimate.estimated_cost, None);

    let provider = FakeProvider::new();
    let err = run_batch(&conn, &provider, None, 10, false)
        .await
        .unwrap_err();
    assert!(matches!(err, SummaryError::ConfirmationRequired));
    assert_eq!(
        provider.call_count(),
        0,
        "unconfirmed run must not call the provider"
    );

    let ok = run_batch(&conn, &provider, None, 10, true).await.unwrap();
    assert_eq!(ok.summarized, 2);
}

#[tokio::test]
async fn summarising_a_selection_touches_only_its_members() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing = landing_id(&conn);
    let a = package_with_readme(&conn, landing, "dir/a", "readme a");
    let b = package_with_readme(&conn, landing, "dir/b", "readme b");
    let _c = package_with_readme(&conn, landing, "dir/c", "readme c");

    let provider = FakeProvider::new();
    let outcome = run_batch(&conn, &provider, Some(&[a, b]), 10, true)
        .await
        .unwrap();
    assert_eq!(outcome.summarized, 2);

    assert!(tables::get_enrichment(&conn, a, "llm_summary").is_ok());
    assert!(tables::get_enrichment(&conn, b, "llm_summary").is_ok());
    assert!(
        tables::get_enrichment(&conn, _c, "llm_summary").is_err(),
        "non-member must be untouched"
    );
}

#[tokio::test]
async fn a_provider_error_on_one_package_does_not_abort_the_batch() {
    let conn = bam_core::store::open(":memory:").unwrap();
    let landing = landing_id(&conn);
    let good_a = package_with_readme(&conn, landing, "dir/a", "good readme a");
    let bad = package_with_readme(&conn, landing, "dir/bad", "poison readme");
    let good_b = package_with_readme(&conn, landing, "dir/b", "good readme b");

    let provider = FakeProvider::new().failing_on("poison readme");
    let outcome = run_batch(&conn, &provider, None, 10, true).await.unwrap();

    assert_eq!(outcome.summarized, 2);
    assert_eq!(outcome.failed.len(), 1);
    assert_eq!(outcome.failed[0].0, bad);
    assert!(tables::get_enrichment(&conn, good_a, "llm_summary").is_ok());
    assert!(tables::get_enrichment(&conn, good_b, "llm_summary").is_ok());
    assert!(tables::get_enrichment(&conn, bad, "llm_summary").is_err());
}
