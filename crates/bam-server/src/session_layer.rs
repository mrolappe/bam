//! Cookie-based session identification, factored out of the route handlers
//! so each one just extracts a `ClientSession` handle rather than repeating
//! cookie parsing (P9.2: a thin adapter, no logic duplicated per route).

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, Request, header};
use axum::middleware::Next;
use axum::response::Response;

use crate::state::{AppState, SESSION_COOKIE};

pub async fn session_middleware(
    State(state): State<Arc<AppState>>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let hint = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_session_cookie);

    let (id, session, is_new) = state.get_or_create(hint).await;
    req.extensions_mut().insert(session);

    let mut resp = next.run(req).await;
    if is_new {
        let cookie = format!("{SESSION_COOKIE}={id}; Path=/; HttpOnly; SameSite=Lax");
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            resp.headers_mut().insert(header::SET_COOKIE, value);
        }
    }
    resp
}

fn parse_session_cookie(header_val: &str) -> Option<u64> {
    header_val.split(';').map(str::trim).find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == SESSION_COOKIE).then(|| v.parse().ok()).flatten()
    })
}
