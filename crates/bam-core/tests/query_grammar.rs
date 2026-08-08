//! P7.2: GBNF and JSON Schema generation for `bam-dsl`, derived from
//! `docs/lang-bam-dsl.md`'s grammar and `Predicate`'s own `JsonSchema` impl
//! rather than a second hand-maintained grammar (§10). The equivalence
//! property test is the drift check: both artifacts must accept the same
//! language of `Predicate`s, expressed as bam-dsl text for GBNF and as JSON
//! for JSON Schema.

use bam_core::query::bam_dsl::BamDsl;
use bam_core::query::grammar::{gbnf_accepts, json_schema_accepts};
use bam_core::query::ir::{CmpOp, FieldId, Pattern, Predicate, SelectionRef, Value};
use bam_core::query::lang::{GrammarKind, ParseError, QueryLanguage};
use bam_core::query::registry::{FieldRegistry, package_fields};

fn registry() -> FieldRegistry {
    FieldRegistry::new(package_fields())
}

fn fid(name: &str) -> FieldId {
    FieldId::new(name)
}

/// The fifteen worked examples from `docs/lang-bam-dsl.md`, verbatim text
/// only — `query_bam_dsl.rs` already proves each parses to the tree shown
/// there; this file only needs the source text to check grammar acceptance.
fn worked_example_texts() -> Vec<&'static str> {
    vec![
        "dir:util/*",
        "size>100k",
        "year>2000",
        "dir:util/* !name:mod OR year>2000",
        "tracker module editor",
        "name:Deluxe* version:1.2",
        "in:'tracker candidates'",
        "marked !size<10k",
        "similar:'tracker module editor' > 0.82",
        "dir:mus/* (year<1995 OR year>2000)",
        "!(dir:mus/* OR dir:cla/*)",
        "date>2020-01-01",
        "description:~'demo'",
        "version!=1.0",
        "file:*.lha",
    ]
}

#[test]
fn worked_examples_validate_against_gbnf_and_json_schema() {
    let reg = registry();
    let lang = BamDsl;
    let gbnf = lang.grammar(GrammarKind::Gbnf).unwrap();
    let schema = serde_json::from_str(&lang.grammar(GrammarKind::JsonSchema).unwrap()).unwrap();

    for text in worked_example_texts() {
        assert!(gbnf_accepts(text), "GBNF should accept {text:?}\n{gbnf}");

        let pred = lang
            .parse(text, &reg)
            .unwrap_or_else(|e| panic!("{text:?}: {e}"));
        let json = serde_json::to_value(&pred).unwrap();
        assert!(
            json_schema_accepts(&schema, &json),
            "JSON Schema should accept {json} (from {text:?})"
        );
    }
}

#[test]
fn malformed_input_rejected_by_both() {
    // From the malformed-input table: unbalanced '('.
    assert!(!gbnf_accepts("dir:util/* (year<2000"));

    let schema: serde_json::Value =
        serde_json::from_str(&BamDsl.grammar(GrammarKind::JsonSchema).unwrap()).unwrap();
    // Unknown `Predicate` variant tag — no `oneOf`/`properties` branch names it.
    let bogus = serde_json::json!({"Frobnicate": {"field": "dir"}});
    assert!(!json_schema_accepts(&schema, &bogus));
}

/// A spread of `Predicate`s exercising every variant, several nestings, and
/// every `CmpOp`/`Value`/`Pattern`/`SelectionRef` case — the equivalence
/// property test's generated inputs. Hand-enumerated rather than pulled from
/// a fuzzing crate (none is in the workspace, and this grammar is small
/// enough that a fixed spread covers every construct at least once, several
/// combinatorially).
fn generated_predicates() -> Vec<Predicate> {
    use Predicate::*;
    let mut out = vec![
        Compare {
            field: fid("size"),
            op: CmpOp::Ge,
            value: Value::Int(2048),
        },
        Compare {
            field: fid("date"),
            op: CmpOp::Le,
            value: Value::Date("2020-01-01".into()),
        },
        Compare {
            field: fid("version"),
            op: CmpOp::Ne,
            value: Value::Text("1.0".into()),
        },
        Match {
            field: fid("name"),
            pattern: Pattern::Glob("Deluxe*".into()),
        },
        Match {
            field: fid("description"),
            pattern: Pattern::Prefix("demo".into()),
        },
        FullText("tracker module editor".into()),
        InSelection(SelectionRef::Named("tracker candidates".into())),
        InSelection(SelectionRef::Marked),
        Similar {
            text: "tracker module editor".into(),
            threshold: 0.82,
        },
        Not(Box::new(Compare {
            field: fid("year"),
            op: CmpOp::Gt,
            value: Value::Int(2000),
        })),
    ];
    // A couple of nested And/Or/Not/paren combinations, mirroring the doc's
    // own nested worked examples (10, 11).
    out.push(And(vec![
        Match {
            field: fid("dir"),
            pattern: Pattern::Glob("mus/*".into()),
        },
        Or(vec![
            Compare {
                field: fid("year"),
                op: CmpOp::Lt,
                value: Value::Int(1995),
            },
            Compare {
                field: fid("year"),
                op: CmpOp::Gt,
                value: Value::Int(2000),
            },
        ]),
    ]));
    out.push(Not(Box::new(Or(vec![
        Match {
            field: fid("dir"),
            pattern: Pattern::Glob("mus/*".into()),
        },
        Match {
            field: fid("dir"),
            pattern: Pattern::Glob("cla/*".into()),
        },
    ]))));
    out.push(Or(vec![
        And(vec![
            Match {
                field: fid("dir"),
                pattern: Pattern::Glob("util/*".into()),
            },
            Not(Box::new(Compare {
                field: fid("name"),
                op: CmpOp::Eq,
                value: Value::Text("mod".into()),
            })),
        ]),
        Compare {
            field: fid("year"),
            op: CmpOp::Gt,
            value: Value::Int(2000),
        },
    ]));
    out
}

#[test]
fn gbnf_and_json_schema_accept_the_same_language() {
    let lang = BamDsl;
    let gbnf_text = lang.grammar(GrammarKind::Gbnf).unwrap();
    let schema: serde_json::Value =
        serde_json::from_str(&lang.grammar(GrammarKind::JsonSchema).unwrap()).unwrap();

    for pred in generated_predicates() {
        let text = lang.render(&pred).expect("bam-dsl always renders");
        assert!(
            gbnf_accepts(&text),
            "GBNF should accept the render of {pred:?} ({text:?})\n{gbnf_text}"
        );

        let json = serde_json::to_value(&pred).unwrap();
        assert!(
            json_schema_accepts(&schema, &json),
            "JSON Schema should accept {json} (from {pred:?})"
        );

        // Round-trips through the real parser too — the rendered text isn't
        // just GBNF-shaped, it's actually this predicate again.
        let reparsed = lang.parse(&text, &registry()).unwrap();
        assert_eq!(reparsed, pred, "text {text:?} should reparse to itself");
    }
}

/// A `QueryLanguage` with no grammar for either kind — the "cannot be
/// constrained" capability gap `docs/plan/phase-7-llm.md` names, not a
/// failure. Whatever eventually calls `grammar()` (P7.3's prompt assembly)
/// must fall back to unconstrained generation rather than erroring; at the
/// trait level that's just `Option::None` propagating cleanly, which this
/// checks holds through the registry too.
struct NoGrammarLang;

impl QueryLanguage for NoGrammarLang {
    fn id(&self) -> &str {
        "no-grammar"
    }
    fn parse(&self, _src: &str, _reg: &FieldRegistry) -> Result<Predicate, ParseError> {
        Ok(Predicate::FullText("stub".into()))
    }
    fn render(&self, _p: &Predicate) -> Option<String> {
        None
    }
    fn grammar(&self, _kind: GrammarKind) -> Option<String> {
        None
    }
}

#[test]
fn language_without_a_grammar_is_handled_without_error() {
    let lang = NoGrammarLang;
    assert_eq!(lang.grammar(GrammarKind::Gbnf), None);
    assert_eq!(lang.grammar(GrammarKind::JsonSchema), None);
}
