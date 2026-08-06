//! Three-pane layout (P3.4): query line, package list, detail pane. Draws
//! only [`App::visible`]'s already-loaded window — never re-queries.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::App;
use crate::store::PackageStore;

pub fn render(app: &App<impl PackageStore>, frame: &mut Frame) {
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(frame.area());
    let body =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).split(rows[1]);

    frame.render_widget(Paragraph::new("search: "), rows[0]);

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
