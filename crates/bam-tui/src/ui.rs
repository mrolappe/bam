//! Three-pane layout (P3.4): query line, package list, detail pane. Draws
//! only [`App::visible`]'s already-loaded window — never re-queries.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::App;
use crate::store::PackageStore;

const QUERY_PREFIX: &str = "search: ";

pub fn render(app: &App<impl PackageStore>, frame: &mut Frame) {
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(frame.area());
    let body =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).split(rows[2]);

    frame.render_widget(
        Paragraph::new(format!("{QUERY_PREFIX}{}", app.query_text())),
        rows[0],
    );

    if let Some(err) = app.query_error() {
        // Clamp to the last character so a span pointing one-past-the-end
        // (an operator with nothing after it, e.g. `size>`) still lands
        // under the last real character rather than one column past it.
        let last = app.query_text().len().saturating_sub(1);
        let col = err.span.map_or(0, |(start, _)| start.min(last));
        let marker = format!("{}^ {}", " ".repeat(QUERY_PREFIX.len() + col), err.message);
        frame.render_widget(Paragraph::new(marker), rows[1]);
    }

    let start = app.window_start();
    let items: Vec<ListItem> = app
        .visible()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let label = format!("{}/{}", p.dir, p.file);
            if start + i == app.cursor() {
                ListItem::new(label).style(Style::default().add_modifier(Modifier::REVERSED))
            } else {
                ListItem::new(label)
            }
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(format!(
        "packages ({}/{})",
        app.cursor() + 1,
        app.total()
    )));
    frame.render_widget(list, body[0]);

    let detail_text = match app.selected() {
        Some(p) => format!(
            "{}/{}\nversion: {}\nsize: {}\n\n{}",
            p.dir,
            p.file,
            p.version.as_deref().unwrap_or("-"),
            p.size_bytes.map_or("-".to_string(), |b| b.to_string()),
            p.description.as_deref().unwrap_or(""),
        ),
        None => String::new(),
    };
    let detail =
        Paragraph::new(detail_text).block(Block::default().borders(Borders::ALL).title("detail"));
    frame.render_widget(detail, body[1]);
}
