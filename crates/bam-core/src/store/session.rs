//! Session-scoped API handle (P2.6, invariant I5): the seam every adapter —
//! TUI, GUI, and eventually a web/MCP server — calls through instead of
//! touching `rusqlite` or the compiler directly. No global state: a
//! `Session` owns its connection, its own operation table, and its own
//! working selection, so two sessions never observe each other.
//!
//! Also P2.7 (invariant I7): `mark`/`unmark`/`toggle`/`clear`/
//! `select_by_query`/`save_as`/`load`/`list_selections`/`delete_selection`
//! operate on the session's ephemeral *working* selection — the same row
//! `store::compile`'s `marked_selection_id` and `SelectionRef::Marked`
//! resolve to.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::compile::{self, CompileError};
use super::ingest::{self, IngestError, IngestMode};
use super::tables::{self, Package, Selection};
use crate::cancel::CancellationToken;
use crate::http::HttpClient;
use crate::progress::{OperationId, Outcome, ProgressEvent, ProgressSink};
use crate::query::bam_dsl::BamDsl;
use crate::query::ir::{Predicate, SelectionRef};
use crate::query::lang::{LanguageError, LanguageRegistry, ParseError};
use crate::query::registry::{FieldRegistry, package_fields};

#[derive(Debug, Error)]
pub enum SessionError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Compile(#[from] CompileError),
    #[error(transparent)]
    Ingest(#[from] IngestError),
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Language(#[from] LanguageError),
    #[error("no selection named '{0}'")]
    UnknownSelection(String),
}

/// Last known status of an operation started through [`Session::run_ingest`],
/// queryable by [`OperationId`] so a reconnecting web client re-attaches
/// instead of orphaning a running ingest (invariant I5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum OperationStatus {
    Running { done: u64, total: Option<u64> },
    Finished(Outcome),
    Cancelled,
}

/// How [`Session::select_by_query`]'s matches combine with the current
/// working selection (invariant I7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SelectionMode {
    Replace,
    Union,
    Intersect,
    Subtract,
}

pub struct Session {
    conn: Connection,
    registry: FieldRegistry,
    langs: LanguageRegistry,
    working_selection_id: i64,
    operations: Mutex<HashMap<OperationId, OperationStatus>>,
    next_operation: AtomicU64,
}

impl Session {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SessionError> {
        Self::from_connection(super::open(path)?)
    }

    pub fn in_memory() -> Result<Self, SessionError> {
        Self::from_connection(super::open(":memory:")?)
    }

    fn from_connection(conn: Connection) -> Result<Self, SessionError> {
        let working_selection_id = tables::insert_selection(
            &conn,
            &Selection {
                id: 0,
                name: None,
                created_at: crate::now_rfc3339(),
                ephemeral: true,
            },
        )?;
        let mut langs = LanguageRegistry::new("bam-dsl");
        langs.register(Box::new(BamDsl));
        Ok(Self {
            conn,
            registry: FieldRegistry::new(package_fields()),
            langs,
            working_selection_id,
            operations: Mutex::new(HashMap::new()),
            next_operation: AtomicU64::new(1),
        })
    }

    // ---- search / get / categories ----

    /// Parses query-line text through `bam-dsl` (P2.4) against this
    /// session's field registry, through the registry's default language
    /// (`bam-dsl` — the only one registered so far).
    pub fn parse_query(&self, src: &str) -> Result<Predicate, SessionError> {
        self.parse_query_lang(None, src)
    }

    /// As [`Self::parse_query`], but selects the language by id (`None` =
    /// the registry default) — P3.8's highlight rules each name their own
    /// `lang`, invariant I3.
    pub fn parse_query_lang(
        &self,
        lang: Option<&str>,
        src: &str,
    ) -> Result<Predicate, SessionError> {
        let language = self.langs.get(lang)?;
        Ok(language.parse(src, &self.registry)?)
    }

    pub fn search_packages(&self, pred: &Predicate) -> Result<Vec<Package>, SessionError> {
        self.matching_ids(pred)?
            .into_iter()
            .map(|id| tables::get_package(&self.conn, id).map_err(SessionError::from))
            .collect()
    }

    /// A page of `limit` matches starting at `offset`, plus the total match
    /// count — the shape P3.4's TUI list needs to query only its visible
    /// window rather than materializing every matching [`Package`] (which,
    /// at Aminet's ~84,000-package scale, is the difference the phase doc's
    /// virtualization requirement exists to avoid). Ordered by `id` so
    /// repeated windowed calls against an unchanged result set are stable.
    pub fn search_window(
        &self,
        pred: &Predicate,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<Package>, usize), SessionError> {
        let compiled = self.compiled_for(pred)?;
        let total: usize = self.conn.query_row(
            &format!("SELECT COUNT(*) FROM ({})", compiled.sql),
            params_from_iter(compiled.params.iter()),
            |r| r.get(0),
        )?;
        let mut window_params = compiled.params.clone();
        window_params.push(SqlValue::Integer(limit as i64));
        window_params.push(SqlValue::Integer(offset as i64));
        let mut stmt = self.conn.prepare(&format!(
            "SELECT id FROM ({}) ORDER BY id LIMIT ? OFFSET ?",
            compiled.sql
        ))?;
        let ids: Vec<i64> = stmt
            .query_map(params_from_iter(window_params.iter()), |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let packages = ids
            .into_iter()
            .map(|id| tables::get_package(&self.conn, id).map_err(SessionError::from))
            .collect::<Result<_, _>>()?;
        Ok((packages, total))
    }

    pub fn get_package(&self, id: i64) -> Result<Option<Package>, SessionError> {
        match tables::get_package(&self.conn, id) {
            Ok(p) => Ok(Some(p)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_categories(&self) -> Result<Vec<String>, SessionError> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT dir FROM package ORDER BY dir")?;
        let categories = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(categories)
    }

    fn matching_ids(&self, pred: &Predicate) -> Result<Vec<i64>, SessionError> {
        let compiled = self.compiled_for(pred)?;
        let mut stmt = self.conn.prepare(&compiled.sql)?;
        let ids = stmt
            .query_map(params_from_iter(compiled.params.iter()), |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    /// Matches `pred` restricted to `ids` (P3.8): the highlight engine only
    /// needs to know which of the *currently visible* rows a rule hits, not
    /// the whole table. `compiled_for` runs regardless of whether `ids` is
    /// empty, so calling with `ids: &[]` doubles as a load-time validation
    /// trial — a rule whose predicate is well-typed but fails to compile
    /// (e.g. `Similar`, not yet supported) is caught here without a real row.
    pub fn matching_ids_among(
        &self,
        pred: &Predicate,
        ids: &[i64],
    ) -> Result<Vec<i64>, SessionError> {
        let compiled = self.compiled_for(pred)?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = vec!["?"; ids.len()].join(",");
        let mut params = compiled.params.clone();
        params.extend(ids.iter().map(|id| SqlValue::Integer(*id)));
        let mut stmt = self.conn.prepare(&format!(
            "SELECT id FROM ({}) WHERE id IN ({placeholders})",
            compiled.sql
        ))?;
        let matched = stmt
            .query_map(params_from_iter(params.iter()), |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(matched)
    }

    fn compiled_for(&self, pred: &Predicate) -> Result<compile::CompiledQuery, SessionError> {
        self.check_named_selections_exist(pred)?;
        Ok(compile::compile(
            pred,
            &self.registry,
            Some(self.working_selection_id),
        )?)
    }

    /// `compile::compile` has no `Connection`, so a `in:'name'` for a
    /// selection that doesn't exist would otherwise just compile to an
    /// `EXISTS` clause matching nothing — silently returning zero results
    /// instead of erroring (P2.8). The existence check belongs here, the
    /// one place that has both the predicate tree and a connection.
    fn check_named_selections_exist(&self, pred: &Predicate) -> Result<(), SessionError> {
        match pred {
            Predicate::And(parts) | Predicate::Or(parts) => parts
                .iter()
                .try_for_each(|p| self.check_named_selections_exist(p)),
            Predicate::Not(inner) => self.check_named_selections_exist(inner),
            Predicate::InSelection(SelectionRef::Named(name)) => self
                .conn
                .query_row(
                    "SELECT 1 FROM selection WHERE name = ?1",
                    params![name],
                    |_| Ok(()),
                )
                .optional()?
                .ok_or_else(|| SessionError::UnknownSelection(name.clone())),
            _ => Ok(()),
        }
    }

    // ---- selections (P2.7, invariant I7) ----

    pub fn mark(&self, package_id: i64) -> Result<(), SessionError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO selection_member (selection_id, package_id) VALUES (?1, ?2)",
            params![self.working_selection_id, package_id],
        )?;
        Ok(())
    }

    pub fn unmark(&self, package_id: i64) -> Result<(), SessionError> {
        self.conn.execute(
            "DELETE FROM selection_member WHERE selection_id = ?1 AND package_id = ?2",
            params![self.working_selection_id, package_id],
        )?;
        Ok(())
    }

    pub fn is_marked(&self, package_id: i64) -> Result<bool, SessionError> {
        let found = self
            .conn
            .query_row(
                "SELECT 1 FROM selection_member WHERE selection_id = ?1 AND package_id = ?2",
                params![self.working_selection_id, package_id],
                |_| Ok(()),
            )
            .optional()?;
        Ok(found.is_some())
    }

    /// Idempotent flip; two calls with the same `package_id` return to the
    /// original membership state.
    pub fn toggle(&self, package_id: i64) -> Result<bool, SessionError> {
        if self.is_marked(package_id)? {
            self.unmark(package_id)?;
            Ok(false)
        } else {
            self.mark(package_id)?;
            Ok(true)
        }
    }

    pub fn clear(&self) -> Result<(), SessionError> {
        self.conn.execute(
            "DELETE FROM selection_member WHERE selection_id = ?1",
            params![self.working_selection_id],
        )?;
        Ok(())
    }

    /// Combines `pred`'s matches into the working selection per `mode`.
    /// Returns the working selection's member count afterward.
    pub fn select_by_query(
        &self,
        pred: &Predicate,
        mode: SelectionMode,
    ) -> Result<usize, SessionError> {
        let matched = self.matching_ids(pred)?;
        match mode {
            SelectionMode::Replace => {
                self.clear()?;
                for id in &matched {
                    self.mark(*id)?;
                }
            }
            SelectionMode::Union => {
                for id in &matched {
                    self.mark(*id)?;
                }
            }
            SelectionMode::Intersect => {
                let matched: HashSet<i64> = matched.into_iter().collect();
                for id in self.member_ids(self.working_selection_id)? {
                    if !matched.contains(&id) {
                        self.unmark(id)?;
                    }
                }
            }
            SelectionMode::Subtract => {
                for id in &matched {
                    self.unmark(*id)?;
                }
            }
        }
        self.member_count(self.working_selection_id)
    }

    fn member_ids(&self, selection_id: i64) -> Result<Vec<i64>, SessionError> {
        let mut stmt = self
            .conn
            .prepare("SELECT package_id FROM selection_member WHERE selection_id = ?1")?;
        let ids = stmt
            .query_map(params![selection_id], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    fn member_count(&self, selection_id: i64) -> Result<usize, SessionError> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM selection_member WHERE selection_id = ?1",
            params![selection_id],
            |r| r.get(0),
        )?;
        Ok(count as usize)
    }

    fn copy_members(&self, from: i64, to: i64) -> Result<(), SessionError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO selection_member (selection_id, package_id)
             SELECT ?1, package_id FROM selection_member WHERE selection_id = ?2",
            params![to, from],
        )?;
        Ok(())
    }

    /// Snapshots the working selection's current members under `name`,
    /// independent of further `mark`/`unmark` on the working selection.
    pub fn save_as(&self, name: &str) -> Result<(), SessionError> {
        let saved_id = tables::insert_selection(
            &self.conn,
            &Selection {
                id: 0,
                name: Some(name.to_string()),
                created_at: crate::now_rfc3339(),
                ephemeral: false,
            },
        )?;
        self.copy_members(self.working_selection_id, saved_id)
    }

    /// Replaces the working selection's members with the named selection's.
    pub fn load(&self, name: &str) -> Result<(), SessionError> {
        let selection_id: i64 = self
            .conn
            .query_row(
                "SELECT id FROM selection WHERE name = ?1",
                params![name],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| SessionError::UnknownSelection(name.to_string()))?;
        self.clear()?;
        self.copy_members(selection_id, self.working_selection_id)
    }

    pub fn list_selections(&self) -> Result<Vec<(String, String, usize)>, SessionError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, created_at FROM selection WHERE ephemeral = 0 ORDER BY name",
        )?;
        let rows: Vec<(i64, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        rows.into_iter()
            .map(|(id, name, created_at)| Ok((name, created_at, self.member_count(id)?)))
            .collect()
    }

    pub fn delete_selection(&self, name: &str) -> Result<(), SessionError> {
        let changed = self.conn.execute(
            "DELETE FROM selection WHERE name = ?1 AND ephemeral = 0",
            params![name],
        )?;
        if changed == 0 {
            return Err(SessionError::UnknownSelection(name.to_string()));
        }
        Ok(())
    }

    // ---- long-running: ingest (invariant I5) ----

    fn new_operation(&self) -> OperationId {
        let id = OperationId(self.next_operation.fetch_add(1, Ordering::SeqCst));
        self.operations.lock().unwrap().insert(
            id,
            OperationStatus::Running {
                done: 0,
                total: None,
            },
        );
        id
    }

    fn record(&self, id: OperationId, event: &ProgressEvent) {
        let status = match event {
            ProgressEvent::Started { total, .. } => OperationStatus::Running {
                done: 0,
                total: *total,
            },
            ProgressEvent::Advanced { done, .. } => {
                let total = match self.operations.lock().unwrap().get(&id) {
                    Some(OperationStatus::Running { total, .. }) => *total,
                    _ => None,
                };
                OperationStatus::Running { done: *done, total }
            }
            ProgressEvent::Finished { outcome, .. } => OperationStatus::Finished(outcome.clone()),
        };
        self.operations.lock().unwrap().insert(id, status);
    }

    pub fn operation_status(&self, id: OperationId) -> Option<OperationStatus> {
        self.operations.lock().unwrap().get(&id).cloned()
    }

    /// Runs one ingest, checked against `cancel` before it starts, so a
    /// pre-cancelled call reports [`OperationStatus::Cancelled`] immediately
    /// rather than running to completion or surfacing as an error.
    /// `ingest::run_ingest` itself has no internal step granular enough to
    /// poll mid-flight (P1.10 built two steps, fetch+land and normalize;
    /// finer-grained cancellation is future work if ingest grows more of
    /// them). `sink` still
    /// receives every event a direct `ingest::run_ingest` call would emit;
    /// `operation_status(id)` reflects the same sequence for a caller that
    /// polls instead of holding `sink` open (e.g. a reconnecting web client).
    pub async fn run_ingest(
        &self,
        client: &impl HttpClient,
        mode: IngestMode,
        cancel: &CancellationToken,
        sink: &mut impl ProgressSink,
    ) -> Result<OperationId, SessionError> {
        let operation = self.new_operation();
        if cancel.is_cancelled() {
            self.operations
                .lock()
                .unwrap()
                .insert(operation, OperationStatus::Cancelled);
            return Ok(operation);
        }

        struct Tracking<'a, S> {
            session: &'a Session,
            operation: OperationId,
            inner: &'a mut S,
        }
        impl<S: ProgressSink> ProgressSink for Tracking<'_, S> {
            fn emit(&mut self, event: ProgressEvent) {
                let event = retarget(event, self.operation);
                self.session.record(self.operation, &event);
                self.inner.emit(event);
            }
        }
        fn retarget(event: ProgressEvent, operation: OperationId) -> ProgressEvent {
            match event {
                ProgressEvent::Started { total, .. } => ProgressEvent::Started { operation, total },
                ProgressEvent::Advanced { done, .. } => ProgressEvent::Advanced { operation, done },
                ProgressEvent::Finished { outcome, .. } => {
                    ProgressEvent::Finished { operation, outcome }
                }
            }
        }

        let fetched_at = crate::now_rfc3339();
        let mut tracking = Tracking {
            session: self,
            operation,
            inner: sink,
        };
        ingest::run_ingest(
            &self.conn,
            client,
            &mut tracking,
            mode,
            &fetched_at,
            operation,
        )
        .await?;
        Ok(operation)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.conn.execute(
            "DELETE FROM selection WHERE id = ?1 AND ephemeral = 1",
            params![self.working_selection_id],
        );
    }
}
