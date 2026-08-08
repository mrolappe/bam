//! AmigaGuide markup parser (P9.7, §13 of the handoff doc): `nom` into a
//! custom AST. No maintained Rust library exists for this format; it earns
//! `nom` where P2.4 didn't — many small, similar line-command and
//! inline-attribute productions.
//!
//! These are thirty-year-old hand-written files and some are simply broken:
//! unrecognised line commands are skipped, unmatched inline attributes fall
//! back to literal text, and unclosed styles at end-of-node are flushed
//! rather than dropped. Nothing here returns `Result` — a malformed document
//! degrades, it never fails to parse.

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_till},
    character::complete::{char, space1},
    combinator::map,
};

use super::charset::decode;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GuideDocument {
    pub database: Option<String>,
    pub nodes: Vec<GuideNode>,
}

impl GuideDocument {
    pub fn find_node(&self, name: &str) -> Option<&GuideNode> {
        self.nodes.iter().find(|n| n.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GuideNode {
    pub name: String,
    pub title: Option<String>,
    pub next: Option<String>,
    pub prev: Option<String>,
    pub toc: Option<String>,
    pub body: Vec<Inline>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Bold,
    Italic,
    Underline,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Styled(Style, Vec<Inline>),
    Link { text: String, target: String },
}

/// Decodes `bytes` (P1.5 — never assumed UTF-8) and parses the result as an
/// AmigaGuide document.
pub fn parse(bytes: &[u8]) -> GuideDocument {
    let (text, _encoding) = decode(bytes);
    parse_text(&text)
}

fn command_line(line: &str) -> Option<(&str, &str)> {
    let line = line.strip_prefix('@')?;
    // `@{...}` is an inline attribute, not a line command, even at column 0
    // (e.g. the real fixture's `@{b}$VER: ...@{ub}` body line).
    if line.starts_with('{') {
        return None;
    }
    match line.find(char::is_whitespace) {
        Some(i) => Some((&line[..i], line[i..].trim_start())),
        None => Some((line, "")),
    }
}

fn quoted(input: &str) -> IResult<&str, &str> {
    let (input, _) = char('"')(input)?;
    let (input, text) = take_till(|c| c == '"')(input)?;
    let (input, _) = char('"')(input)?;
    Ok((input, text))
}

fn bare_word(input: &str) -> IResult<&str, &str> {
    take_till(char::is_whitespace)(input)
}

/// A node-header token: a quoted string or a bareword, as `@node` and
/// `@next`/`@prev`/`@toc` all accept either.
fn token(input: &str) -> IResult<&str, &str> {
    alt((quoted, bare_word)).parse(input)
}

fn parse_node_header(rest: &str) -> (String, Option<String>) {
    let rest = rest.trim();
    match token(rest) {
        Ok((remainder, name)) if !name.is_empty() => {
            let title = token(remainder.trim_start())
                .ok()
                .map(|(_, t)| t.to_string())
                .filter(|t| !t.is_empty());
            (name.to_string(), title)
        }
        // Malformed/empty `@node` line — keep the whole rest as the name
        // rather than failing the document.
        _ => (rest.to_string(), None),
    }
}

fn first_token(rest: &str) -> String {
    token(rest.trim())
        .map(|(_, t)| t.to_string())
        .unwrap_or_else(|_| rest.trim().to_string())
}

fn close_node(
    doc: &mut GuideDocument,
    current: &mut Option<GuideNode>,
    body_lines: &mut Vec<&str>,
) {
    if let Some(mut node) = current.take() {
        node.body = parse_body(&body_lines.join("\n"));
        doc.nodes.push(node);
    }
    body_lines.clear();
}

fn parse_text(text: &str) -> GuideDocument {
    let mut doc = GuideDocument::default();
    let mut current: Option<GuideNode> = None;
    let mut body_lines: Vec<&str> = Vec::new();

    for line in text.lines() {
        match command_line(line) {
            Some((cmd, rest)) => match cmd.to_ascii_lowercase().as_str() {
                "database" => doc.database = Some(rest.trim().to_string()),
                "node" => {
                    close_node(&mut doc, &mut current, &mut body_lines);
                    let (name, title) = parse_node_header(rest);
                    current = Some(GuideNode {
                        name,
                        title,
                        ..Default::default()
                    });
                }
                "endnode" => close_node(&mut doc, &mut current, &mut body_lines),
                "next" => {
                    if let Some(node) = current.as_mut() {
                        node.next = Some(first_token(rest));
                    }
                }
                "prev" => {
                    if let Some(node) = current.as_mut() {
                        node.prev = Some(first_token(rest));
                    }
                }
                "toc" => {
                    if let Some(node) = current.as_mut() {
                        node.toc = Some(first_token(rest));
                    }
                }
                // Every other line command (@width, @wordwrap, @remark, ...)
                // doesn't affect the AST this parser produces — skipped.
                _ => {}
            },
            // Preamble text before the first @node, and anything outside a
            // node, is dropped; body lines are collected verbatim.
            None => {
                if current.is_some() {
                    body_lines.push(line);
                }
            }
        }
    }
    close_node(&mut doc, &mut current, &mut body_lines);

    doc
}

enum Chunk {
    Text(String),
    Open(Style),
    Close(Style),
    Link { text: String, target: String },
}

fn style_code(input: &str) -> IResult<&str, Chunk> {
    // Longest match first: "ub"/"ui"/"uu" must not be shadowed by "u".
    alt((
        map(tag_no_case("ub"), |_| Chunk::Close(Style::Bold)),
        map(tag_no_case("ui"), |_| Chunk::Close(Style::Italic)),
        map(tag_no_case("uu"), |_| Chunk::Close(Style::Underline)),
        map(tag_no_case("b"), |_| Chunk::Open(Style::Bold)),
        map(tag_no_case("i"), |_| Chunk::Open(Style::Italic)),
        map(tag_no_case("u"), |_| Chunk::Open(Style::Underline)),
    ))
    .parse(input)
}

fn link_code(input: &str) -> IResult<&str, Chunk> {
    let (input, text) = quoted(input)?;
    let (input, _) = space1(input)?;
    let (input, _) = tag_no_case("link")(input)?;
    let (input, _) = space1(input)?;
    let (input, target) = quoted(input)?;
    // Trailing tokens (e.g. a line number) before the closing brace are
    // recognised but not represented in the AST.
    let (input, _) = take_till(|c| c == '}')(input)?;
    Ok((
        input,
        Chunk::Link {
            text: text.to_string(),
            target: target.to_string(),
        },
    ))
}

/// One `@{...}` attribute, including its closing brace.
fn attribute(input: &str) -> IResult<&str, Chunk> {
    let (input, _) = tag("@{")(input)?;
    let (input, chunk) = alt((link_code, style_code)).parse(input)?;
    let (input, _) = char('}')(input)?;
    Ok((input, chunk))
}

/// Tokenizes a node body into text runs, style toggles, and links.
/// `\@` escapes a literal `@`; an `@{` that doesn't parse as a known
/// attribute is emitted as literal text instead of failing.
fn chunks(mut input: &str) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut text = String::new();

    while !input.is_empty() {
        if let Some(rest) = input.strip_prefix("\\@") {
            text.push('@');
            input = rest;
            continue;
        }
        if input.starts_with("@{") {
            match attribute(input) {
                Ok((rest, chunk)) => {
                    if !text.is_empty() {
                        out.push(Chunk::Text(std::mem::take(&mut text)));
                    }
                    out.push(chunk);
                    input = rest;
                    continue;
                }
                Err(_) => {
                    text.push_str("@{");
                    input = &input[2..];
                    continue;
                }
            }
        }
        let mut chars = input.chars();
        text.push(chars.next().expect("input is non-empty"));
        input = chars.as_str();
    }
    if !text.is_empty() {
        out.push(Chunk::Text(text));
    }
    out
}

/// Builds the nested [`Inline`] tree from a flat chunk stream via an
/// explicit open-style stack. An unmatched close is dropped silently; any
/// style still open at end-of-node is flushed as if closed there —
/// tolerating unbalanced markup rather than losing the trailing content.
fn build_tree(chunks: Vec<Chunk>) -> Vec<Inline> {
    let mut root: Vec<Inline> = Vec::new();
    let mut stack: Vec<(Style, Vec<Inline>)> = Vec::new();

    for chunk in chunks {
        match chunk {
            Chunk::Text(s) => stack
                .last_mut()
                .map(|(_, v)| v)
                .unwrap_or(&mut root)
                .push(Inline::Text(s)),
            Chunk::Link { text, target } => stack
                .last_mut()
                .map(|(_, v)| v)
                .unwrap_or(&mut root)
                .push(Inline::Link { text, target }),
            Chunk::Open(style) => stack.push((style, Vec::new())),
            Chunk::Close(style) => {
                if let Some(pos) = stack.iter().rposition(|(s, _)| *s == style) {
                    while stack.len() > pos {
                        let (s, children) = stack.pop().expect("just checked len > pos");
                        stack
                            .last_mut()
                            .map(|(_, v)| v)
                            .unwrap_or(&mut root)
                            .push(Inline::Styled(s, children));
                    }
                }
            }
        }
    }
    while let Some((s, children)) = stack.pop() {
        stack
            .last_mut()
            .map(|(_, v)| v)
            .unwrap_or(&mut root)
            .push(Inline::Styled(s, children));
    }
    root
}

fn parse_body(text: &str) -> Vec<Inline> {
    build_tree(chunks(text))
}
