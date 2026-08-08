//! P9.7 — AmigaGuide markup parser.

use bam_core::ingest::amigaguide::{Inline, Style, parse};
use encoding_rs::WINDOWS_1252;
use std::path::Path;

fn fixture_bytes(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(path).unwrap()
}

fn find_link<'a>(body: &'a [Inline], text: &str) -> Option<&'a str> {
    body.iter().find_map(|i| match i {
        Inline::Link { text: t, target } if t.trim() == text => Some(target.as_str()),
        _ => None,
    })
}

/// A real AmigaGuide fixture (Commodore's own `Amigaguide.guide`, from
/// Aminet's `text/hyper/amigaguidedocs.lha`) parses to the expected AST:
/// eight nodes in file order, headers captured, and the front-page node's
/// seven links and one italic span present.
#[test]
fn real_fixture_parses_to_expected_ast() {
    let doc = parse(&fixture_bytes("Amigaguide.guide"));

    assert_eq!(doc.database.as_deref(), Some("Amigaguide.guide"));

    let names: Vec<&str> = doc.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Main",
            "Copyright",
            "About",
            "Amigaguide Background",
            "Paths and Nodes",
            "Global Commands",
            "Node Commands",
            "Attribute Commands",
        ]
    );

    let main = doc.find_node("Main").unwrap();
    assert_eq!(main.title.as_deref(), Some("Amigaguide.guide"));

    let copyright = doc.find_node("Copyright").unwrap();
    assert_eq!(copyright.next.as_deref(), Some("About"));
    assert_eq!(copyright.prev.as_deref(), Some("Main"));
    assert_eq!(copyright.toc.as_deref(), Some("Main"));

    // "The @{i}complete@{ui} documentation ..." — an italic span mid-sentence.
    let has_italic_complete = main.body.iter().any(|i| {
        matches!(i, Inline::Styled(Style::Italic, children)
            if children == &[Inline::Text("complete".to_string())])
    });
    assert!(has_italic_complete, "expected an italic \"complete\" span");

    let links: Vec<&Inline> = main
        .body
        .iter()
        .filter(|i| matches!(i, Inline::Link { .. }))
        .collect();
    assert_eq!(links.len(), 7);
}

/// Every link target on the front page names a node that actually exists in
/// the parsed document — links resolve, they aren't just opaque strings.
#[test]
fn node_links_resolve_to_node_names() {
    let doc = parse(&fixture_bytes("Amigaguide.guide"));
    let main = doc.find_node("Main").unwrap();

    for target in [
        "Copyright",
        "About",
        "Amigaguide Background",
        "Paths and Nodes",
        "Global Commands",
        "Node Commands",
        "Attribute Commands",
    ] {
        let link_target = find_link(&main.body, target)
            .unwrap_or_else(|| panic!("expected a link with text {target:?} on the Main node"));
        assert!(
            doc.find_node(link_target).is_some(),
            "link target {link_target:?} does not name a real node"
        );
    }
}

/// Inline attributes nest: bold containing italic containing a link, closed
/// in reverse order, builds a matching nested tree rather than a flat list.
#[test]
fn inline_attributes_nest_correctly() {
    let doc = parse(
        br#"@node "N" "N"
plain @{b}bold @{i}bold-italic @{" link " Link "N"} tail@{ui} still-bold@{ub} done
@endnode
"#,
    );

    let node = doc.find_node("N").unwrap();
    let bold = node
        .body
        .iter()
        .find_map(|i| match i {
            Inline::Styled(Style::Bold, children) => Some(children),
            _ => None,
        })
        .expect("expected a bold span");

    let italic = bold
        .iter()
        .find_map(|i| match i {
            Inline::Styled(Style::Italic, children) => Some(children),
            _ => None,
        })
        .expect("expected an italic span nested inside the bold span");

    assert!(
        italic
            .iter()
            .any(|i| matches!(i, Inline::Link { target, .. } if target == "N")),
        "expected the link to be nested inside the italic span"
    );
    assert!(
        bold.iter()
            .any(|i| matches!(i, Inline::Text(t) if t.contains("still-bold"))),
        "expected bold-only text after the italic span closed"
    );
}

/// Malformed markup degrades to plain text instead of failing the document:
/// an unrecognised attribute code and an unclosed style both parse without
/// panicking, and the unrecognised code survives as literal text.
#[test]
fn malformed_markup_degrades_to_plain_text() {
    let doc = parse(
        br#"@node "N" "N"
before @{notarealcode} after
@{b}never closed
@endnode
"#,
    );

    let node = doc.find_node("N").unwrap();
    let flattened: String = node
        .body
        .iter()
        .map(|i| match i {
            Inline::Text(t) => t.clone(),
            Inline::Styled(_, children) => children
                .iter()
                .map(|c| match c {
                    Inline::Text(t) => t.clone(),
                    _ => String::new(),
                })
                .collect(),
            Inline::Link { text, .. } => text.clone(),
        })
        .collect();

    assert!(
        flattened.contains("@{notarealcode}"),
        "unrecognised attribute should survive as literal text, got {flattened:?}"
    );
    assert!(flattened.contains("never closed"));
}

/// Body text is decoded through P1.5's `decode`, not assumed UTF-8: a
/// Latin-1-encoded accented byte in a node body comes back as the right
/// character rather than mojibake or a decode failure.
#[test]
fn body_text_decoded_through_p1_5_not_assumed_utf8() {
    let mut raw = br#"@node "N" "N"
caf"#
        .to_vec();
    let (encoded, _, had_errors) = WINDOWS_1252.encode("café");
    assert!(!had_errors);
    raw.extend_from_slice(&encoded[3..]); // the "é" byte only ("caf" already written as ASCII)
    raw.extend_from_slice(b"\n@endnode\n");

    let doc = parse(&raw);
    let node = doc.find_node("N").unwrap();
    let flattened: String = node
        .body
        .iter()
        .map(|i| match i {
            Inline::Text(t) => t.clone(),
            _ => String::new(),
        })
        .collect();
    assert!(
        flattened.contains("café"),
        "expected the Latin-1 byte to decode to 'é', got {flattened:?}"
    );
}
