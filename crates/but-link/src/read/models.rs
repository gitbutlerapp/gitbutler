//! Shared read-side models used by both CLI JSON shaping and the TUI.

use serde::Serialize;
use serde_json::Value;

use crate::db;

/// Cursor metadata for inbox reads.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct CursorState {
    /// Previous cursor value before this read.
    pub prev: i64,
    /// Cursor value after this read.
    pub next: i64,
}

/// Structured inbox snapshot.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct InboxSnapshot {
    /// Directed updates for the reader.
    pub mentions_or_directed_updates: Vec<db::UnreadUpdate>,
    /// Open blocks overlapping the reader's active claims.
    pub open_blocks_relevant_to_me: Vec<db::TypedBlock>,
    /// Active claims owned by the reader.
    pub my_active_claims: Vec<db::ActiveClaim>,
    /// Open blocks still awaiting acknowledgement.
    pub pending_acks: Vec<db::TypedBlock>,
    /// Dependency hints relevant to the reader's current work.
    pub dependency_hints_relevant_to_requested_paths: Vec<db::DependencyHint>,
    /// Stale claim holders relevant to the reader's work.
    pub stale_agents_holding_relevant_claims: Vec<db::StaleAgent>,
    /// Advisory items relevant to the reader.
    pub recent_advisories: Vec<Value>,
    /// Inbox cursor metadata.
    pub cursor: CursorState,
}

/// Structured full read snapshot.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct FullSnapshot {
    /// Free-text transcript messages.
    pub messages: Vec<Value>,
    /// Discovery entries.
    pub discoveries: Vec<Value>,
    /// Active claims.
    pub claims: Vec<db::ActiveClaim>,
    /// Agent snapshots.
    pub agents: Vec<db::AgentSnapshot>,
    /// Open typed blocks.
    pub blocks: Vec<db::TypedBlock>,
    /// Typed intent/declaration rows.
    pub surfaces: Vec<db::SurfaceDeclaration>,
}

/// Discovery-only snapshot.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct DiscoveriesSnapshot {
    /// Discovery entries.
    pub discoveries: Vec<Value>,
    /// Suggested next steps emitted by discovery messages.
    pub next_steps: Vec<Value>,
}

/// Messages-only snapshot.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct MessagesSnapshot {
    /// Free-text transcript messages.
    pub messages: Vec<Value>,
}

/// Claims-only snapshot.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct ClaimsSnapshot {
    /// Active claims.
    pub claims: Vec<db::ActiveClaim>,
}

/// Agents-only snapshot.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct AgentsSnapshot {
    /// Agent snapshots.
    pub agents: Vec<db::AgentSnapshot>,
}

/// TUI-ready message row.
#[derive(Clone, Debug)]
pub(crate) struct MessageDisplayEntry {
    /// Message timestamp.
    pub created_at_ms: i64,
    /// Sending agent.
    pub agent_id: String,
    /// Formatted message content.
    pub content: String,
}

/// TUI-ready agent row.
#[derive(Clone, Debug)]
pub(crate) struct AgentPanelEntry {
    /// Stable agent id.
    pub agent_id: String,
    /// Optional status string.
    pub status: Option<String>,
    /// Optional plan string.
    pub plan: Option<String>,
    /// Last seen timestamp.
    pub last_seen_at_ms: i64,
    /// Last progress timestamp.
    pub last_progress_at_ms: i64,
}

/// TUI-ready claim row.
#[derive(Clone, Debug)]
pub(crate) struct ClaimListEntry {
    /// Claimed repo-relative path.
    pub path: String,
    /// Owning agent id.
    pub agent_id: String,
}

/// TUI-ready block row.
#[derive(Clone, Debug)]
pub(crate) struct BlockListEntry {
    /// Block id.
    pub id: i64,
    /// Owning agent id.
    pub agent_id: String,
    /// Block mode.
    pub mode: String,
    /// Human-readable reason.
    pub reason: String,
    /// Covered repo-relative paths.
    pub paths: Vec<String>,
}

/// TUI-ready discovery row.
#[derive(Clone, Debug)]
pub(crate) struct DiscoveryListEntry {
    /// Discovery title.
    pub title: String,
    /// Owning agent id.
    pub agent_id: String,
    /// Evidence lines.
    pub evidence: Vec<String>,
}

/// TUI-ready surface declaration row.
#[derive(Clone, Debug)]
pub(crate) struct SurfaceListEntry {
    /// `intent` or `declaration`.
    pub kind: String,
    /// Owning agent id.
    pub agent_id: String,
    /// Shared scope.
    pub scope: String,
    /// Surface tokens.
    pub surface: Vec<String>,
    /// Optional path scopes.
    pub paths: Vec<String>,
}

/// Shared snapshot polled by the TUI.
#[derive(Clone, Debug)]
pub(crate) struct TuiSnapshot {
    /// Free-text messages shown in the transcript pane.
    pub messages: Vec<MessageDisplayEntry>,
    /// Agent panel rows.
    pub agents: Vec<AgentPanelEntry>,
    /// Claim rows.
    pub claims: Vec<ClaimListEntry>,
    /// Block rows.
    pub blocks: Vec<BlockListEntry>,
    /// Discovery rows.
    pub discoveries: Vec<DiscoveryListEntry>,
    /// Surface declaration rows.
    pub surfaces: Vec<SurfaceListEntry>,
}
