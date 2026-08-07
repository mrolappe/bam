//! Aminet readme header block parser.
//!
//! The block is semi-standardised: field order varies, some readmes omit it
//! entirely, capitalisation differs, and values wrap across lines. This
//! parses leniently — it extracts what it recognises and is infallible,
//! never failing a whole file over one bad or unknown field.

use serde::{Deserialize, Serialize};

/// `enrichment.kind` this parser's output is stored under.
pub const README_HEADER_KIND: &str = "readme_header";
/// `enrichment.producer_version` for this parser's output shape.
pub const README_HEADER_PRODUCER_VERSION: i64 = 1;

/// Base URL matching [`crate::store::ingest::INDEX_URL`]'s mirror.
pub const AMINET_BASE_URL: &str = "https://ftp.fau.de/aminet";

/// A package's readme URL: `{dir}/{stem}.readme`, the archive extension
/// stripped (confirmed against real Aminet data in Round 24/P4.5 — `{dir}/
/// {file}.readme` with the extension kept 404s).
pub fn readme_url(dir: &str, file: &str) -> String {
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem);
    format!("{AMINET_BASE_URL}/{dir}/{stem}.readme")
}

/// The readme header fields Aminet packages conventionally carry. Any other
/// field (`Architecture:`, misspellings like `Distrubution:`) is ignored.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReadmeHeader {
    pub short: Option<String>,
    pub author: Option<String>,
    pub uploader: Option<String>,
    pub r#type: Option<String>,
    pub version: Option<String>,
    pub requires: Option<String>,
    pub distribution: Option<String>,
}

impl ReadmeHeader {
    /// How many fields were recognised. Pinned in tests so a parser change
    /// shows up as a visible diff in expected counts, not a silent pass.
    pub fn field_count(&self) -> usize {
        self.fields().iter().filter(|(_, v)| v.is_some()).count()
    }

    fn fields(&self) -> [(&'static str, &Option<String>); 7] {
        [
            ("short", &self.short),
            ("author", &self.author),
            ("uploader", &self.uploader),
            ("type", &self.r#type),
            ("version", &self.version),
            ("requires", &self.requires),
            ("distribution", &self.distribution),
        ]
    }

    fn field_mut(&mut self, key: &str) -> Option<&mut Option<String>> {
        match key {
            "short" => Some(&mut self.short),
            "author" => Some(&mut self.author),
            "uploader" => Some(&mut self.uploader),
            "type" => Some(&mut self.r#type),
            "version" => Some(&mut self.version),
            "requires" => Some(&mut self.requires),
            "distribution" => Some(&mut self.distribution),
            _ => None,
        }
    }
}

/// A line of the form `Word[ Word...]:` — either a recognised header field or
/// an unrecognised one (e.g. `Architecture:`). Distinguishing this from a
/// wrapped continuation line is what lets wrapped values merge into the
/// right field instead of being dropped or bleeding into the next one.
fn field_line(line: &str) -> Option<(String, &str)> {
    let (name, rest) = line.split_once(':')?;
    let name = name.trim();
    let looks_like_a_field = !name.is_empty()
        && name.len() <= 30
        && name.chars().all(|c| c.is_ascii_alphabetic() || c == ' ');
    looks_like_a_field.then(|| (name.to_ascii_lowercase(), rest.trim()))
}

/// Parses the header block at the start of a readme's text. The block runs
/// from the first line to the first blank line; a readme with no header
/// (first line already blank, or not field-shaped) yields an empty result.
pub fn parse_readme_header(text: &str) -> ReadmeHeader {
    let mut header = ReadmeHeader::default();
    let mut open_field: Option<String> = None;

    for line in text.lines() {
        if line.trim().is_empty() {
            break;
        }
        match field_line(line) {
            Some((key, value)) => match header.field_mut(&key) {
                Some(slot) => {
                    if !value.is_empty() {
                        *slot = Some(value.to_string());
                    }
                    open_field = Some(key);
                }
                None => open_field = None,
            },
            None => {
                let Some(slot) = open_field.as_deref().and_then(|k| header.field_mut(k)) else {
                    continue;
                };
                *slot = Some(match slot.take() {
                    Some(existing) => format!("{existing} {}", line.trim()),
                    None => line.trim().to_string(),
                });
            }
        }
    }

    header
}
