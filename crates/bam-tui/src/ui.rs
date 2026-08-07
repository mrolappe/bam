//! Three-pane layout (P3.4): query line, package list, detail pane. Draws
//! only [`App::visible`]'s already-loaded window — never re-queries.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Modifier;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::App;
use crate::store::PackageStore;
use crate::tokens;

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
    } else if !app.command_text().is_empty() {
        frame.render_widget(Paragraph::new(format!(":{}", app.command_text())), rows[1]);
    } else if let Some(status) = app.status() {
        frame.render_widget(Paragraph::new(status.to_string()), rows[1]);
    } else if !app.highlight_errors().is_empty() {
        // Lowest-priority row content: a highlight rule that failed to
        // resolve (P3.8) is reported here rather than aborting the reload
        // or disabling the other rules.
        frame.render_widget(Paragraph::new(app.highlight_errors().join("; ")), rows[1]);
    }

    let start = app.window_start();
    let items: Vec<ListItem> = app
        .visible()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let row = app.row_tokens(i);
            let gutter: String = row.gutters.iter().map(|g| tokens::gutter_char(g)).collect();
            let marker = if gutter.is_empty() {
                String::new()
            } else {
                format!("{gutter} ")
            };
            let badges: String = row
                .badges
                .iter()
                .map(|b| format!(" [{}]", tokens::badge_text(b)))
                .collect();
            let label = format!("{marker}{}/{}{badges}", p.dir, p.file);
            let mut style = row
                .background
                .as_deref()
                .map(tokens::background_style)
                .unwrap_or_default();
            if start + i == app.cursor() {
                style = style.add_modifier(Modifier::REVERSED);
            }
            ListItem::new(label).style(style)
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

    if let Some(keymap) = app.help_bindings() {
        let mut lines: Vec<String> = keymap
            .0
            .iter()
            .map(|(token, kind)| {
                let name = serde_json::to_value(kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                format!("{token:<8} {name}")
            })
            .collect();
        lines.sort();
        let overlay = Paragraph::new(lines.join("\n")).block(
            Block::default()
                .borders(Borders::ALL)
                .title("help (esc/q to close)"),
        );
        frame.render_widget(overlay, frame.area());
    }
}
