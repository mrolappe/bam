//! Token → ratatui presentation (P3.7): the one place a semantic token name
//! (`bam_core::highlight::Decoration`'s `gutter`/`badge`/`background`
//! strings, already conflict-resolved into a `RowTokens`) becomes a `Style`
//! or a gutter character. An unrecognized token renders unstyled rather
//! than panicking — a highlight rule naming a token this table hasn't
//! learned yet degrades gracefully instead of crashing the TUI.

use ratatui::style::{Color, Style};

pub fn background_style(token: &str) -> Style {
    match token {
        "accent-subtle" => Style::default().bg(Color::Rgb(40, 40, 60)),
        _ => Style::default(),
    }
}

pub fn gutter_char(token: &str) -> char {
    match token {
        "marked" => '*',
        "user" => 'u',
        "flag" => '!',
        _ => ' ',
    }
}

pub fn badge_text(token: &str) -> &str {
    match token {
        "XL" => "XL",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_token_renders_as_unstyled_rather_than_panicking() {
        assert_eq!(background_style("no-such-token"), Style::default());
        assert_eq!(gutter_char("no-such-token"), ' ');
        assert_eq!(badge_text("no-such-token"), "");
    }
}
