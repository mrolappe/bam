//! IR → SQL compiler (P2.5, invariant I2). `Predicate` compiles to
//! parameterized SQL over `package`. **Every literal is a bound
//! parameter, never string-interpolated** — this is what makes §11's "the
//! LLM never emits SQL" claim true by construction rather than by review.
//!
//! `Predicate::InSelection` compiles here directly (an `EXISTS` subquery
//! over `selection`/`selection_member`, P1.2's schema) rather than through
//! `FieldRegistry`/`SqlSource`: P2.1 made `InSelection` its own IR variant
//! (not a `Compare`/`Match` against a registered field), and P2.4's
//! `in:'name'`/`marked` already parse to it — this compiler has to handle
//! it regardless of what, if anything, P2.8 later adds to the registry for
//! the same syntax. `SqlSource::Join` itself is not yet exercised by any
//! field and is left unimplemented (`CompileError::UnsupportedJoinSource`)
//! until a field actually needs it — P2.8's own task text acknowledges this
//! might turn out to be a P2.1 shape question, not a P2.5 one.

use rusqlite::types::Value as SqlValue;
use thiserror::Error;

use crate::query::ir::{CmpOp, FieldId, Pattern, Predicate, SelectionRef, Value};
use crate::query::registry::{FieldDef, FieldRegistry, RegistryError, SqlSource};

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledQuery {
    pub sql: String,
    pub params: Vec<SqlValue>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum CompileError {
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error("similarity search is not yet supported")]
    SimilarNotSupported,
    #[error("'marked' used with no working selection")]
    NoWorkingSelection,
    #[error("field '{0}' has a join source, not yet supported by the compiler")]
    UnsupportedJoinSource(&'static str),
}

/// Compiles `pred` to `SELECT id FROM package WHERE ...` plus its bound
/// parameters, in order. `marked_selection_id` is the caller's current
/// working selection (session-scoped, invariant I5) — required only if
/// `pred` contains `InSelection(Marked)`.
pub fn compile(
    pred: &Predicate,
    reg: &FieldRegistry,
    marked_selection_id: Option<i64>,
) -> Result<CompiledQuery, CompileError> {
    let mut params = Vec::new();
    let where_sql = compile_predicate(pred, reg, marked_selection_id, &mut params)?;
    Ok(CompiledQuery {
        sql: format!("SELECT id FROM package WHERE {where_sql}"),
        params,
    })
}

fn compile_predicate(
    pred: &Predicate,
    reg: &FieldRegistry,
    marked: Option<i64>,
    params: &mut Vec<SqlValue>,
) -> Result<String, CompileError> {
    match pred {
        Predicate::And(parts) => join_parts(parts, "AND", reg, marked, params),
        Predicate::Or(parts) => join_parts(parts, "OR", reg, marked, params),
        Predicate::Not(inner) => {
            let inner_sql = compile_predicate(inner, reg, marked, params)?;
            Ok(format!("NOT ({inner_sql})"))
        }
        Predicate::Compare { field, op, value } => compile_compare(field, *op, value, reg, params),
        Predicate::Match { field, pattern } => compile_match(field, pattern, reg, params),
        Predicate::FullText(text) => Ok(compile_fulltext(text, params)),
        Predicate::InSelection(sel) => compile_in_selection(sel, marked, params),
        Predicate::Similar { .. } => Err(CompileError::SimilarNotSupported),
    }
}

/// Joins `parts` with `op` (`"AND"`/`"OR"`), parenthesizing any child that
/// is itself an `And`/`Or` — always correct regardless of nesting, at the
/// cost of the odd redundant paren around an already-flat list.
fn join_parts(
    parts: &[Predicate],
    op: &str,
    reg: &FieldRegistry,
    marked: Option<i64>,
    params: &mut Vec<SqlValue>,
) -> Result<String, CompileError> {
    let mut clauses = Vec::with_capacity(parts.len());
    for part in parts {
        let clause = compile_predicate(part, reg, marked, params)?;
        clauses.push(if matches!(part, Predicate::And(_) | Predicate::Or(_)) {
            format!("({clause})")
        } else {
            clause
        });
    }
    Ok(clauses.join(&format!(" {op} ")))
}

fn column_of(field: &FieldDef) -> Result<&'static str, CompileError> {
    match field.source {
        SqlSource::Column(c) => Ok(c),
        SqlSource::Join { .. } => Err(CompileError::UnsupportedJoinSource(field.name)),
    }
}

fn cmp_op_sql(op: CmpOp) -> &'static str {
    match op {
        CmpOp::Eq => "=",
        CmpOp::Ne => "!=",
        CmpOp::Lt => "<",
        CmpOp::Le => "<=",
        CmpOp::Gt => ">",
        CmpOp::Ge => ">=",
    }
}

fn value_param(value: &Value) -> SqlValue {
    match value {
        Value::Text(s) => SqlValue::Text(s.clone()),
        Value::Int(i) => SqlValue::Integer(*i),
        // Stored as ISO 8601 TEXT (`package.uploaded_on`), compared
        // lexicographically — see `query::ir::Value::Date`.
        Value::Date(s) => SqlValue::Text(s.clone()),
    }
}

fn compile_compare(
    field_id: &FieldId,
    op: CmpOp,
    value: &Value,
    reg: &FieldRegistry,
    params: &mut Vec<SqlValue>,
) -> Result<String, CompileError> {
    let field = reg.resolve(&field_id.0)?;
    reg.check_compare(field, op)?;
    if field.name == "year" {
        return compile_year_compare(op, value, params);
    }
    let column = column_of(field)?;
    params.push(value_param(value));
    Ok(format!("{column} {} ?", cmp_op_sql(op)))
}

/// `year` shares `uploaded_on` with `date` but must additionally respect
/// `date_precision`: a `week`-precision row's true date is only known to
/// within +/-7 days, so `year>2000` can only be asserted for a row whose
/// *entire* uncertainty window has a year greater than 2000 — not just its
/// stored date. `lo`/`hi` are the extracted year at the window's early and
/// late edge (equal to each other for `exact`-precision rows, where the
/// window is a single day); each comparison operator picks whichever edge
/// makes the assertion safe to state unconditionally.
fn compile_year_compare(
    op: CmpOp,
    value: &Value,
    params: &mut Vec<SqlValue>,
) -> Result<String, CompileError> {
    let Value::Int(year) = value else {
        // `year`'s FieldType is Int (registry.rs), so `check_compare`
        // already guarantees this for any registry-validated predicate;
        // guarded here only for a hand-built `Predicate` that skipped it.
        return Ok("0".to_string());
    };
    const LO: &str = "CAST(strftime('%Y', CASE WHEN date_precision = 'exact' THEN uploaded_on ELSE date(uploaded_on, '-7 days') END) AS INTEGER)";
    const HI: &str = "CAST(strftime('%Y', CASE WHEN date_precision = 'exact' THEN uploaded_on ELSE date(uploaded_on, '+7 days') END) AS INTEGER)";
    let sql = match op {
        CmpOp::Gt => {
            params.push(SqlValue::Integer(*year));
            format!("{LO} > ?")
        }
        CmpOp::Ge => {
            params.push(SqlValue::Integer(*year));
            format!("{LO} >= ?")
        }
        CmpOp::Lt => {
            params.push(SqlValue::Integer(*year));
            format!("{HI} < ?")
        }
        CmpOp::Le => {
            params.push(SqlValue::Integer(*year));
            format!("{HI} <= ?")
        }
        CmpOp::Eq => {
            params.push(SqlValue::Integer(*year));
            params.push(SqlValue::Integer(*year));
            format!("({LO} = ? AND {HI} = ?)")
        }
        CmpOp::Ne => {
            params.push(SqlValue::Integer(*year));
            params.push(SqlValue::Integer(*year));
            format!("({LO} > ? OR {HI} < ?)")
        }
    };
    Ok(sql)
}

fn compile_match(
    field_id: &FieldId,
    pattern: &Pattern,
    reg: &FieldRegistry,
    params: &mut Vec<SqlValue>,
) -> Result<String, CompileError> {
    let field = reg.resolve(&field_id.0)?;
    reg.check_match(field)?;
    let column = column_of(field)?;
    // GLOB, not LIKE: case-sensitive, matching Aminet's case-significant
    // paths (LIKE is case-insensitive for ASCII in SQLite by default).
    let glob = match pattern {
        Pattern::Glob(g) => g.clone(),
        Pattern::Prefix(p) => format!("{p}*"),
    };
    params.push(SqlValue::Text(glob));
    Ok(format!("{column} GLOB ?"))
}

/// No FTS5 table exists until P4.6; until then a bareword search is a
/// `description LIKE` fallback, switched in this one place. `%`/`_`
/// (SQLite `LIKE` wildcards) and the escape character itself are escaped
/// so a literal search for e.g. `demo_pack` doesn't act as a wildcard —
/// common in Aminet filenames.
fn compile_fulltext(text: &str, params: &mut Vec<SqlValue>) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    params.push(SqlValue::Text(format!("%{escaped}%")));
    "description LIKE ? ESCAPE '\\'".to_string()
}

fn compile_in_selection(
    sel: &SelectionRef,
    marked: Option<i64>,
    params: &mut Vec<SqlValue>,
) -> Result<String, CompileError> {
    match sel {
        SelectionRef::Named(name) => {
            params.push(SqlValue::Text(name.clone()));
            Ok("EXISTS (SELECT 1 FROM selection_member sm \
                 JOIN selection s ON s.id = sm.selection_id \
                 WHERE s.name = ? AND sm.package_id = package.id)"
                .to_string())
        }
        SelectionRef::Marked => {
            let id = marked.ok_or(CompileError::NoWorkingSelection)?;
            params.push(SqlValue::Integer(id));
            Ok("EXISTS (SELECT 1 FROM selection_member sm \
                 WHERE sm.selection_id = ? AND sm.package_id = package.id)"
                .to_string())
        }
    }
}
