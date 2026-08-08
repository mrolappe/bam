# bam

Rust workspace: `bam-core` (library), `bam-tui`, `bam-server`, `bam-tauri`, plus a Vue `frontend`.

## Running the apps

**TUI** (`bam-tui`, binary `bam`) — terminal file browser:

```bash
cargo run -p bam-tui
```

Uses `~/.local/share/bam/bam.db` and `~/.config/bam/bam.toml` by default.

**Server** (`bam-server`) — standalone HTTP API backend (axum):

```bash
cargo run -p bam-server
# BAM_DB=<path> BAM_PORT=<port> to override defaults (db: ~/.local/share/bam/bam.db, port: 8080)
```

**Desktop app** (`bam-tauri`) — Tauri shell wrapping the Vue frontend + embedded server:

```bash
cd crates/bam-tauri && cargo tauri dev
```

Needs `cargo install tauri-cli` if not already installed. This runs `npm run dev --prefix ../../frontend` for you (Vite dev server on :5173) and opens the native window. For a production build: `cargo tauri build`.

**Frontend alone** (Vue, without Tauri):

```bash
cd frontend && npm run dev
```
