//! `search_packages`, `get_package`, `list_categories` — the three named
//! use cases from P2.6's task text. Thin: each just adapts a typed
//! request/response pair onto a [`Session`] call.

use super::Error;
use super::types::{
    GetPackageRequest, GetPackageResponse, ListCategoriesResponse, SearchPackagesRequest,
    SearchPackagesResponse,
};
use crate::store::session::Session;

pub fn search_packages(
    session: &Session,
    req: &SearchPackagesRequest,
) -> Result<SearchPackagesResponse, Error> {
    let packages = session.search_packages(&req.predicate)?;
    Ok(SearchPackagesResponse { packages })
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
