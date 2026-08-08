//! Per-client session state (invariant I5: sessions, not global state).
//!
//! `bam_core::store::session::Session` wraps a SQLite connection object
//! that is not `Sync` (P0.4/I1 never asked it to be — its only prior
//! caller, `bam-tui`, is single-threaded). Rather than forcing `Sync` onto
//! `bam-core` to fit axum's multi-threaded executor, each session gets its
//! own dedicated OS thread with a single-threaded Tokio runtime and owns
//! its `Session` for that thread's whole lifetime; HTTP handlers talk to it
//! only through [`SessionHandle`], sending it plain, `Send` closures over a
//! channel. `Session` itself never crosses a thread boundary.
//!
//! `active_ingest` (which operation, if any, is currently live, and its
//! broadcast channel) lives in its own `std::sync::Mutex`, outside that
//! channel: an ingest job occupies the actor thread for its whole duration
//! (P9.2's "one connection" model), so an SSE progress request that had to
//! queue behind it would only ever learn about progress after the ingest
//! had already finished. Reading this mutex directly lets a progress
//! request subscribe immediately, while the ingest itself is still running.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};

use bam_core::api::{self, CancellationToken, Session};
use bam_core::http::ReqwestClient;
use bam_core::progress::{OperationId, Outcome, ProgressEvent, ProgressSink};
use tokio::sync::{Mutex as AsyncMutex, broadcast, mpsc, oneshot};

pub const SESSION_COOKIE: &str = "bam_session";

pub struct ClientSession {
    pub session: Session,
}

type ActiveIngest = Arc<StdMutex<Option<(OperationId, broadcast::Sender<ProgressEvent>)>>>;

type SyncJob = Box<dyn FnOnce(&mut ClientSession) + Send>;

enum Job {
    Sync(SyncJob),
    StartIngest {
        req: api::StartIngestRequest,
        http: ReqwestClient,
        reply: oneshot::Sender<OperationId>,
    },
}

/// A handle to one session's dedicated actor thread. Cheap to clone (an
/// mpsc sender plus an `Arc`); every clone still talks to the same actor,
/// and therefore the same `Session`.
#[derive(Clone)]
pub struct SessionHandle {
    tx: mpsc::UnboundedSender<Job>,
    active: ActiveIngest,
}

impl SessionHandle {
    fn spawn(db_path: PathBuf) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let active: ActiveIngest = Arc::new(StdMutex::new(None));
        let active_for_actor = active.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("bam-server: session actor runtime");
            rt.block_on(actor_loop(db_path, rx, active_for_actor));
        });
        Self { tx, active }
    }

    /// Runs `f` against the session's `ClientSession` on its own thread,
    /// returning its result. `f` must not block — the actor thread has no
    /// other work to interleave with.
    pub async fn call<R, F>(&self, f: F) -> R
    where
        R: Send + 'static,
        F: FnOnce(&mut ClientSession) -> R + Send + 'static,
    {
        let (reply_tx, reply_rx) = oneshot::channel();
        let job: SyncJob = Box::new(move |cs| {
            let _ = reply_tx.send(f(cs));
        });
        let _ = self.tx.send(Job::Sync(job));
        reply_rx.await.expect("session actor alive")
    }

    /// Starts an ingest on the actor thread and returns as soon as its
    /// `OperationId` is known (the `Started` event, emitted before any
    /// network I/O) — the ingest itself keeps running on the actor after
    /// this call returns, independent of the HTTP request that started it.
    pub async fn start_ingest(
        &self,
        req: api::StartIngestRequest,
        http: ReqwestClient,
    ) -> OperationId {
        let (reply, reply_rx) = oneshot::channel();
        let _ = self.tx.send(Job::StartIngest { req, http, reply });
        reply_rx.await.expect("session actor alive")
    }

    /// Subscribes to `operation`'s live progress channel if it is still
    /// this session's active ingest — synchronous and immediate, never
    /// queued behind the actor (see the module docs for why that matters).
    pub fn subscribe_if_active(
        &self,
        operation: OperationId,
    ) -> Option<broadcast::Receiver<ProgressEvent>> {
        match &*self.active.lock().unwrap() {
            Some((id, tx)) if *id == operation => Some(tx.subscribe()),
            _ => None,
        }
    }
}

async fn actor_loop(db_path: PathBuf, mut rx: mpsc::UnboundedReceiver<Job>, active: ActiveIngest) {
    let session = Session::open(&db_path).expect("bam-server: session db opens");
    let mut client = ClientSession { session };
    while let Some(job) = rx.recv().await {
        match job {
            Job::Sync(f) => f(&mut client),
            Job::StartIngest { req, http, reply } => {
                let (tx, _rx) = broadcast::channel(64);
                let cancel = CancellationToken::new();
                let mut sink = RelaySink {
                    tx,
                    active: active.clone(),
                    started: Some(reply),
                };
                let _ = api::start_ingest(&client.session, &http, &req, &cancel, &mut sink).await;
            }
        }
    }
}

/// Relays every progress event to `tx` (for SSE subscribers) and keeps
/// `active` in sync: set to this operation on `Started` (once its id is
/// known and before any network I/O — early enough that the HTTP handler
/// waiting to reply with it barely notices), cleared again on `Finished`
/// so a request arriving afterward falls back to reading the now-final
/// status instead of subscribing to a channel nothing will ever send to
/// again.
struct RelaySink {
    tx: broadcast::Sender<ProgressEvent>,
    active: ActiveIngest,
    started: Option<oneshot::Sender<OperationId>>,
}

impl ProgressSink for RelaySink {
    fn emit(&mut self, event: ProgressEvent) {
        match &event {
            ProgressEvent::Started { operation, .. } => {
                *self.active.lock().unwrap() = Some((*operation, self.tx.clone()));
                if let Some(started) = self.started.take() {
                    let _ = started.send(*operation);
                }
            }
            ProgressEvent::Finished { .. } => {
                *self.active.lock().unwrap() = None;
            }
            ProgressEvent::Advanced { .. } => {}
        }
        let _ = self.tx.send(event);
    }
}

/// One terminal [`ProgressEvent`] synthesized from `operation_status`, for
/// an SSE client reconnecting to an operation that already finished.
pub fn terminal_event(
    operation: OperationId,
    status: bam_core::api::OperationStatus,
) -> Option<ProgressEvent> {
    use bam_core::api::OperationStatus;
    match status {
        OperationStatus::Finished(outcome) => Some(ProgressEvent::Finished { operation, outcome }),
        OperationStatus::Cancelled => Some(ProgressEvent::Finished {
            operation,
            outcome: Outcome::Failed {
                message: "cancelled".into(),
            },
        }),
        OperationStatus::Running { .. } => None,
    }
}

pub struct AppState {
    db_path: PathBuf,
    pub http: ReqwestClient,
    sessions: AsyncMutex<HashMap<u64, SessionHandle>>,
    next_id: AtomicU64,
}

impl AppState {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            http: ReqwestClient::default(),
            sessions: AsyncMutex::new(HashMap::new()),
            next_id: AtomicU64::new(0),
        }
    }

    /// Looks up `hint` (the cookie the client sent, if any); spawns a fresh
    /// session actor against the shared db file otherwise. Returns the id
    /// to hand back, the session handle, and whether it is new (so the
    /// caller knows to set the cookie).
    pub async fn get_or_create(&self, hint: Option<u64>) -> (u64, SessionHandle, bool) {
        let mut sessions = self.sessions.lock().await;
        if let Some(existing) = hint.and_then(|id| sessions.get(&id)) {
            return (hint.unwrap(), existing.clone(), false);
        }

        let id = self.fresh_id();
        let handle = SessionHandle::spawn(self.db_path.clone());
        sessions.insert(id, handle.clone());
        (id, handle, true)
    }

    // ponytail: plain hashed counter, not a CSPRNG token — fine for a
    // localhost/trusted-network dev tool; swap for a proper random token
    // (e.g. from a `rand` CSPRNG) before exposing this beyond that.
    fn fresh_id(&self) -> u64 {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u64(n);
        hasher.finish()
    }
}
