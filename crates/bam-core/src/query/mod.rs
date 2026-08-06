//! The query core (invariant I2): a typed predicate IR plus a field
//! registry is the stable contract; surface syntaxes (`query::lang`) are
//! pluggable implementations over it. See `docs/query-ir.md`.

pub mod ir;
pub mod lang;
pub mod registry;
