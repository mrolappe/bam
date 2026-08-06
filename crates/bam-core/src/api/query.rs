//! `search_packages`, `get_package`, `list_categories` — the three named
//! use cases from P2.6's task text. Thin: each just adapts a typed
//! request/response pair onto a [`Session`] call.

use super::Error;
use super::types::{
    GetPackageRequest, GetPackageResponse, ListCategoriesResponse, SearchPackagesRequest,
    SearchPackagesResponse, SearchWindowRequest, SearchWindowResponse,
};
use crate::store::session::Session;

pub fn search_packages(
    session: &Session,
    req: &SearchPackagesRequest,
) -> Result<SearchPackagesResponse, Error> {
    let packages = session.search_packages(&req.predicate)?;
    Ok(SearchPackagesResponse { packages })
}

/// A page of results plus the total match count (P3.4) — the visible-window
/// query the TUI's virtualized list issues instead of [`search_packages`],
/// which materializes every match.
pub fn search_window(
    session: &Session,
    req: &SearchWindowRequest,
) -> Result<SearchWindowResponse, Error> {
    let (packages, total) = session.search_window(&req.predicate, req.offset, req.limit)?;
    Ok(SearchWindowResponse { packages, total })
}

pub fn get_package(
    session: &Session,
    req: &GetPackageRequest,
) -> Result<GetPackageResponse, Error> {
    let package = session.get_package(req.id)?;
    Ok(GetPackageResponse { package })
}

pub fn list_categories(session: &Session) -> Result<ListCategoriesResponse, Error> {
    let categories = session.list_categories()?;
    Ok(ListCategoriesResponse { categories })
}
