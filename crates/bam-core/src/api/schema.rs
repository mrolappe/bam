//! JSON Schema export for every `bam_core::api` type (P9.1): one root
//! schema per type, keyed by name, the same `schema_for!` pattern
//! [`crate::query::grammar::bam_dsl_json_schema`] already uses for
//! `Predicate`. `frontend/scripts/gen-types.mjs` turns this into checked-in
//! TypeScript so the frontend never hand-maintains a mirror of these types.

use schemars::schema_for;
use serde_json::{Map, Value};

use crate::progress::{OperationId, Outcome, ProgressEvent};
use crate::query::ir::Predicate;
use crate::store::ingest::IngestMode;
use crate::store::session::{OperationStatus, SelectionMode};
use crate::store::tables::Package;

use super::types::*;

/// All request/response types plus the shared domain types they carry,
/// keyed by name. Each value is a self-contained root schema (its own
/// `$defs`), matching how `schema_for!` already works for `Predicate`.
pub fn all_schemas() -> Map<String, Value> {
    macro_rules! schemas {
        ($($ty:ty),+ $(,)?) => {{
            let mut map = Map::new();
            $(
                map.insert(
                    stringify!($ty).to_string(),
                    serde_json::to_value(schema_for!($ty)).expect("RootSchema serializes"),
                );
            )+
            map
        }};
    }

    schemas![
        Predicate,
        Package,
        OperationId,
        Outcome,
        ProgressEvent,
        OperationStatus,
        SelectionMode,
        IngestMode,
        SearchPackagesRequest,
        SearchPackagesResponse,
        ParseQueryRequest,
        ParseQueryResponse,
        FilterIdsRequest,
        FilterIdsResponse,
        SearchWindowRequest,
        SearchWindowResponse,
        GetPackageRequest,
        GetPackageResponse,
        ListCategoriesResponse,
        SelectByQueryRequest,
        SelectByQueryResponse,
        SaveAsRequest,
        LoadRequest,
        DeleteSelectionRequest,
        SelectionSummary,
        ListSelectionsResponse,
        StartIngestRequest,
        OperationStatusRequest,
        OperationStatusResponse,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_api_type_has_a_schema_with_the_right_key() {
        let schemas = all_schemas();
        assert!(schemas.contains_key("SearchPackagesRequest"));
        assert!(schemas.contains_key("ProgressEvent"));
        assert!(schemas.contains_key("Package"));
    }
}
