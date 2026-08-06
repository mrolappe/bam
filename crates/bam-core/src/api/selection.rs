//! Selection operations (P2.7, invariant I7). `mark`/`unmark`/`toggle`/
//! `clear` take a bare `package_id` rather than a wrapper request type —
//! one primitive field doesn't earn a named schema type the way
//! [`SelectByQueryRequest`](super::types::SelectByQueryRequest) or a
//! by-name lookup does.

use super::Error;
use super::types::{
    DeleteSelectionRequest, ListSelectionsResponse, LoadRequest, SaveAsRequest,
    SelectByQueryRequest, SelectByQueryResponse, SelectionSummary,
};
use crate::store::session::Session;

pub fn mark(session: &Session, package_id: i64) -> Result<(), Error> {
    session.mark(package_id)
}

pub fn unmark(session: &Session, package_id: i64) -> Result<(), Error> {
    session.unmark(package_id)
}

/// Returns the resulting membership state (`true` = now marked).
pub fn toggle(session: &Session, package_id: i64) -> Result<bool, Error> {
    session.toggle(package_id)
}

pub fn clear(session: &Session) -> Result<(), Error> {
    session.clear()
}

pub fn select_by_query(
    session: &Session,
    req: &SelectByQueryRequest,
) -> Result<SelectByQueryResponse, Error> {
    let member_count = session.select_by_query(&req.predicate, req.mode)?;
    Ok(SelectByQueryResponse { member_count })
}

pub fn save_as(session: &Session, req: &SaveAsRequest) -> Result<(), Error> {
    session.save_as(&req.name)
}

pub fn load(session: &Session, req: &LoadRequest) -> Result<(), Error> {
    session.load(&req.name)
}

pub fn list(session: &Session) -> Result<ListSelectionsResponse, Error> {
    let selections = session
        .list_selections()?
        .into_iter()
        .map(|(name, created_at, member_count)| SelectionSummary {
            name,
            created_at,
            member_count,
        })
        .collect();
    Ok(ListSelectionsResponse { selections })
}

pub fn delete(session: &Session, req: &DeleteSelectionRequest) -> Result<(), Error> {
    session.delete_selection(&req.name)
}
