//! `bam-dsl` (P2.4): the default `QueryLanguage`, per `docs/lang-bam-dsl.md`.
//! Hand-rolled — ten productions and a hard requirement for byte-accurate
//! error spans made a precedence climber shorter here than `nom` plus the
//! error-mapping needed to extract spans (see the phase doc's own note).

use super::ir::{CmpOp, FieldId, Pattern, Predicate, SelectionRef, Value};
use super::lang::{GrammarKind, ParseError, QueryLanguage};
use super::registry::{FieldRegistry, FieldType, RegistryError};
use crate::ingest::normalize::parse_size_bytes;

pub struct BamDsl;

impl QueryLanguage for BamDsl {
    fn id(&self) -> &str {
        "bam-dsl"
    }

    fn parse(&self, src: &str, reg: &FieldRegistry) -> Result<Predicate, ParseError> {
        let mut p = Parser {
            src,
            bytes: src.as_bytes(),
            pos: 0,
            reg,
        };
        let pred = p.parse_or_expr()?;
        p.skip_ws();
        if p.pos != src.len() {
            return Err(ParseError {
                message: "unexpected trailing input".to_string(),
                span: Some((p.pos, src.len())),
            });
        }
        Ok(pred)
    }

    fn render(&self, p: &Predicate) -> Option<String> {
        Some(render(p))
    }

    fn grammar(&self, kind: GrammarKind) -> Option<String> {
        Some(match kind {
            GrammarKind::Gbnf => super::grammar::bam_dsl_gbnf(),
            GrammarKind::JsonSchema => super::grammar::bam_dsl_json_schema().to_string(),
        })
    }
}

struct Parser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    reg: &'a FieldRegistry,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn peek_is_or_keyword(&self) -> bool {
        self.src[self.pos..].starts_with("OR")
            && self
                .bytes
                .get(self.pos + 2)
                .is_none_or(|b| b.is_ascii_whitespace())
    }

    /// `[A-Za-z_][A-Za-z0-9_]*`, or `None` (and no movement) if `pos` isn't
    /// on an identifier start.
    fn lex_ident(&mut self) -> Option<(String, usize, usize)> {
        let start = self.pos;
        let first = *self.bytes.get(start)?;
        if !(first.is_ascii_alphabetic() || first == b'_') {
            return None;
        }
        let mut end = start + 1;
        while self
            .bytes
            .get(end)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
        {
            end += 1;
        }
        self.pos = end;
        Some((self.src[start..end].to_string(), start, end))
    }

    /// Maximal run of non-whitespace, non-paren bytes — used both for
    /// bareword atoms and for an unquoted rhs value token.
    fn lex_bareword_token(&mut self) -> (String, usize, usize) {
        let start = self.pos;
        let mut end = start;
        while self
            .bytes
            .get(end)
            .is_some_and(|b| !b.is_ascii_whitespace() && *b != b'(' && *b != b')')
        {
            end += 1;
        }
        self.pos = end;
        (self.src[start..end].to_string(), start, end)
    }

    fn lex_quoted(&mut self) -> Result<(String, usize, usize), ParseError> {
        let quote_start = self.pos;
        self.pos += 1; // opening '
        let content_start = self.pos;
        match self.src[content_start..].find('\'') {
            Some(rel) => {
                let content_end = content_start + rel;
                let content = self.src[content_start..content_end].to_string();
                self.pos = content_end + 1; // closing '
                Ok((content, quote_start, self.pos))
            }
            None => Err(ParseError {
                message: "unterminated string literal".to_string(),
                span: Some((quote_start, self.src.len())),
            }),
        }
    }

    /// A term's rhs: a quoted string, or a bare token. Errors if empty —
    /// covers both `size<` (nothing follows the operator) and `dir:` alone.
    fn lex_rhs(&mut self) -> Result<(String, usize, usize), ParseError> {
        if self.peek() == Some(b'\'') {
            self.lex_quoted()
        } else {
            let (raw, start, end) = self.lex_bareword_token();
            if raw.is_empty() {
                Err(ParseError {
                    message: "expected a value".to_string(),
                    span: Some((self.pos, self.pos)),
                })
            } else {
                Ok((raw, start, end))
            }
        }
    }

    fn try_lex_cmp_op(&mut self) -> Option<CmpOp> {
        let rest = &self.src[self.pos..];
        let (op, len) = if rest.starts_with("!=") {
            (CmpOp::Ne, 2)
        } else if rest.starts_with("<=") {
            (CmpOp::Le, 2)
        } else if rest.starts_with(">=") {
            (CmpOp::Ge, 2)
        } else if rest.starts_with('<') {
            (CmpOp::Lt, 1)
        } else if rest.starts_with('>') {
            (CmpOp::Gt, 1)
        } else if rest.starts_with('=') {
            (CmpOp::Eq, 1)
        } else {
            return None;
        };
        self.pos += len;
        Some(op)
    }

    fn field_error(&self, e: RegistryError, start: usize, end: usize) -> ParseError {
        let message = match e {
            RegistryError::UnknownField(name) => match self.nearest_field_name(&name) {
                Some(near) => format!("unknown field '{name}'; nearest known field: {near}"),
                None => format!("unknown field '{name}'"),
            },
            RegistryError::OperatorNotPermitted { field, op } => {
                format!("field '{field}' does not permit operator {op:?}")
            }
            RegistryError::MatchNotPermitted(field) => {
                format!("field '{field}' does not support glob/prefix matching")
            }
        };
        ParseError {
            message,
            span: Some((start, end)),
        }
    }

    /// Nearest registered field/alias by Levenshtein distance, within a
    /// small typo-sized budget — a coarser suggestion isn't worth showing.
    fn nearest_field_name(&self, name: &str) -> Option<&str> {
        const MAX_DISTANCE: usize = 2;
        self.reg
            .field_names()
            .map(|candidate| (candidate, levenshtein(name, candidate)))
            .filter(|(_, d)| *d <= MAX_DISTANCE)
            .min_by_key(|(_, d)| *d)
            .map(|(candidate, _)| candidate)
    }

    fn typed_value(
        &self,
        raw: &str,
        ty: FieldType,
        start: usize,
        end: usize,
    ) -> Result<Value, ParseError> {
        match ty {
            FieldType::Text => Ok(Value::Text(raw.to_string())),
            // `parse_size_bytes` (P1.6) only recognizes uppercase K/M, matching
            // real Aminet INDEX data; a typed query value is user input, where
            // the phase doc's own grammar sketch calls for lowercase `k`/`M`
            // too — upper-casing here keeps one size-suffix parser instead of
            // forking it.
            FieldType::Int => parse_size_bytes(&raw.to_ascii_uppercase())
                .map(Value::Int)
                .ok_or_else(|| ParseError {
                    message: format!("invalid integer value '{raw}'"),
                    span: Some((start, end)),
                }),
            FieldType::Date => Ok(Value::Date(raw.to_string())),
        }
    }

    fn parse_or_expr(&mut self) -> Result<Predicate, ParseError> {
        let mut parts = vec![self.parse_and_expr()?];
        loop {
            self.skip_ws();
            if !self.peek_is_or_keyword() {
                break;
            }
            self.pos += 2; // "OR"
            parts.push(self.parse_and_expr()?);
        }
        Ok(one_or_and(parts, Predicate::Or))
    }

    fn parse_and_expr(&mut self) -> Result<Predicate, ParseError> {
        let mut results: Vec<Predicate> = Vec::new();
        let mut run: Option<(usize, usize)> = None; // (start, end) of the current bareword run

        loop {
            self.skip_ws();
            match self.peek() {
                None | Some(b')') => break,
                _ if self.peek_is_or_keyword() => break,
                _ => {}
            }
            let (pred, start, end) = self.parse_unary()?;
            if matches!(pred, Predicate::FullText(_)) {
                run = Some(match run {
                    Some((s, _)) => (s, end),
                    None => (start, end),
                });
            } else {
                if let Some((s, e)) = run.take() {
                    results.push(Predicate::FullText(self.src[s..e].to_string()));
                }
                results.push(pred);
            }
        }
        if let Some((s, e)) = run.take() {
            results.push(Predicate::FullText(self.src[s..e].to_string()));
        }
        if results.is_empty() {
            return Err(ParseError {
                message: "expected a term".to_string(),
                span: Some((self.pos, self.pos)),
            });
        }
        Ok(one_or_and(results, Predicate::And))
    }

    fn parse_unary(&mut self) -> Result<(Predicate, usize, usize), ParseError> {
        self.skip_ws();
        let start = self.pos;
        if self.peek() == Some(b'!') {
            self.pos += 1;
            let (inner, _, end) = self.parse_unary()?;
            Ok((Predicate::Not(Box::new(inner)), start, end))
        } else {
            self.parse_atom()
        }
    }

    fn parse_atom(&mut self) -> Result<(Predicate, usize, usize), ParseError> {
        self.skip_ws();
        let start = self.pos;
        if self.peek() == Some(b'(') {
            self.pos += 1;
            let inner = self.parse_or_expr()?;
            self.skip_ws();
            if self.peek() != Some(b')') {
                return Err(ParseError {
                    message: "unbalanced '('".to_string(),
                    span: Some((start, start + 1)),
                });
            }
            self.pos += 1;
            return Ok((inner, start, self.pos));
        }
        if self.peek().is_none() {
            return Err(ParseError {
                message: "expected a term".to_string(),
                span: Some((self.pos, self.pos)),
            });
        }
        if let Some(pred) = self.try_special_term()? {
            return Ok((pred, start, self.pos));
        }
        let pred = self.parse_term()?;
        Ok((pred, start, self.pos))
    }

    /// `in:'name'`, `marked`, `similar:'text' > threshold`. Rewinds and
    /// returns `None` if `pos` isn't one of these three reserved words.
    fn try_special_term(&mut self) -> Result<Option<Predicate>, ParseError> {
        let checkpoint = self.pos;
        let Some((name, _, _)) = self.lex_ident() else {
            return Ok(None);
        };
        match name.as_str() {
            "in" if self.peek() == Some(b':') => {
                self.pos += 1;
                let (value, _, _) = self.lex_rhs()?;
                Ok(Some(Predicate::InSelection(SelectionRef::Named(value))))
            }
            "marked" if !matches!(self.peek(), Some(b':')) => {
                Ok(Some(Predicate::InSelection(SelectionRef::Marked)))
            }
            "similar" if self.peek() == Some(b':') => {
                self.pos += 1;
                let (text, _, _) = self.lex_rhs()?;
                self.skip_ws();
                let threshold = if self.peek() == Some(b'>') {
                    self.pos += 1;
                    self.skip_ws();
                    let (raw, start, end) = self.lex_rhs()?;
                    raw.parse::<f32>().map_err(|_| ParseError {
                        message: format!("invalid similarity threshold '{raw}'"),
                        span: Some((start, end)),
                    })?
                } else {
                    0.0
                };
                Ok(Some(Predicate::Similar { text, threshold }))
            }
            _ => {
                self.pos = checkpoint;
                Ok(None)
            }
        }
    }

    /// `field:rhs`, `field:~rhs`, `field cmp_op value`, or a bareword.
    fn parse_term(&mut self) -> Result<Predicate, ParseError> {
        let restart = self.pos;
        let Some((name, ident_start, ident_end)) = self.lex_ident() else {
            let (raw, _, _) = self.lex_bareword_token();
            return Ok(Predicate::FullText(raw));
        };

        if self.peek() == Some(b':') {
            self.pos += 1;
            let forced_match = if self.peek() == Some(b'~') {
                self.pos += 1;
                true
            } else {
                false
            };
            let (raw, start, end) = self.lex_rhs()?;
            let field = self
                .reg
                .resolve(&name)
                .map_err(|e| self.field_error(e, ident_start, ident_end))?;
            return if forced_match || raw.contains('*') {
                self.reg
                    .check_match(field)
                    .map_err(|e| self.field_error(e, ident_start, ident_end))?;
                let pattern = if raw.contains('*') {
                    Pattern::Glob(raw)
                } else {
                    Pattern::Prefix(raw)
                };
                Ok(Predicate::Match {
                    field: field.id.clone(),
                    pattern,
                })
            } else {
                self.reg
                    .check_compare(field, CmpOp::Eq)
                    .map_err(|e| self.field_error(e, ident_start, ident_end))?;
                let value = self.typed_value(&raw, field.ty, start, end)?;
                Ok(Predicate::Compare {
                    field: field.id.clone(),
                    op: CmpOp::Eq,
                    value,
                })
            };
        }

        if let Some(op) = self.try_lex_cmp_op() {
            let (raw, start, end) = self.lex_rhs()?;
            let field = self
                .reg
                .resolve(&name)
                .map_err(|e| self.field_error(e, ident_start, ident_end))?;
            self.reg
                .check_compare(field, op)
                .map_err(|e| self.field_error(e, ident_start, ident_end))?;
            let value = self.typed_value(&raw, field.ty, start, end)?;
            return Ok(Predicate::Compare {
                field: field.id.clone(),
                op,
                value,
            });
        }

        // Not a term after all: the identifier is just the start of a
        // bareword (e.g. `tracker-mod` lexes an ident "tracker" but isn't
        // followed by `:`/an operator) — rewind and take the whole token.
        self.pos = restart;
        let (raw, _, _) = self.lex_bareword_token();
        Ok(Predicate::FullText(raw))
    }
}

/// Collapses a single-element list to its one element (no `And([x])` /
/// `Or([x])` wrapper), matching `docs/lang-bam-dsl.md`'s worked examples.
fn one_or_and(
    mut parts: Vec<Predicate>,
    wrap: impl FnOnce(Vec<Predicate>) -> Predicate,
) -> Predicate {
    if parts.len() == 1 {
        parts.pop().unwrap()
    } else {
        wrap(parts)
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn render(p: &Predicate) -> String {
    match p {
        Predicate::And(parts) => parts
            .iter()
            .map(render_and_child)
            .collect::<Vec<_>>()
            .join(" "),
        Predicate::Or(parts) => parts
            .iter()
            .map(render_or_child)
            .collect::<Vec<_>>()
            .join(" OR "),
        Predicate::Not(inner) => match inner.as_ref() {
            Predicate::And(_) | Predicate::Or(_) => format!("!({})", render(inner)),
            _ => format!("!{}", render(inner)),
        },
        Predicate::Compare { field, op, value } => render_compare(field, *op, value),
        Predicate::Match { field, pattern } => render_match(field, pattern),
        Predicate::FullText(s) => s.clone(),
        Predicate::InSelection(SelectionRef::Named(name)) => format!("in:'{name}'"),
        Predicate::InSelection(SelectionRef::Marked) => "marked".to_string(),
        Predicate::Similar { text, threshold } => format!("similar:'{text}' > {threshold}"),
    }
}

fn render_and_child(p: &Predicate) -> String {
    match p {
        Predicate::And(_) | Predicate::Or(_) => format!("({})", render(p)),
        _ => render(p),
    }
}

fn render_or_child(p: &Predicate) -> String {
    match p {
        Predicate::Or(_) => format!("({})", render(p)),
        _ => render(p),
    }
}

fn render_compare(field: &FieldId, op: CmpOp, value: &Value) -> String {
    let v = render_value(value);
    match op {
        CmpOp::Eq => format!("{}:{v}", field.0),
        CmpOp::Ne => format!("{}!={v}", field.0),
        CmpOp::Lt => format!("{}<{v}", field.0),
        CmpOp::Le => format!("{}<={v}", field.0),
        CmpOp::Gt => format!("{}>{v}", field.0),
        CmpOp::Ge => format!("{}>={v}", field.0),
    }
}

fn render_match(field: &FieldId, pattern: &Pattern) -> String {
    match pattern {
        Pattern::Glob(g) => format!("{}:{g}", field.0),
        Pattern::Prefix(p) => format!("{}:~{p}", field.0),
    }
}

fn render_value(v: &Value) -> String {
    match v {
        Value::Text(s) if s.is_empty() || s.chars().any(char::is_whitespace) => format!("'{s}'"),
        Value::Text(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Date(s) => s.clone(),
    }
}
