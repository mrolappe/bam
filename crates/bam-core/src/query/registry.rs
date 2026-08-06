//! The field registry (invariant I2): maps a name or alias to a type, the
//! comparison operators it permits, and where the compiler (P2.5) reads it
//! from. Registering a field here is meant to be the *only* change needed
//! to make it queryable — P2.8 exists to prove that claim for `in:`/`marked`.

use thiserror::Error;

use super::ir::CmpOp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    Text,
    Int,
    Date,
}

/// Where the compiler reads a field's value from: a plain column, or a
/// column reached through a join (e.g. a future `EXISTS` subquery over
/// `selection_member` for `in:`/`marked` — P2.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlSource {
    Column(&'static str),
    Join {
        join_sql: &'static str,
        column: &'static str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDef {
    pub id: super::ir::FieldId,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub ty: FieldType,
    pub ops: &'static [CmpOp],
    pub source: SqlSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    #[error("unknown field '{0}'")]
    UnknownField(String),
    #[error("field '{field}' does not permit operator {op:?}")]
    OperatorNotPermitted { field: String, op: CmpOp },
    #[error("field '{0}' does not support glob/prefix matching")]
    MatchNotPermitted(String),
}

pub struct FieldRegistry {
    fields: Vec<FieldDef>,
}

impl FieldRegistry {
    pub fn new(fields: Vec<FieldDef>) -> Self {
        Self { fields }
    }

    /// Resolves a field by its primary name or any alias.
    pub fn resolve(&self, name: &str) -> Result<&FieldDef, RegistryError> {
        self.fields
            .iter()
            .find(|f| f.name == name || f.aliases.contains(&name))
            .ok_or_else(|| RegistryError::UnknownField(name.to_string()))
    }

    /// Checks that `field` permits `op` in a `Compare` predicate.
    pub fn check_compare(&self, field: &FieldDef, op: CmpOp) -> Result<(), RegistryError> {
        if field.ops.contains(&op) {
            Ok(())
        } else {
            Err(RegistryError::OperatorNotPermitted {
                field: field.name.to_string(),
                op,
            })
        }
    }

    /// Checks that `field` permits a `Match` (glob/prefix) predicate.
    /// Only `Text`-typed fields do — a numeric or date field being matched
    /// against a glob (`size:~'foo'`) is a resolve-time error, not a SQL one.
    pub fn check_match(&self, field: &FieldDef) -> Result<(), RegistryError> {
        if field.ty == FieldType::Text {
            Ok(())
        } else {
            Err(RegistryError::MatchNotPermitted(field.name.to_string()))
        }
    }
}

/// The initial field set, mapped to P1.2's `package` table. `type` and
/// `author` from `bam-handoff.md`'s §11 examples are deliberately absent:
/// neither has a backing column yet (`type` awaits a derived category,
/// `author` awaits Phase 4's readme harvesting) — registering a field with
/// no `SqlSource` to point at would be speculative, not initial.
pub fn package_fields() -> Vec<FieldDef> {
    use super::ir::FieldId;
    use CmpOp::*;
    use FieldType::*;
    use SqlSource::Column;

    const TEXT_OPS: &[CmpOp] = &[Eq, Ne];
    const ORD_OPS: &[CmpOp] = &[Eq, Ne, Lt, Le, Gt, Ge];

    vec![
        FieldDef {
            id: FieldId::new("dir"),
            name: "dir",
            aliases: &[],
            ty: Text,
            ops: TEXT_OPS,
            source: Column("dir"),
        },
        FieldDef {
            id: FieldId::new("file"),
            name: "file",
            aliases: &[],
            ty: Text,
            ops: TEXT_OPS,
            source: Column("file"),
        },
        FieldDef {
            id: FieldId::new("name"),
            name: "name",
            aliases: &[],
            ty: Text,
            ops: TEXT_OPS,
            source: Column("name"),
        },
        FieldDef {
            id: FieldId::new("version"),
            name: "version",
            aliases: &["ver"],
            ty: Text,
            ops: TEXT_OPS,
            source: Column("version"),
        },
        FieldDef {
            id: FieldId::new("size"),
            name: "size",
            aliases: &["size_bytes"],
            ty: Int,
            ops: ORD_OPS,
            source: Column("size_bytes"),
        },
        FieldDef {
            id: FieldId::new("date"),
            name: "date",
            aliases: &["uploaded_on"],
            ty: Date,
            ops: ORD_OPS,
            source: Column("uploaded_on"),
        },
        // Extracted from `uploaded_on` by the compiler, which must also
        // consult `date_precision` — a `week`-precision row near a year
        // boundary can't be asserted into a `year>` range (P2.5).
        FieldDef {
            id: FieldId::new("year"),
            name: "year",
            aliases: &[],
            ty: Int,
            ops: ORD_OPS,
            source: Column("uploaded_on"),
        },
        FieldDef {
            id: FieldId::new("description"),
            name: "description",
            aliases: &["desc"],
            ty: Text,
            ops: TEXT_OPS,
            source: Column("description"),
        },
    ]
}
