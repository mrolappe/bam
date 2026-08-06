use std::process::ExitCode;

use bam_core::http::ReqwestClient;
use bam_core::progress::{OperationId, Outcome, ProgressEvent, ProgressSink};
use bam_core::store;
use bam_core::store::ingest::{IngestMode, run_ingest};

struct CliProgress;

impl ProgressSink for CliProgress {
    fn emit(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::Started { total, .. } => match total {
                Some(n) => eprintln!("ingest: starting ({n} steps)"),
                None => eprintln!("ingest: starting"),
            },
            ProgressEvent::Advanced { done, .. } => eprintln!("ingest: {done} done"),
            ProgressEvent::Finished {
                outcome: Outcome::Success,
                ..
            } => eprintln!("ingest: done"),
            ProgressEvent::Finished {
                outcome: Outcome::Failed { message },
                ..
            } => eprintln!("ingest: failed: {message}"),
        }
    }
}

fn default_db_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    format!("{home}/.local/share/bam/bam.db")
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("ingest") => ingest(&args[1..]).await,
        _ => {
            println!("bam {}", bam_core::version());
            ExitCode::SUCCESS
        }
    }
}

async fn ingest(flags: &[String]) -> ExitCode {
    let offline = flags.iter().any(|a| a == "--offline");
    let rebuild = flags.iter().any(|a| a == "--rebuild-normalized");
    let db_path = flags
        .iter()
        .position(|a| a == "--db")
        .and_then(|i| flags.get(i + 1))
        .cloned()
        .unwrap_or_else(default_db_path);

    let mode = if rebuild {
        IngestMode::RebuildNormalized
    } else if offline {
        IngestMode::Offline
    } else {
        IngestMode::Fetch
    };

    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let conn = match store::open(&db_path) {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("failed to open {db_path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let client = ReqwestClient::new();
    let mut sink = CliProgress;
    let fetched_at = bam_core::now_rfc3339();

    match run_ingest(&conn, &client, &mut sink, mode, &fetched_at, OperationId(0)).await {
        Ok(outcome) => {
            println!("{} packages", outcome.package_count);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("ingest failed: {e}");
            ExitCode::FAILURE
        }
    }
}
