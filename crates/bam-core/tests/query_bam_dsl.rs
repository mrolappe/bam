//! P2.4: the fifteen worked examples and malformed inputs from
//! `docs/lang-bam-dsl.md`, verbatim.

use bam_core::query::bam_dsl::BamDsl;
use bam_core::query::ir::{CmpOp, FieldId, Pattern, Predicate, SelectionRef, Value};
use bam_core::query::lang::QueryLanguage;
use bam_core::query::registry::{FieldRegistry, package_fields};

fn fid(name: &str) -> FieldId {
    FieldId::new(name)
}

fn registry() -> FieldRegistry {
    FieldRegistry::new(package_fields())
}

/// The fifteen worked examples from `docs/lang-bam-dsl.md`, in order.
fn worked_examples() -> Vec<(&'static str, Predicate)> {
    use Predicate::*;
    vec![
        (
            "dir:util/*",
            Match {
                field: fid("dir"),
                pattern: Pattern::Glob("util/*".into()),
            },
        ),
        (
            "size>100k",
            Compare {
                field: fid("size"),
                op: CmpOp::Gt,
                value: Value::Int(102_400),
            },
        ),
        (
            "year>2000",
            Compare {
                field: fid("year"),
                op: CmpOp::Gt,
                value: Value::Int(2000),
            },
        ),
        (
            "dir:util/* !name:mod OR year>2000",
            Or(vec![
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
            ]),
        ),
        (
            "tracker module editor",
            FullText("tracker module editor".into()),
        ),
        (
            "name:Deluxe* version:1.2",
            And(vec![
                Match {
                    field: fid("name"),
                    pattern: Pattern::Glob("Deluxe*".into()),
                },
                Compare {
                    field: fid("version"),
                    op: CmpOp::Eq,
                    value: Value::Text("1.2".into()),
                },
            ]),
        ),
        (
            "in:'tracker candidates'",
            InSelection(SelectionRef::Named("tracker candidates".into())),
        ),
        (
            "marked !size<10k",
            And(vec![
                InSelection(SelectionRef::Marked),
                Not(Box::new(Compare {
                    field: fid("size"),
                    op: CmpOp::Lt,
                    value: Value::Int(10_240),
                })),
            ]),
        ),
        (
            "similar:'tracker module editor' > 0.82",
            Similar {
                text: "tracker module editor".into(),
                threshold: 0.82,
            },
        ),
        (
            "dir:mus/* (year<1995 OR year>2000)",
            And(vec![
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
            ]),
        ),
        (
            "!(dir:mus/* OR dir:cla/*)",
            Not(Box::new(Or(vec![
                Match {
                    field: fid("dir"),
                    pattern: Pattern::Glob("mus/*".into()),
                },
                Match {
                    field: fid("dir"),
                    pattern: Pattern::Glob("cla/*".into()),
                },
            ]))),
        ),
        (
            "date>2020-01-01",
            Compare {
                field: fid("date"),
                op: CmpOp::Gt,
                value: Value::Date("2020-01-01".into()),
            },
        ),
        (
            "description:~'demo'",
            Match {
                field: fid("description"),
                pattern: Pattern::Prefix("demo".into()),
            },
        ),
        (
            "version!=1.0",
            Compare {
                field: fid("version"),
                op: CmpOp::Ne,
                value: Value::Text("1.0".into()),
            },
        ),
        (
            "file:*.lha",
            Match {
                field: fid("file"),
                pattern: Pattern::Glob("*.lha".into()),
            },
        ),
    ]
}

#[test]
fn all_fifteen_examples_parse_to_their_documented_predicate() {
    let reg = registry();
    let lang = BamDsl;
    for (src, expected) in worked_examples() {
        let got = lang
            .parse(src, &reg)
            .unwrap_or_else(|e| panic!("{src}: {e}"));
        assert_eq!(got, expected, "source: {src}");
    }
}

#[test]
fn render_round_trips_each_of_the_fifteen_examples() {
    let reg = registry();
    let lang = BamDsl;
    for (src, predicate) in worked_examples() {
        let rendered = lang
            .render(&predicate)
            .unwrap_or_else(|| panic!("{src}: render returned None"));
        let reparsed = lang
            .parse(&rendered, &reg)
            .unwrap_or_else(|e| panic!("{src}: rendered {rendered:?} failed to reparse: {e}"));
        assert_eq!(reparsed, predicate, "source: {src}, rendered: {rendered}");
    }
}

#[test]
fn juxtaposition_binds_tighter_than_or() {
    let reg = registry();
    let lang = BamDsl;
    let got = lang
        .parse("dir:util/* !name:mod OR year>2000", &reg)
        .unwrap();
    match got {
        Predicate::Or(parts) => {
            assert_eq!(parts.len(), 2);
            assert!(
                matches!(parts[0], Predicate::And(_)),
                "left of OR must be one AND group"
            );
        }
        other => panic!("expected a top-level Or, got {other:?}"),
    }
}

#[test]
fn unknown_field_names_field_and_suggests_near_match() {
    let reg = registry();
    let lang = BamDsl;
    let err = lang.parse("siz:100", &reg).unwrap_err();
    assert!(err.message.contains("siz"), "message: {}", err.message);
    assert!(err.message.contains("size"), "message: {}", err.message);
    assert_eq!(err.span, Some((0, 3)));
}

/// The malformed-input table from `docs/lang-bam-dsl.md`, verbatim.
#[test]
fn malformed_inputs_report_the_documented_span() {
    let reg = registry();
    let lang = BamDsl;
    let cases: &[(&str, (usize, usize))] = &[
        ("siz:100", (0, 3)),
        ("type:mod", (0, 4)),
        ("dir:util/* (year<2000", (11, 12)),
        ("size<", (5, 5)),
        ("in:'tracker", (3, 11)),
        ("size:~'foo'", (0, 4)),
    ];
    for (src, span) in cases {
        let err = lang.parse(src, &reg).unwrap_err();
        assert_eq!(
            err.span,
            Some(*span),
            "source: {src}, message: {}",
            err.message
        );
    }
}
