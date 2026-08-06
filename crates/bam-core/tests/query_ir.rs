use bam_core::query::ir::{CmpOp, FieldId, Pattern, Predicate, SelectionRef, Value};
use bam_core::query::registry::{FieldRegistry, RegistryError, package_fields};

fn every_variant() -> Vec<Predicate> {
    vec![
        Predicate::And(vec![Predicate::FullText("mod".into())]),
        Predicate::Or(vec![Predicate::FullText("mod".into())]),
        Predicate::Not(Box::new(Predicate::FullText("mod".into()))),
        Predicate::Compare {
            field: FieldId::new("size"),
            op: CmpOp::Gt,
            value: Value::Int(1024),
        },
        Predicate::Match {
            field: FieldId::new("dir"),
            pattern: Pattern::Glob("util/*".into()),
        },
        Predicate::FullText("tracker module".into()),
        Predicate::InSelection(SelectionRef::Named("tracker candidates".into())),
        Predicate::InSelection(SelectionRef::Marked),
        Predicate::Similar {
            text: "tracker module editor".into(),
            threshold: 0.82,
        },
    ]
}

#[test]
fn every_predicate_variant_round_trips_through_serde() {
    for p in every_variant() {
        let json = serde_json::to_string(&p).expect("serialize");
        let back: Predicate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(p, back, "round-trip mismatch for {json}");
    }
}

#[test]
fn registry_resolves_field_by_name_and_by_alias() {
    let reg = FieldRegistry::new(package_fields());

    let by_name = reg.resolve("size").expect("resolve by name");
    let by_alias = reg.resolve("size_bytes").expect("resolve by alias");
    assert_eq!(by_name.id, by_alias.id);
    assert_eq!(by_name.name, "size");
}

#[test]
fn unknown_field_name_errors_naming_the_field() {
    let reg = FieldRegistry::new(package_fields());

    let err = reg.resolve("bogus").unwrap_err();
    assert_eq!(err, RegistryError::UnknownField("bogus".to_string()));
    assert!(err.to_string().contains("bogus"));
}

#[test]
fn operator_the_field_does_not_permit_errors_at_resolve_time() {
    let reg = FieldRegistry::new(package_fields());

    // `size:~'foo'` — a glob/Match predicate against an Int field.
    let size = reg.resolve("size").expect("size field exists");
    let err = reg.check_match(size).unwrap_err();
    assert_eq!(err, RegistryError::MatchNotPermitted("size".to_string()));

    // `dir>5` — an ordering comparison a Text field doesn't permit.
    let dir = reg.resolve("dir").expect("dir field exists");
    let err = reg.check_compare(dir, CmpOp::Gt).unwrap_err();
    assert_eq!(
        err,
        RegistryError::OperatorNotPermitted {
            field: "dir".to_string(),
            op: CmpOp::Gt,
        }
    );
}

#[test]
fn similar_constructs_and_serializes() {
    // Compile-time rejection ("not yet supported") is P2.5's job and is
    // documented in docs/query-ir.md; this only proves the IR node itself
    // is usable today.
    let p = Predicate::Similar {
        text: "tracker module editor".into(),
        threshold: 0.82,
    };
    let json = serde_json::to_string(&p).expect("serialize");
    assert!(json.contains("0.82"));
}
