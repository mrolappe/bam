use bam_core::query::ir::Predicate;
use bam_core::query::lang::{GrammarKind, LanguageRegistry, ParseError, QueryLanguage};
use bam_core::query::registry::FieldRegistry;

/// Always parses to the same fixed predicate, ignoring `src`. Round-trips
/// through `render` and advertises a GBNF grammar.
struct EchoLang;

impl QueryLanguage for EchoLang {
    fn id(&self) -> &str {
        "echo"
    }
    fn parse(&self, _src: &str, _reg: &FieldRegistry) -> Result<Predicate, ParseError> {
        Ok(Predicate::FullText("echo".into()))
    }
    fn render(&self, _p: &Predicate) -> Option<String> {
        Some("echo".to_string())
    }
    fn grammar(&self, _kind: GrammarKind) -> Option<String> {
        Some("root ::= 'echo'".to_string())
    }
}

/// Never parses. Cannot render or emit a grammar.
struct MuteLang;

impl QueryLanguage for MuteLang {
    fn id(&self) -> &str {
        "mute"
    }
    fn parse(&self, _src: &str, _reg: &FieldRegistry) -> Result<Predicate, ParseError> {
        Err(ParseError {
            message: "mute never parses".into(),
            span: None,
        })
    }
    fn render(&self, _p: &Predicate) -> Option<String> {
        None
    }
    fn grammar(&self, _kind: GrammarKind) -> Option<String> {
        None
    }
}

fn registry_with_both() -> LanguageRegistry {
    let mut reg = LanguageRegistry::new("echo");
    reg.register(Box::new(EchoLang));
    reg.register(Box::new(MuteLang));
    reg
}

#[test]
fn two_stub_languages_register_and_resolve_by_id() {
    let reg = registry_with_both();
    assert_eq!(reg.get(Some("echo")).unwrap().id(), "echo");
    assert_eq!(reg.get(Some("mute")).unwrap().id(), "mute");
}

#[test]
fn unknown_id_errors_naming_requested_and_available() {
    let reg = registry_with_both();
    let err = match reg.get(Some("bogus")) {
        Err(e) => e,
        Ok(_) => panic!("expected an error"),
    };
    let msg = err.to_string();
    assert!(msg.contains("bogus"));
    assert!(msg.contains("echo"));
    assert!(msg.contains("mute"));
}

#[test]
fn configured_default_used_when_no_id_given() {
    let reg = registry_with_both();
    assert_eq!(reg.get(None).unwrap().id(), "echo");
}

#[test]
fn stub_language_parses_a_string_to_a_known_predicate() {
    let reg = registry_with_both();
    let field_registry = FieldRegistry::new(bam_core::query::registry::package_fields());
    let lang = reg.get(Some("echo")).unwrap();
    let predicate = lang.parse("anything", &field_registry).unwrap();
    assert_eq!(predicate, Predicate::FullText("echo".into()));
}

#[test]
fn language_returning_none_from_grammar_is_handled_without_error() {
    let reg = registry_with_both();
    let lang = reg.get(Some("mute")).unwrap();
    // A caller that wanted a grammar (e.g. for LLM-constrained generation)
    // must treat `None` as "not supported by this language", not an error.
    let grammar = lang.grammar(GrammarKind::Gbnf);
    assert!(grammar.is_none());
}
