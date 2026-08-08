use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use bam_server::{AppState, app};

fn default_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(format!("{home}/.local/share/bam/bam.db"))
}

#[tokio::main]
async fn main() {
    let db_path = std::env::var("BAM_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_db_path());
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let port: u16 = std::env::var("BAM_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let state = Arc::new(AppState::new(db_path));
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bam-server: bind");
    println!("bam-server listening on http://{addr}");
    axum::serve(listener, app(state)).await.expect("serve");
}
