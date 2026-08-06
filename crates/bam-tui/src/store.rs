//! The TUI's query seam (P3.4): a windowed fetch, so the app can depend on
//! something narrower than [`bam_core::api::Session`] and tests can inject a
//! fake that counts calls without a database.

use bam_core::api::{self, SelectionMode, SelectionSummary, Session};
use bam_core::query::ir::Predicate;
use bam_core::query::lang::ParseError;
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

    /// Parses query-line text into a [`Predicate`] (P3.5). Returns the
    /// parser's own span-carrying [`ParseError`] rather than [`StoreError`]
    /// so the caller can render the error under the offending byte range.
    fn parse(&self, src: &str) -> Result<Predicate, ParseError>;

    // ---- selections (P3.6, invariant I7) — all delegate to P2.7's API;
    // no selection membership is ever computed or cached by the TUI itself.

    fn toggle(&self, package_id: i64) -> Result<bool, StoreError>;
    fn is_marked(&self, package_id: i64) -> Result<bool, StoreError>;
    fn mark(&self, package_id: i64) -> Result<(), StoreError>;
    fn select_by_query(&self, pred: &Predicate, mode: SelectionMode) -> Result<usize, StoreError>;
    fn save_as(&self, name: &str) -> Result<(), StoreError>;
    fn load(&self, name: &str) -> Result<(), StoreError>;
    fn list_selections(&self) -> Result<Vec<SelectionSummary>, StoreError>;
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

    fn parse(&self, src: &str) -> Result<Predicate, ParseError> {
        api::parse_query(
            &self.session,
            &api::ParseQueryRequest {
                src: src.to_string(),
            },
        )
        .map(|resp| resp.predicate)
        .map_err(|e| match e {
            api::Error::Parse(pe) => pe,
            other => ParseError {
                message: other.to_string(),
                span: None,
            },
        })
    }

    fn toggle(&self, package_id: i64) -> Result<bool, StoreError> {
        api::toggle(&self.session, package_id).map_err(|e| StoreError(e.to_string()))
    }

    fn is_marked(&self, package_id: i64) -> Result<bool, StoreError> {
        api::is_marked(&self.session, package_id).map_err(|e| StoreError(e.to_string()))
    }

    fn mark(&self, package_id: i64) -> Result<(), StoreError> {
        api::mark(&self.session, package_id).map_err(|e| StoreError(e.to_string()))
    }

    fn select_by_query(&self, pred: &Predicate, mode: SelectionMode) -> Result<usize, StoreError> {
        api::select_by_query(
            &self.session,
            &api::SelectByQueryRequest {
                predicate: pred.clone(),
                mode,
            },
        )
        .map(|resp| resp.member_count)
        .map_err(|e| StoreError(e.to_string()))
    }

    fn save_as(&self, name: &str) -> Result<(), StoreError> {
        api::save_as(
            &self.session,
            &api::SaveAsRequest {
                name: name.to_string(),
            },
        )
        .map_err(|e| StoreError(e.to_string()))
    }

    fn load(&self, name: &str) -> Result<(), StoreError> {
        api::load(
            &self.session,
            &api::LoadRequest {
                name: name.to_string(),
            },
        )
        .map_err(|e| StoreError(e.to_string()))
    }

    fn list_selections(&self) -> Result<Vec<SelectionSummary>, StoreError> {
        api::list(&self.session)
            .map(|resp| resp.selections)
            .map_err(|e| StoreError(e.to_string()))
    }
}
