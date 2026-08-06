//! The TUI's query seam (P3.4): a windowed fetch, so the app can depend on
//! something narrower than [`bam_core::api::Session`] and tests can inject a
//! fake that counts calls without a database.

use bam_core::api::{self, Session};
use bam_core::query::ir::Predicate;
use bam_core::store::tables::Package;

#[derive(Debug, Clone, PartialEq)]
pub struct WindowResult {
    pub packages: Vec<Package>,
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoreError(pub String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for StoreError {}

/// `limit` matches starting at `offset`, plus the total match count.
/// Implementations must not fetch more than `limit` rows — that's what lets
/// the app's virtualized list stay flat against an 84,000-row result set.
pub trait PackageStore {
    fn window(
        &mut self,
        pred: &Predicate,
        offset: usize,
        limit: usize,
    ) -> Result<WindowResult, StoreError>;
}

pub struct SessionStore {
    session: Session,
}

impl SessionStore {
    pub fn new(session: Session) -> Self {
        Self { session }
    }
}

impl PackageStore for SessionStore {
    fn window(
        &mut self,
        pred: &Predicate,
        offset: usize,
        limit: usize,
    ) -> Result<WindowResult, StoreError> {
        let resp = api::search_window(
            &self.session,
            &api::SearchWindowRequest {
                predicate: pred.clone(),
                offset,
                limit,
            },
        )
        .map_err(|e| StoreError(e.to_string()))?;
        Ok(WindowResult {
            packages: resp.packages,
            total: resp.total,
        })
    }
}
