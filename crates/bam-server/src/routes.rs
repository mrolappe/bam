//! HTTP/SSE handlers (P9.2). Each one adapts a request/response pair onto
//! `bam_core::api` — no SQL, no query logic, matching P0.4's purity check's
//! spirit for this crate too.

use std::convert::Infallible;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Extension, Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, http::StatusCode};
use bam_core::api;
use bam_core::progress::{OperationId, ProgressEvent};
use futures_util::stream;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::session_layer::session_middleware;
use crate::state::{AppState, SessionHandle, terminal_event};

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/search-packages", post(search_packages))
        .route("/api/search-window", post(search_window))
        .route("/api/get-package", post(get_package))
        .route("/api/get-inventory", post(get_inventory))
        .route("/api/parse-query", post(parse_query))
        .route("/api/filter-ids", post(filter_ids))
        .route("/api/list-categories", post(list_categories))
        .route("/api/select-by-query", post(select_by_query))
        .route("/api/save-as", post(save_as))
        .route("/api/load", post(load))
        .route("/api/delete-selection", post(delete_selection))
        .route("/api/list-selections", post(list_selections))
        .route("/api/mark", post(mark))
        .route("/api/unmark", post(unmark))
        .route("/api/toggle", post(toggle))
        .route("/api/is-marked", post(is_marked))
        .route("/api/clear", post(clear))
        .route("/api/start-ingest", post(start_ingest))
        .route("/api/operation-status", post(operation_status))
        .route("/api/progress/{operation}", get(progress_stream))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            session_middleware,
        ))
        .with_state(state)
}

/// Wraps `bam_core::api::Error` (a plain `SessionError`) as a JSON error
/// response — the only place this crate translates a core error into HTTP.
/// `span` survives a `Parse` variant so the frontend's query input can
/// highlight the offending byte range (P3.5's reference behaviour), rather
/// than flattening every error down to a bare message.
struct ApiError {
    message: String,
    span: Option<(usize, usize)>,
}

impl From<api::Error> for ApiError {
    fn from(e: api::Error) -> Self {
        let span = match &e {
            api::Error::Parse(parse_err) => parse_err.span,
            _ => None,
        };
        ApiError {
            message: e.to_string(),
            span,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": self.message, "span": self.span })),
        )
            .into_response()
    }
}

/// Runs `f` on `handle`'s actor thread and turns its `Result` into a JSON
/// response or an [`ApiError`] — the shape every plain (non-ingest,
/// non-empty-body) handler below reduces to.
async fn adapt<Req, Res>(
    handle: SessionHandle,
    req: Req,
    f: impl FnOnce(&api::Session, &Req) -> Result<Res, api::Error> + Send + 'static,
) -> Result<Response, ApiError>
where
    Req: Send + 'static,
    Res: Serialize + Send + 'static,
{
    let result = handle.call(move |cs| f(&cs.session, &req)).await?;
    Ok(Json(result).into_response())
}

macro_rules! route {
    ($name:ident, $req:ty, $call:expr) => {
        async fn $name(
            Extension(handle): Extension<SessionHandle>,
            Json(req): Json<$req>,
        ) -> Result<Response, ApiError> {
            adapt(handle, req, $call).await
        }
    };
}

route!(
    search_packages,
    api::SearchPackagesRequest,
    api::search_packages
);
route!(search_window, api::SearchWindowRequest, api::search_window);
route!(get_package, api::GetPackageRequest, api::get_package);
route!(get_inventory, api::GetInventoryRequest, api::get_inventory);
route!(parse_query, api::ParseQueryRequest, api::parse_query);
route!(filter_ids, api::FilterIdsRequest, api::filter_ids);
route!(
    select_by_query,
    api::SelectByQueryRequest,
    api::select_by_query
);

#[derive(Deserialize)]
struct Empty {}

async fn list_categories(
    Extension(handle): Extension<SessionHandle>,
    Json(_req): Json<Empty>,
) -> Result<Response, ApiError> {
    adapt(handle, (), |session, _| api::list_categories(session)).await
}

async fn list_selections(
    Extension(handle): Extension<SessionHandle>,
    Json(_req): Json<Empty>,
) -> Result<Response, ApiError> {
    adapt(handle, (), |session, _| api::list(session)).await
}

async fn save_as(
    Extension(handle): Extension<SessionHandle>,
    Json(req): Json<api::SaveAsRequest>,
) -> Result<StatusCode, ApiError> {
    handle
        .call(move |cs| api::save_as(&cs.session, &req))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn load(
    Extension(handle): Extension<SessionHandle>,
    Json(req): Json<api::LoadRequest>,
) -> Result<StatusCode, ApiError> {
    handle.call(move |cs| api::load(&cs.session, &req)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_selection(
    Extension(handle): Extension<SessionHandle>,
    Json(req): Json<api::DeleteSelectionRequest>,
) -> Result<StatusCode, ApiError> {
    handle
        .call(move |cs| api::delete(&cs.session, &req))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct PackageIdRequest {
    package_id: i64,
}

#[derive(Serialize)]
struct MarkedResponse {
    marked: bool,
}

async fn mark(
    Extension(handle): Extension<SessionHandle>,
    Json(req): Json<PackageIdRequest>,
) -> Result<StatusCode, ApiError> {
    handle
        .call(move |cs| api::mark(&cs.session, req.package_id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn unmark(
    Extension(handle): Extension<SessionHandle>,
    Json(req): Json<PackageIdRequest>,
) -> Result<StatusCode, ApiError> {
    handle
        .call(move |cs| api::unmark(&cs.session, req.package_id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn clear(Extension(handle): Extension<SessionHandle>) -> Result<StatusCode, ApiError> {
    handle.call(move |cs| api::clear(&cs.session)).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn toggle(
    Extension(handle): Extension<SessionHandle>,
    Json(req): Json<PackageIdRequest>,
) -> Result<Json<MarkedResponse>, ApiError> {
    let marked = handle
        .call(move |cs| api::toggle(&cs.session, req.package_id))
        .await?;
    Ok(Json(MarkedResponse { marked }))
}

async fn is_marked(
    Extension(handle): Extension<SessionHandle>,
    Json(req): Json<PackageIdRequest>,
) -> Result<Json<MarkedResponse>, ApiError> {
    let marked = handle
        .call(move |cs| api::is_marked(&cs.session, req.package_id))
        .await?;
    Ok(Json(MarkedResponse { marked }))
}

async fn operation_status(
    Extension(handle): Extension<SessionHandle>,
    Json(req): Json<api::OperationStatusRequest>,
) -> Json<api::OperationStatusResponse> {
    Json(
        handle
            .call(move |cs| api::operation_status(&cs.session, &req))
            .await,
    )
}

#[derive(Serialize)]
struct StartIngestResponse {
    operation: OperationId,
}

/// Starts the ingest on the session's actor thread, which keeps it running
/// independent of this request and of any particular SSE connection
/// (P9.2's fourth test: reconnecting re-attaches rather than orphaning
/// it), replying as soon as the operation id is known rather than waiting
/// for the ingest to finish.
async fn start_ingest(
    Extension(handle): Extension<SessionHandle>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<api::StartIngestRequest>,
) -> Json<StartIngestResponse> {
    let operation = handle.start_ingest(req, state.http.clone()).await;
    Json(StartIngestResponse { operation })
}

fn sse_event(event: &ProgressEvent) -> Event {
    Event::default().data(serde_json::to_string(event).expect("ProgressEvent serializes"))
}

enum StreamState {
    Live(broadcast::Receiver<ProgressEvent>),
    Single(ProgressEvent),
    Done,
}

/// SSE progress stream (P9.2's third and fourth tests). If this session's
/// active ingest is still `operation`, subscribes to its live broadcast
/// channel — the same channel a reconnecting client re-subscribes to,
/// since the actor thread (not this handler) owns it and keeps running
/// regardless of who is listening. Otherwise, if the operation is known but
/// already finished, synthesizes one terminal event from `operation_status`
/// so a reconnecting client still resolves cleanly instead of hanging.
async fn progress_stream(
    Extension(handle): Extension<SessionHandle>,
    Path(operation): Path<u64>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let operation = OperationId(operation);
    let initial = match handle.subscribe_if_active(operation) {
        Some(rx) => StreamState::Live(rx),
        None => {
            let status = handle
                .call(move |cs| cs.session.operation_status(operation))
                .await;
            match status.and_then(|status| terminal_event(operation, status)) {
                Some(event) => StreamState::Single(event),
                None => StreamState::Done,
            }
        }
    };

    let stream = stream::unfold(initial, |mut state| async move {
        loop {
            state = match state {
                StreamState::Live(mut rx) => match rx.recv().await {
                    Ok(event) => {
                        let finished = matches!(event, ProgressEvent::Finished { .. });
                        let next = if finished {
                            StreamState::Done
                        } else {
                            StreamState::Live(rx)
                        };
                        return Some((Ok(sse_event(&event)), next));
                    }
                    // Missed events under a slow client: keep the
                    // connection alive and pick up with whatever comes
                    // next, rather than dropping the stream.
                    Err(broadcast::error::RecvError::Lagged(_)) => StreamState::Live(rx),
                    Err(broadcast::error::RecvError::Closed) => return None,
                },
                StreamState::Single(event) => {
                    return Some((Ok(sse_event(&event)), StreamState::Done));
                }
                StreamState::Done => return None,
            };
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
