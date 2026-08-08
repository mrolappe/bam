//! GBNF and JSON Schema generation for `bam-dsl` (P7.2, invariant I2).
//!
//! One production-rule description — [`rules`], transliterated directly
//! from `docs/lang-bam-dsl.md`'s grammar (field names are left as a generic
//! `ident`, exactly as the doc defines `field := ident`: field *validity* is
//! a registry-resolution concern the real parser only checks after parsing,
//! so the grammar that constrains generation shouldn't be tighter than the
//! grammar that constrains parsing) — is rendered two ways: [`bam_dsl_gbnf`]
//! walks it into GBNF text for llama.cpp's token-by-token constraining;
//! [`bam_dsl_json_schema`] instead derives from `Predicate`'s own
//! `schemars::JsonSchema` impl (`ir.rs`), because a cloud provider's
//! "structured output" constrains a JSON *tree*, not raw DSL text — the two
//! providers constrain different concrete syntaxes for the same language of
//! `Predicate`s, one text (`bam_dsl::render`'s output), one JSON
//! (`serde_json::to_value`'s output). §10 requires both to derive from one
//! source rather than being hand-maintained separately; here that source is
//! the DSL's own documented grammar plus the `Predicate` type itself, not a
//! second hand-copied grammar.
//!
//! [`gbnf_accepts`] and [`json_schema_accepts`] exist only for tests: a
//! small backtracking interpreter over the same [`Node`] AST the GBNF
//! renderer walks (so a renderer bug shows up as a
//! matcher/renderer disagreement, not a silently-wrong GBNF string), and a
//! minimal JSON Schema validator covering the subset `schemars` 0.8 emits
//! (`$ref`, `oneOf`/`anyOf`/`allOf`, `enum`, `type`, `properties`/
//! `required`, `items`) — enough to check the *generated* schema text
//! actually accepts what it's supposed to, rather than trusting `schemars`
//! by construction.

use std::collections::{BTreeSet, HashMap};

use serde_json::Value as Json;

use super::ir::Predicate;

#[derive(Clone)]
enum Node {
    Lit(&'static str),
    Ref(&'static str),
    Seq(Vec<Node>),
    Alt(Vec<Node>),
    Star(Box<Node>),
    Plus(Box<Node>),
    Opt(Box<Node>),
    /// `(gbnf char class text, membership predicate)`.
    Class(&'static str, fn(char) -> bool),
}

/// `docs/lang-bam-dsl.md`'s grammar, one rule per doc production. `value`'s
/// finer breakdown in the doc (`number size_suffix? | date | string |
/// bareword_value`) collapses to `rhs := string | bareword` here because
/// that's what the real parser (`bam_dsl::Parser::lex_rhs`) actually
/// accepts — it never distinguishes a number/date token from any other
/// bareword at the lexical level, only later, by field type
/// (`Parser::typed_value`). A grammar meant to constrain what the real
/// parser will accept has to match the parser, not the doc's more
/// suggestive gloss.
fn rules() -> Vec<(&'static str, Node)> {
    use Node::*;
    fn is_ident_start(c: char) -> bool {
        c.is_ascii_alphabetic() || c == '_'
    }
    fn is_ident_cont(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }
    fn is_ws(c: char) -> bool {
        c == ' ' || c == '\t' || c == '\n'
    }
    fn is_bareword_char(c: char) -> bool {
        !is_ws(c) && c != '(' && c != ')'
    }
    fn is_not_quote(c: char) -> bool {
        c != '\''
    }

    vec![
        ("root", Ref("or_expr")),
        (
            "or_expr",
            Seq(vec![
                Ref("and_expr"),
                Star(Box::new(Seq(vec![
                    Ref("ws"),
                    Lit("OR"),
                    Ref("ws"),
                    Ref("and_expr"),
                ]))),
            ]),
        ),
        (
            "and_expr",
            Seq(vec![
                Ref("unary"),
                Star(Box::new(Seq(vec![Ref("ws"), Ref("unary")]))),
            ]),
        ),
        (
            "unary",
            Alt(vec![Seq(vec![Lit("!"), Ref("unary")]), Ref("atom")]),
        ),
        (
            "atom",
            Alt(vec![
                Seq(vec![
                    Lit("("),
                    Opt(Box::new(Ref("ws"))),
                    Ref("or_expr"),
                    Opt(Box::new(Ref("ws"))),
                    Lit(")"),
                ]),
                Ref("special_term"),
                Ref("term"),
            ]),
        ),
        (
            "special_term",
            Alt(vec![Ref("in_term"), Ref("similar_term"), Lit("marked")]),
        ),
        ("in_term", Seq(vec![Lit("in:"), Ref("rhs")])),
        (
            "similar_term",
            Seq(vec![
                Lit("similar:"),
                Ref("rhs"),
                Opt(Box::new(Seq(vec![
                    Opt(Box::new(Ref("ws"))),
                    Lit(">"),
                    Opt(Box::new(Ref("ws"))),
                    Ref("bareword"),
                ]))),
            ]),
        ),
        (
            "term",
            Alt(vec![
                Seq(vec![
                    Ref("field"),
                    Lit(":"),
                    Opt(Box::new(Lit("~"))),
                    Ref("rhs"),
                ]),
                Seq(vec![Ref("field"), Ref("cmp_op"), Ref("rhs")]),
                Ref("bareword"),
            ]),
        ),
        ("field", Ref("ident")),
        (
            "cmp_op",
            Alt(vec!["!=", "<=", ">=", "<", ">", "="]
                .into_iter()
                .map(Lit)
                .collect()),
        ),
        ("rhs", Alt(vec![Ref("string"), Ref("bareword")])),
        (
            "string",
            Seq(vec![
                Lit("'"),
                Star(Box::new(Class("[^']", is_not_quote))),
                Lit("'"),
            ]),
        ),
        (
            "ident",
            Seq(vec![
                Class("[A-Za-z_]", is_ident_start),
                Star(Box::new(Class("[A-Za-z0-9_]", is_ident_cont))),
            ]),
        ),
        (
            "bareword",
            Plus(Box::new(Class("[^ \\t\\n()]", is_bareword_char))),
        ),
        ("ws", Plus(Box::new(Class("[ \\t\\n]", is_ws)))),
    ]
}

fn render(node: &Node) -> String {
    match node {
        Node::Lit(s) => format!("{s:?}"),
        Node::Ref(name) => (*name).to_string(),
        Node::Class(text, _) => (*text).to_string(),
        Node::Seq(items) => items
            .iter()
            .map(render_grouped)
            .collect::<Vec<_>>()
            .join(" "),
        Node::Alt(items) => items
            .iter()
            .map(render_grouped)
            .collect::<Vec<_>>()
            .join(" | "),
        Node::Star(inner) => format!("{}*", render_grouped(inner)),
        Node::Plus(inner) => format!("{}+", render_grouped(inner)),
        Node::Opt(inner) => format!("{}?", render_grouped(inner)),
    }
}

fn render_grouped(node: &Node) -> String {
    match node {
        Node::Seq(_) | Node::Alt(_) => format!("({})", render(node)),
        _ => render(node),
    }
}

/// GBNF text for `bam-dsl`, constraining raw query text to the language
/// `docs/lang-bam-dsl.md` documents (see [`rules`] for the one deliberate
/// deviation from the doc's gloss).
pub fn bam_dsl_gbnf() -> String {
    rules()
        .iter()
        .map(|(name, node)| format!("{name} ::= {}", render(node)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// JSON Schema for the `Predicate` tree that `serde_json::to_value` produces
/// — the JSON encoding a cloud, structured-output provider is constrained
/// to emit instead of raw `bam-dsl` text.
pub fn bam_dsl_json_schema() -> Json {
    serde_json::to_value(schemars::schema_for!(Predicate)).expect("RootSchema serializes")
}

// --- Test-only interpreters: prove the generated text/JSON actually accept
// what they're meant to, instead of trusting the renderers by construction.

fn parse_node(
    node: &Node,
    chars: &[char],
    pos: usize,
    rules: &HashMap<&'static str, Node>,
) -> BTreeSet<usize> {
    match node {
        Node::Lit(s) => {
            let lit: Vec<char> = s.chars().collect();
            if chars[pos..].starts_with(&lit[..]) {
                BTreeSet::from([pos + lit.len()])
            } else {
                BTreeSet::new()
            }
        }
        Node::Ref(name) => parse_node(&rules[name], chars, pos, rules),
        Node::Class(_, pred) => {
            if pos < chars.len() && pred(chars[pos]) {
                BTreeSet::from([pos + 1])
            } else {
                BTreeSet::new()
            }
        }
        Node::Seq(items) => {
            let mut positions = BTreeSet::from([pos]);
            for item in items {
                let mut next = BTreeSet::new();
                for p in &positions {
                    next.extend(parse_node(item, chars, *p, rules));
                }
                positions = next;
                if positions.is_empty() {
                    break;
                }
            }
            positions
        }
        Node::Alt(items) => {
            let mut out = BTreeSet::new();
            for item in items {
                out.extend(parse_node(item, chars, pos, rules));
            }
            out
        }
        Node::Opt(inner) => {
            let mut out = BTreeSet::from([pos]);
            out.extend(parse_node(inner, chars, pos, rules));
            out
        }
        Node::Star(inner) => repeat(inner, chars, rules, BTreeSet::from([pos])),
        Node::Plus(inner) => {
            let first = parse_node(inner, chars, pos, rules);
            repeat(inner, chars, rules, first)
        }
    }
}

/// Shared fixpoint loop for `*`/`+`: from `seed` (the zero- or one-rep
/// starting set), repeatedly apply `inner` until no new *forward* end
/// position appears — "forward" rules out looping forever on a
/// zero-width match, which none of this grammar's repeated bodies produce
/// (`ws`/`bareword`/string content/OR-and_expr all consume >=1 char per
/// rep), but the guard costs nothing to keep.
fn repeat(
    inner: &Node,
    chars: &[char],
    rules: &HashMap<&'static str, Node>,
    seed: BTreeSet<usize>,
) -> BTreeSet<usize> {
    let mut out = seed.clone();
    let mut frontier = seed;
    loop {
        let mut next = BTreeSet::new();
        for p in &frontier {
            for np in parse_node(inner, chars, *p, rules) {
                if np > *p && !out.contains(&np) {
                    next.insert(np);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        out.extend(next.iter().copied());
        frontier = next;
    }
    out
}

/// Whether `input` is a complete match for the GBNF grammar [`bam_dsl_gbnf`]
/// renders (test-only: interprets [`rules`] directly, see module docs).
pub fn gbnf_accepts(input: &str) -> bool {
    let table: HashMap<&'static str, Node> = rules().into_iter().collect();
    let chars: Vec<char> = input.chars().collect();
    parse_node(&table["root"], &chars, 0, &table).contains(&chars.len())
}

fn resolve_ref<'a>(root: &'a Json, pointer: &str) -> &'a Json {
    let mut cur = root;
    for part in pointer.trim_start_matches("#/").split('/') {
        cur = &cur[part];
    }
    cur
}

fn type_matches(ty: &Json, instance: &Json) -> bool {
    let kinds: Vec<&str> = match ty {
        Json::String(s) => vec![s.as_str()],
        Json::Array(a) => a.iter().filter_map(Json::as_str).collect(),
        _ => return true,
    };
    kinds.iter().any(|k| match *k {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "boolean" => instance.is_boolean(),
        "null" => instance.is_null(),
        "integer" => instance.is_i64() || instance.is_u64(),
        "number" => instance.is_number(),
        _ => true,
    })
}

/// A minimal Draft-07-subset validator — only what `schemars` 0.8 emits for
/// `Predicate` (see module docs for the covered keywords).
fn schema_accepts(schema: &Json, instance: &Json, root: &Json) -> bool {
    if let Some(r) = schema.get("$ref").and_then(Json::as_str) {
        return schema_accepts(resolve_ref(root, r), instance, root);
    }
    if let Some(all_of) = schema.get("allOf").and_then(Json::as_array) {
        if !all_of.iter().all(|s| schema_accepts(s, instance, root)) {
            return false;
        }
    }
    if let Some(any_of) = schema.get("anyOf").and_then(Json::as_array) {
        return any_of.iter().any(|s| schema_accepts(s, instance, root));
    }
    if let Some(one_of) = schema.get("oneOf").and_then(Json::as_array) {
        return one_of
            .iter()
            .filter(|s| schema_accepts(s, instance, root))
            .count()
            == 1;
    }
    if let Some(enum_vals) = schema.get("enum").and_then(Json::as_array) {
        return enum_vals.contains(instance);
    }
    if let Some(ty) = schema.get("type") {
        if !type_matches(ty, instance) {
            return false;
        }
    }
    if let Some(props) = schema.get("properties").and_then(Json::as_object) {
        let Some(obj) = instance.as_object() else {
            return false;
        };
        for (k, subschema) in props {
            if let Some(v) = obj.get(k) {
                if !schema_accepts(subschema, v, root) {
                    return false;
                }
            }
        }
        if let Some(required) = schema.get("required").and_then(Json::as_array) {
            for r in required {
                if r.as_str().is_some_and(|name| !obj.contains_key(name)) {
                    return false;
                }
            }
        }
        if schema.get("additionalProperties") == Some(&Json::Bool(false))
            && obj.keys().any(|k| !props.contains_key(k))
        {
            return false;
        }
    }
    if let Some(items_schema) = schema.get("items") {
        return match instance.as_array() {
            Some(arr) => arr
                .iter()
                .all(|item| schema_accepts(items_schema, item, root)),
            None => false,
        };
    }
    true
}

/// Whether `instance` validates against [`bam_dsl_json_schema`] (test-only).
pub fn json_schema_accepts(schema: &Json, instance: &Json) -> bool {
    schema_accepts(schema, instance, schema)
}
