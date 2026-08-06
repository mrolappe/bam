//! The one long-running operation the core currently has: ingest.
//! Exercises invariant I5's `CancellationToken`/`OperationId` rules against
//! real work, rather than inventing a fake long operation just to have
//! something to cancel.

use super::Error;
use super::types::{OperationStatusRequest, OperationStatusResponse, StartIngestRequest};
use crate::cancel::CancellationToken;
use crate::http::HttpClient;
use crate::progress::{OperationId, ProgressSink};
use crate::store::session::Session;

pub async fn start_ingest(
    session: &Session,
    client: &impl HttpClient,
    req: &StartIngestRequest,
    cancel: &CancellationToken,
    sink: &mut impl ProgressSink,
) -> Result<OperationId, Error> {
    session.run_ingest(client, req.mode, cancel, sink).await
}

pub fn operation_status(
    session: &Session,
    req: &OperationStatusRequest,
) -> OperationStatusResponse {
    OperationStatusResponse {
        status: session.operation_status(req.operation),
    }
}
