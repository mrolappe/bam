//! `bam-tauri` (P9.3): a thin Tauri host. It provides `TauriClient` its
//! `invoke` commands and `progress:{operation}` events; no UI lives here,
//! only Vue under `frontend/` (P9.1) does — this crate is a transport
//! backend, the mirror image of `bam-server`'s HTTP/SSE adapter.
//!
//! A desktop app has exactly one user, so unlike `bam-server` there is no
//! cookie-keyed session map: one [`bam_server::state::SessionHandle`] is
//! spawned at startup and shared by every command, reusing the same
//! actor-thread machinery `bam-server` already has rather than
//! reimplementing it.

use std::path::PathBuf;

use bam_core::api::{self, OperationStatusRequest};
use bam_core::http::ReqwestClient;
use bam_core::progress::{OperationId, ProgressEvent};
use bam_server::state::SessionHandle;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

struct DesktopState {
    handle: SessionHandle,
    http: ReqwestClient,
}

type CmdResult<T> = Result<T, CmdError>;

/// Mirrors `bam-server`'s `ApiError` (P9.2): `span` survives a `Parse`
/// variant so `TauriClient`'s query input can highlight the offending byte
/// range the same way the HTTP transport does, from the one place this
/// crate translates a core error into a command rejection.
#[derive(Serialize)]
struct CmdError {
    message: String,
    span: Option<(usize, usize)>,
}

fn err(e: api::Error) -> CmdError {
    let span = match &e {
        api::Error::Parse(parse_err) => parse_err.span,
        _ => None,
    };
    CmdError {
        message: e.to_string(),
        span,
    }
}

#[derive(Deserialize)]
struct PackageIdRequest {
    package_id: i64,
}

#[derive(Serialize)]
struct MarkedResponse {
    marked: bool,
}

#[tauri::command]
async fn search_packages(
    state: State<'_, DesktopState>,
    req: api::SearchPackagesRequest,
) -> CmdResult<api::SearchPackagesResponse> {
    state
        .handle
        .call(move |cs| api::search_packages(&cs.session, &req))
        .await
        .map_err(err)
}

#[tauri::command]
async fn search_window(
    state: State<'_, DesktopState>,
    req: api::SearchWindowRequest,
) -> CmdResult<api::SearchWindowResponse> {
    state
        .handle
        .call(move |cs| api::search_window(&cs.session, &req))
        .await
        .map_err(err)
}

#[tauri::command]
async fn get_package(
    state: State<'_, DesktopState>,
    req: api::GetPackageRequest,
) -> CmdResult<api::GetPackageResponse> {
    state
        .handle
        .call(move |cs| api::get_package(&cs.session, &req))
        .await
        .map_err(err)
}

#[tauri::command]
async fn parse_query(
    state: State<'_, DesktopState>,
    req: api::ParseQueryRequest,
) -> CmdResult<api::ParseQueryResponse> {
    state
        .handle
        .call(move |cs| api::parse_query(&cs.session, &req))
        .await
        .map_err(err)
}

#[tauri::command]
async fn filter_ids(
    state: State<'_, DesktopState>,
    req: api::FilterIdsRequest,
) -> CmdResult<api::FilterIdsResponse> {
    state
        .handle
        .call(move |cs| api::filter_ids(&cs.session, &req))
        .await
        .map_err(err)
}

#[tauri::command]
async fn list_categories(state: State<'_, DesktopState>) -> CmdResult<api::ListCategoriesResponse> {
    state
        .handle
        .call(|cs| api::list_categories(&cs.session))
        .await
        .map_err(err)
}

#[tauri::command]
async fn select_by_query(
    state: State<'_, DesktopState>,
    req: api::SelectByQueryRequest,
) -> CmdResult<api::SelectByQueryResponse> {
    state
        .handle
        .call(move |cs| api::select_by_query(&cs.session, &req))
        .await
        .map_err(err)
}

#[tauri::command]
async fn save_as(state: State<'_, DesktopState>, req: api::SaveAsRequest) -> CmdResult<()> {
    state
        .handle
        .call(move |cs| api::save_as(&cs.session, &req))
        .await
        .map_err(err)
}

#[tauri::command]
async fn load(state: State<'_, DesktopState>, req: api::LoadRequest) -> CmdResult<()> {
    state
        .handle
        .call(move |cs| api::load(&cs.session, &req))
        .await
        .map_err(err)
}

#[tauri::command]
async fn delete_selection(
    state: State<'_, DesktopState>,
    req: api::DeleteSelectionRequest,
) -> CmdResult<()> {
    state
        .handle
        .call(move |cs| api::delete(&cs.session, &req))
        .await
        .map_err(err)
}

#[tauri::command]
async fn list_selections(state: State<'_, DesktopState>) -> CmdResult<api::ListSelectionsResponse> {
    state
        .handle
        .call(|cs| api::list(&cs.session))
        .await
        .map_err(err)
}

/// Starts the ingest on the session actor and spawns a relay task that
/// forwards its progress broadcast to a `progress:{operation}` Tauri event
/// until `Finished` — `TauriClient::progress` (P9.1) listens for exactly
/// that event name.
#[tauri::command]
async fn start_ingest(
    app: AppHandle,
    state: State<'_, DesktopState>,
    req: api::StartIngestRequest,
) -> CmdResult<OperationId> {
    let operation = state.handle.start_ingest(req, state.http.clone()).await;
    if let Some(mut rx) = state.handle.subscribe_if_active(operation) {
        tauri::async_runtime::spawn(async move {
            let event_name = format!("progress:{}", operation.0);
            while let Ok(event) = rx.recv().await {
                let finished = matches!(event, ProgressEvent::Finished { .. });
                let _ = app.emit(&event_name, event);
                if finished {
                    break;
                }
            }
        });
    }
    Ok(operation)
}

#[tauri::command]
async fn toggle(
    state: State<'_, DesktopState>,
    req: PackageIdRequest,
) -> CmdResult<MarkedResponse> {
    let marked = state
        .handle
        .call(move |cs| api::toggle(&cs.session, req.package_id))
        .await
        .map_err(err)?;
    Ok(MarkedResponse { marked })
}

#[tauri::command]
async fn operation_status(
    state: State<'_, DesktopState>,
    operation: OperationId,
) -> CmdResult<api::OperationStatusResponse> {
    Ok(state
        .handle
        .call(move |cs| api::operation_status(&cs.session, &OperationStatusRequest { operation }))
        .await)
}

fn default_db_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    let _ = std::fs::create_dir_all(&dir);
    dir.join("bam.db")
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let db_path = default_db_path(app.handle());
            app.manage(DesktopState {
                handle: SessionHandle::spawn(db_path),
                http: ReqwestClient::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search_packages,
            search_window,
            get_package,
            parse_query,
            filter_ids,
            list_categories,
            select_by_query,
            save_as,
            load,
            delete_selection,
            list_selections,
            toggle,
            start_ingest,
            operation_status,
        ])
        .run(tauri::generate_context!())
        .expect("bam-tauri: run");
}
