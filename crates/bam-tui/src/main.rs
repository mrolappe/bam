use std::io::Stdout;
use std::ops::ControlFlow;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use bam_core::api::Session;
use bam_core::http::ReqwestClient;
use bam_core::progress::{OperationId, Outcome, ProgressEvent, ProgressSink};
use bam_core::store;
use bam_core::store::ingest::{IngestMode, run_ingest};
use bam_tui::app::{App, all_packages};
use bam_tui::input::{
    Action, Key, KeymapConfig, Mode, Resolution, Resolver, default_keymap, merge_keymap,
};
use bam_tui::store::{SessionStore, StoreError};
use bam_tui::ui::render;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// The event-loop poll timeout: short enough that a pending debounced query
/// (P3.5, 150ms) fires promptly even with no further keypresses.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

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

fn default_config_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    format!("{home}/.config/bam/bam.toml")
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("ingest") => ingest(&args[1..]).await,
        Some("tui") => tui(&args[1..]),
        _ => {
            println!("bam {}", bam_core::version());
            ExitCode::SUCCESS
        }
    }
}

/// Loads the `[keys]` section of `bam.toml` if the config file exists,
/// merging it over [`default_keymap`]; a missing file yields exactly the
/// defaults (P3.3's fifth test bullet, re-applied here to real config
/// loading rather than a value built by hand).
fn load_keymap(flags: &[String]) -> bam_tui::input::Keymap {
    let config_path = flags
        .iter()
        .position(|a| a == "--config")
        .and_then(|i| flags.get(i + 1))
        .cloned()
        .unwrap_or_else(default_config_path);
    let config: KeymapConfig = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();
    match merge_keymap(default_keymap(), &config) {
        Ok(keymap) => keymap,
        Err(e) => {
            eprintln!("{config_path}: {e}, ignoring [keys] overrides");
            default_keymap()
        }
    }
}

fn to_input_key(code: KeyCode, modifiers: KeyModifiers) -> Option<Key> {
    match code {
        KeyCode::Esc => Some(Key::Esc),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::Char(c) if modifiers.contains(KeyModifiers::CONTROL) => Some(Key::Ctrl(c)),
        KeyCode::Char(c) => Some(Key::Char(c)),
        _ => None,
    }
}

/// `MoveDown`/`MoveUp`/`GoTop`/`GoBottom`/`Quit` are the only actions v1's
/// shell acts on — everything else (modes, marking, help) is later rounds'
/// scope (P3.5-P3.9) and is resolved but ignored for now.
fn apply_action(
    app: &mut App<SessionStore>,
    action: Action,
) -> Result<ControlFlow<()>, StoreError> {
    use ControlFlow::{Break, Continue};
    match action {
        Action::MoveDown(n) => app.move_down(n).map(|()| Continue(())),
        Action::MoveUp(n) => app.move_up(n).map(|()| Continue(())),
        Action::GoTop => app.go_top().map(|()| Continue(())),
        Action::GoBottom => app.go_bottom().map(|()| Continue(())),
        Action::Quit => Ok(Break(())),
        _ => Ok(Continue(())),
    }
}

/// While in [`Mode::Insert`] (entered via `/`), keys type into the query
/// line directly rather than resolving through the keymap — the same
/// distinction vim makes between insert and normal mode. `Esc` leaves back
/// to normal; everything else is either a character to append or backspace.
fn edit_query_line(
    app: &mut App<SessionStore>,
    key: Key,
    mode: &mut Mode,
    resolver: &mut Resolver,
) {
    match key {
        Key::Esc => {
            *mode = Mode::Normal;
            resolver.clear();
        }
        Key::Backspace => {
            let mut text = app.query_text().to_string();
            text.pop();
            app.edit_query(text, Instant::now());
        }
        Key::Char(c) => {
            let mut text = app.query_text().to_string();
            text.push(c);
            app.edit_query(text, Instant::now());
        }
        Key::Ctrl(_) => {}
    }
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App<SessionStore>,
    resolver: &mut Resolver,
) -> std::io::Result<()> {
    let mut mode = Mode::Normal;
    loop {
        terminal.draw(|frame| render(app, frame))?;
        if event::poll(POLL_INTERVAL)? {
            let Event::Key(key_event) = event::read()? else {
                continue;
            };
            if key_event.kind != KeyEventKind::Press {
                continue;
            }
            let Some(key) = to_input_key(key_event.code, key_event.modifiers) else {
                continue;
            };

            if mode == Mode::Insert {
                edit_query_line(app, key, &mut mode, resolver);
                continue;
            }

            if let Resolution::Resolved(action) = resolver.handle_key(key) {
                if action == Action::EnterMode(Mode::Insert) {
                    mode = Mode::Insert;
                    continue;
                }
                match apply_action(app, action) {
                    Ok(ControlFlow::Break(())) => return Ok(()),
                    Ok(ControlFlow::Continue(())) => {}
                    Err(e) => return Err(std::io::Error::other(e.to_string())),
                }
            }
        }
        app.tick(Instant::now())
            .map_err(|e| std::io::Error::other(e.to_string()))?;
    }
}

fn tui(flags: &[String]) -> ExitCode {
    let db_path = flags
        .iter()
        .position(|a| a == "--db")
        .and_then(|i| flags.get(i + 1))
        .cloned()
        .unwrap_or_else(default_db_path);
    if let Some(parent) = std::path::Path::new(&db_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let session = match Session::open(&db_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to open {db_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut app = match App::new(SessionStore::new(session), all_packages(), 20) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("failed to query packages: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut resolver = Resolver::new(load_keymap(flags));

    if enable_raw_mode().is_err() {
        eprintln!("failed to enable raw mode (is this a real terminal?)");
        return ExitCode::FAILURE;
    }
    let mut stdout = std::io::stdout();
    let _ = stdout.execute(EnterAlternateScreen);
    let result = Terminal::new(CrosstermBackend::new(stdout))
        .and_then(|mut terminal| run_loop(&mut terminal, &mut app, &mut resolver));

    let _ = disable_raw_mode();
    let _ = std::io::stdout().execute(LeaveAlternateScreen);

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tui error: {e}");
            ExitCode::FAILURE
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
