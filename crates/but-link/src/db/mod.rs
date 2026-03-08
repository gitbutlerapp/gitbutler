//! Database initialization, migrations, and typed coordination queries.
//!
//! All database access shared across command handlers and the TUI lives here.

use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use rusqlite::{Connection, Transaction};
use serde::Serialize;
use serde_json::Value;

mod agents;
mod blocks;
mod claims;
mod dependencies;
mod discoveries;
mod messages;
mod migrations;
mod surfaces;

pub(crate) use agents::*;
pub(crate) use blocks::*;
pub(crate) use claims::*;
pub(crate) use dependencies::*;
pub(crate) use discoveries::*;
pub(crate) use messages::*;
pub(crate) use migrations::*;
pub(crate) use surfaces::*;

/// Agent snapshot used by commands and the TUI.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct AgentSnapshot {
    /// Stable agent identifier.
    pub agent_id: String,
    /// Optional short status string.
    pub status: Option<String>,
    /// Optional short plan string.
    pub plan: Option<String>,
    /// Last observed command from the agent.
    pub last_seen_at_ms: i64,
    /// Last command that counts as progress.
    pub last_progress_at_ms: i64,
}

/// Active claim row used in structured responses.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ActiveClaim {
    /// Claimed repo-relative path.
    pub path: String,
    /// Owning agent identifier.
    pub agent_id: String,
    /// Claim expiry in unix milliseconds.
    pub expires_at_ms: i64,
}

/// Typed coordination block used in structured responses.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct TypedBlock {
    /// Block identifier.
    pub id: i64,
    /// Agent that created the block.
    pub agent_id: String,
    /// `hard` or `advisory`.
    pub mode: String,
    /// Human-readable reason.
    pub reason: String,
    /// Covered repo-relative paths.
    pub paths: Vec<String>,
    /// Creation timestamp in unix milliseconds.
    pub created_at_ms: i64,
    /// Optional expiry timestamp in unix milliseconds.
    pub expires_at_ms: Option<i64>,
    /// Optional resolution timestamp in unix milliseconds.
    pub resolved_at_ms: Option<i64>,
    /// Optional resolving agent.
    pub resolved_by_agent_id: Option<String>,
}

/// Structured discovery row loaded from typed storage.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct DiscoveryRecord {
    /// Discovery identifier.
    pub id: i64,
    /// Creation timestamp in unix milliseconds.
    pub created_at_ms: i64,
    /// Agent that created the discovery.
    pub agent_id: String,
    /// Short discovery title.
    pub title: String,
    /// Supporting evidence details.
    pub evidence: Vec<String>,
    /// Optional signal level.
    pub signal: Option<String>,
    /// Suggested next command when present.
    pub suggested_cmd: Option<String>,
}

/// Structured intent/declaration row loaded from typed storage.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct SurfaceDeclaration {
    /// Row identifier.
    pub id: i64,
    /// Creation timestamp in unix milliseconds.
    pub created_at_ms: i64,
    /// Owning agent.
    pub agent_id: String,
    /// `intent` or `declaration`.
    pub kind: String,
    /// Shared scope string.
    pub scope: String,
    /// Attached tags.
    pub tags: Vec<String>,
    /// Declared surface tokens.
    pub surface: Vec<String>,
    /// Optional path scopes.
    pub paths: Vec<String>,
}

/// Dependency hint emitted when intents and declarations overlap.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct DependencyHint {
    /// Stable kind tag for machine consumers.
    pub kind: &'static str,
    /// Agent that declared the dependency surface.
    pub provider_agent_id: String,
    /// Shared scope of the surface.
    pub scope: String,
    /// Tags attached to the declaration.
    pub tags: Vec<String>,
    /// Overlapping surface tokens.
    pub overlap_tokens: Vec<String>,
    /// Optional overlapping scoped paths.
    pub overlap_paths: Vec<String>,
    /// Human-readable explanation.
    pub why: String,
}

/// Stale claim holder summary.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct StaleAgent {
    /// Stable kind tag for machine consumers.
    pub kind: &'static str,
    /// Agent id of the stale holder.
    pub agent_id: String,
    /// Last progress timestamp.
    pub last_progress_at_ms: i64,
    /// How long the holder has been stale.
    pub stale_for_ms: i64,
    /// Configured stale threshold.
    pub threshold_ms: i64,
    /// Whether the holder is stale.
    pub is_stale: bool,
    /// Relevant claimed paths owned by the stale agent.
    pub claim_paths: Vec<String>,
}

/// Unread inbox entry surfaced by `read`.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct UnreadUpdate {
    /// Message id cursor.
    pub id: i64,
    /// Creation timestamp.
    pub created_at_ms: i64,
    /// Sender agent.
    pub agent_id: String,
    /// Message kind.
    pub kind: String,
    /// Parsed body payload.
    pub body: Value,
}

/// Claim detail selected for a requester.
#[derive(Clone, Debug)]
pub(crate) struct SelfClaimState {
    /// `active` or `stale`.
    pub status: &'static str,
    /// Matching claimed path.
    pub path: String,
    /// Claim expiry in unix milliseconds.
    pub expires_at_ms: i64,
}

/// Raw history row used by shared read-side model builders.
#[derive(Clone, Debug)]
pub(crate) struct HistoryMessageRecord {
    /// Creation timestamp.
    pub created_at_ms: i64,
    /// Sending agent.
    pub agent_id: String,
    /// Message kind.
    pub kind: String,
    /// Stored JSON body.
    pub body_json: String,
}

/// Joined block row used when grouping block query results in this module.
pub(crate) type BlockRow = (
    i64,
    String,
    String,
    String,
    i64,
    Option<i64>,
    Option<i64>,
    Option<String>,
    String,
);

/// Shared statement preparation across connections and transactions.
pub(crate) trait PrepareSql {
    /// Prepare an SQL statement on the underlying SQLite handle.
    fn prepare_query<'a>(&'a self, sql: &str) -> rusqlite::Result<rusqlite::Statement<'a>>;
}

impl PrepareSql for Connection {
    fn prepare_query<'a>(&'a self, sql: &str) -> rusqlite::Result<rusqlite::Statement<'a>> {
        self.prepare(sql)
    }
}

impl PrepareSql for Transaction<'_> {
    fn prepare_query<'a>(&'a self, sql: &str) -> rusqlite::Result<rusqlite::Statement<'a>> {
        self.prepare(sql)
    }
}

/// Return the current unix timestamp in milliseconds.
pub(crate) fn now_unix_ms() -> anyhow::Result<i64> {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock")?;
    dur.as_millis().try_into().context("timestamp overflow")
}

/// Resolve the stale coordination threshold.
pub(crate) fn coord_stale_threshold_ms() -> i64 {
    let default_s: i64 = 15 * 60;
    let seconds = std::env::var("COORD_STALE_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .filter(|value| *value >= 0)
        .unwrap_or(default_s);
    seconds.saturating_mul(1000)
}

/// Produce the path at which our db-file is located, given the `project_data_dir`.
pub(crate) fn db_path(project_data_dir: &Path) -> PathBuf {
    project_data_dir.join("but-link.db")
}
