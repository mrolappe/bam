//! Request/response types for `bam_core::api` (invariant I5, rule 3): every
//! one `Serialize` + `Deserialize` + `JsonSchema`, so a future MCP tool
//! definition or web endpoint's schema is derived from the type rather than
//! hand-written (`bam-handoff.md` §8).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::progress::OperationId;
use crate::query::ir::Predicate;
use crate::store::ingest::IngestMode;
use crate::store::session::{OperationStatus, SelectionMode};
use crate::store::tables::Package;
use crate::unpack::Inventory;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SearchPackagesRequest {
    pub predicate: Predicate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SearchPackagesResponse {
    pub packages: Vec<Package>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParseQueryRequest {
    pub src: String,
    /// Selects the query language by id (P3.8, invariant I3); `None` falls
    /// back to the registry default (`bam-dsl`, the only one registered).
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParseQueryResponse {
    pub predicate: Predicate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FilterIdsRequest {
    pub predicate: Predicate,
    pub ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FilterIdsResponse {
    pub ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SearchWindowRequest {
    pub predicate: Predicate,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SearchWindowResponse {
    pub packages: Vec<Package>,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetPackageRequest {
    pub id: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetPackageResponse {
    pub package: Option<Package>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetInventoryRequest {
    pub package_id: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetInventoryResponse {
    pub inventory: Option<Inventory>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListCategoriesResponse {
    pub categories: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SelectByQueryRequest {
    pub predicate: Predicate,
    pub mode: SelectionMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SelectByQueryResponse {
    pub member_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SaveAsRequest {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LoadRequest {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeleteSelectionRequest {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SelectionSummary {
    pub name: String,
    pub created_at: String,
    pub member_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListSelectionsResponse {
    pub selections: Vec<SelectionSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StartIngestRequest {
    pub mode: IngestMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OperationStatusRequest {
    pub operation: OperationId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OperationStatusResponse {
    pub status: Option<OperationStatus>,
}
