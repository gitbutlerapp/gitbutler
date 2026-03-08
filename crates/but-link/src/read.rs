//! Read-side response shaping for `but link`.

use std::collections::BTreeSet;

use rusqlite::Connection;
use serde::Serialize;
use serde_json::{Value, json};

use crate::cli::DiscoveryFormat;
use crate::db;
use crate::services::acquire::block_to_advisory_value;

mod models;

pub(crate) use models::{
    AgentPanelEntry, AgentsSnapshot, BlockListEntry, ClaimListEntry, ClaimsSnapshot, CursorState,
    DiscoveriesSnapshot, DiscoveryListEntry, FullSnapshot, InboxSnapshot, MessageDisplayEntry,
    MessagesSnapshot, SurfaceListEntry, TuiSnapshot,
};

/// Agent staleness window used by the TUI observer snapshot.
pub(crate) const TUI_AGENT_STALE_MS: i64 = 10 * 60 * 1000;

/// Build the inbox view for a specific agent.
fn inbox_snapshot(conn: &Connection, agent_id: &str) -> anyhow::Result<InboxSnapshot> {
    let now_ms = db::now_unix_ms()?;
    let my_active_claims = db::load_active_claims_for_agent(conn, agent_id)?;
    let my_claim_paths: Vec<String> = my_active_claims
        .iter()
        .map(|claim| claim.path.clone())
        .collect();
    let open_blocks = db::load_open_blocks(conn, Some(agent_id), None)?;
    let open_blocks_relevant_to_me: Vec<db::TypedBlock> = if my_claim_paths.is_empty() {
        Vec::new()
    } else {
        open_blocks
            .iter()
            .filter(|block| {
                block.paths.iter().any(|block_path| {
                    my_claim_paths
                        .iter()
                        .any(|claim_path| db::paths_overlap(block_path, claim_path))
                })
            })
            .cloned()
            .collect()
    };

    let pending_acks: Vec<db::TypedBlock> = open_blocks_relevant_to_me
        .iter()
        .filter(|block| {
            block.paths.is_empty()
                || block.paths.iter().any(|path| {
                    db::ack_exists_since(conn, agent_id, &block.agent_id, path, block.created_at_ms)
                        .map(|acked| !acked)
                        .unwrap_or(true)
                })
        })
        .cloned()
        .collect();

    let (mut mentions_or_directed_updates, prev_cursor, new_cursor) =
        db::unread_inbox_updates(conn, agent_id, now_ms)?;
    mentions_or_directed_updates.extend(db::directed_typed_updates(conn, agent_id)?);
    mentions_or_directed_updates.sort_by_key(|update| (update.created_at_ms, update.id));
    let dependency_hints_relevant_to_requested_paths =
        db::dependency_hints_for_paths(conn, agent_id, &my_claim_paths)?;
    let stale_agents_holding_relevant_claims = if my_claim_paths.is_empty() {
        Vec::new()
    } else {
        db::stale_agents_for_paths(conn, &my_claim_paths, now_ms)?
    };

    let mut recent_advisories: Vec<Value> = open_blocks_relevant_to_me
        .iter()
        .filter(|block| block.mode == "advisory")
        .map(block_to_advisory_value)
        .collect();
    if my_claim_paths.is_empty() {
        recent_advisories.clear();
    } else {
        recent_advisories.extend(db::recent_discoveries_for_paths(conn, &my_claim_paths, 10)?);
    }

    Ok(InboxSnapshot {
        mentions_or_directed_updates,
        open_blocks_relevant_to_me,
        my_active_claims,
        pending_acks,
        dependency_hints_relevant_to_requested_paths,
        stale_agents_holding_relevant_claims,
        recent_advisories,
        cursor: CursorState {
            prev: prev_cursor,
            next: new_cursor,
        },
    })
}

/// Build the inbox view for a specific agent.
pub(crate) fn inbox_view(conn: &Connection, agent_id: &str) -> anyhow::Result<Value> {
    let snapshot = inbox_snapshot(conn, agent_id)?;
    Ok(json!({
        "ok": true,
        "view": "inbox",
        "mentions_or_directed_updates": snapshot.mentions_or_directed_updates,
        "open_blocks_relevant_to_me": snapshot.open_blocks_relevant_to_me,
        "my_active_claims": snapshot.my_active_claims,
        "pending_acks": snapshot.pending_acks,
        "dependency_hints_relevant_to_requested_paths": snapshot.dependency_hints_relevant_to_requested_paths,
        "stale_agents_holding_relevant_claims": snapshot.stale_agents_holding_relevant_claims,
        "recent_advisories": snapshot.recent_advisories,
        "cursor": {
            "topic": "inbox",
            "prev": snapshot.cursor.prev,
            "next": snapshot.cursor.next,
        }
    }))
}

/// Build the full transcript-style snapshot.
fn full_snapshot(conn: &Connection) -> anyhow::Result<FullSnapshot> {
    Ok(FullSnapshot {
        messages: db::load_messages_since(conn, None, 0)?,
        discoveries: db::load_discoveries(conn, false)?
            .into_iter()
            .map(db::discovery_to_value)
            .collect(),
        claims: db::load_active_claims(conn, None)?,
        agents: db::load_agent_snapshots(conn)?,
        blocks: db::load_open_blocks(conn, None, None)?,
        surfaces: db::load_surface_declarations(conn)?,
    })
}

/// Build the full transcript-style snapshot.
pub(crate) fn full_view(conn: &Connection) -> anyhow::Result<Value> {
    let snapshot = full_snapshot(conn)?;
    Ok(json!({
        "ok": true,
        "view": "full",
        "messages": snapshot.messages,
        "discoveries": snapshot.discoveries,
        "claims": snapshot.claims,
        "agents": snapshot.agents,
        "blocks": snapshot.blocks,
        "surfaces": snapshot.surfaces,
    }))
}

/// Build the discovery-only view.
fn discoveries_snapshot(conn: &Connection) -> anyhow::Result<DiscoveriesSnapshot> {
    let discoveries = db::load_discoveries(conn, false)?;
    let next_steps = discoveries
        .iter()
        .filter_map(|record| {
            record
                .suggested_cmd
                .as_ref()
                .map(|cmd| json!({ "kind": "run", "cmd": cmd }))
        })
        .collect();
    Ok(DiscoveriesSnapshot {
        discoveries: discoveries
            .into_iter()
            .map(db::discovery_to_value)
            .collect(),
        next_steps,
    })
}

/// Build the messages-only view.
fn messages_snapshot(conn: &Connection) -> anyhow::Result<MessagesSnapshot> {
    Ok(MessagesSnapshot {
        messages: db::load_messages_since(conn, Some("message"), 0)?,
    })
}

/// Build the messages-only view.
pub(crate) fn messages_view(conn: &Connection) -> anyhow::Result<Value> {
    let snapshot = messages_snapshot(conn)?;
    collection_view("messages", "messages", snapshot.messages)
}

/// Build the claims-only view.
fn claims_snapshot(conn: &Connection) -> anyhow::Result<ClaimsSnapshot> {
    Ok(ClaimsSnapshot {
        claims: db::load_active_claims(conn, None)?,
    })
}

/// Build the claims-only view.
pub(crate) fn claims_view(conn: &Connection) -> anyhow::Result<Value> {
    let snapshot = claims_snapshot(conn)?;
    collection_view("claims", "claims", snapshot.claims)
}

/// Build the agents-only view.
fn agents_snapshot(conn: &Connection) -> anyhow::Result<AgentsSnapshot> {
    Ok(AgentsSnapshot {
        agents: db::load_agent_snapshots(conn)?,
    })
}

/// Build the agents-only view.
pub(crate) fn agents_view(conn: &Connection) -> anyhow::Result<Value> {
    let snapshot = agents_snapshot(conn)?;
    collection_view("agents", "agents", snapshot.agents)
}

/// Build the discovery view using the selected format.
pub(crate) fn discoveries_view_with_format(
    conn: &Connection,
    format: DiscoveryFormat,
) -> anyhow::Result<Value> {
    let snapshot = discoveries_snapshot(conn)?;
    Ok(discoveries_response(snapshot, format))
}

/// Load TUI display messages within the provided recent window.
fn load_message_display_entries(
    conn: &Connection,
    since_ms: i64,
    limit: i64,
) -> anyhow::Result<Vec<MessageDisplayEntry>> {
    let records = db::load_message_records_since_ms(conn, Some("message"), since_ms, Some(limit))?;
    Ok(records
        .into_iter()
        .map(|record| {
            let content = message_record_content(&record);
            MessageDisplayEntry {
                created_at_ms: record.created_at_ms,
                agent_id: record.agent_id,
                content,
            }
        })
        .collect())
}

/// Load TUI agent rows from shared snapshots.
pub(crate) fn load_agent_panel_entries(conn: &Connection) -> anyhow::Result<Vec<AgentPanelEntry>> {
    let snapshot = agents_snapshot(conn)?;
    Ok(snapshot
        .agents
        .into_iter()
        .map(|agent| AgentPanelEntry {
            agent_id: agent.agent_id,
            status: agent.status,
            plan: agent.plan,
            last_seen_at_ms: agent.last_seen_at_ms,
            last_progress_at_ms: agent.last_progress_at_ms,
        })
        .collect())
}

/// Load TUI claim rows from shared snapshots.
pub(crate) fn load_claim_list_entries(conn: &Connection) -> anyhow::Result<Vec<ClaimListEntry>> {
    let snapshot = claims_snapshot(conn)?;
    Ok(snapshot
        .claims
        .into_iter()
        .map(|claim| ClaimListEntry {
            path: claim.path,
            agent_id: claim.agent_id,
        })
        .collect())
}

/// Load TUI block rows from shared snapshots.
pub(crate) fn load_block_list_entries(conn: &Connection) -> anyhow::Result<Vec<BlockListEntry>> {
    Ok(db::load_open_blocks(conn, None, None)?
        .into_iter()
        .map(|block| BlockListEntry {
            id: block.id,
            agent_id: block.agent_id,
            mode: block.mode,
            reason: block.reason,
            paths: block.paths,
        })
        .collect())
}

/// Load a shared TUI snapshot from the current typed storage.
pub(crate) fn load_tui_snapshot(
    conn: &Connection,
    since_ms: i64,
    message_limit: i64,
    now_ms: i64,
) -> anyhow::Result<TuiSnapshot> {
    let discoveries = db::load_discoveries(conn, false)?;
    let surfaces = db::load_surface_declarations(conn)?;
    let claims = load_claim_list_entries(conn)?;
    let blocks = load_block_list_entries(conn)?;
    let agents = filter_tui_agents(load_agent_panel_entries(conn)?, &claims, &blocks, now_ms);
    Ok(TuiSnapshot {
        messages: load_message_display_entries(conn, since_ms, message_limit)?,
        agents,
        claims,
        blocks,
        discoveries: discoveries
            .into_iter()
            .map(|discovery| DiscoveryListEntry {
                title: discovery.title,
                agent_id: discovery.agent_id,
                evidence: discovery.evidence,
            })
            .collect(),
        surfaces: surfaces
            .into_iter()
            .map(|surface| SurfaceListEntry {
                kind: surface.kind,
                agent_id: surface.agent_id,
                scope: surface.scope,
                surface: surface.surface,
                paths: surface.paths,
            })
            .collect(),
    })
}

/// Build a discovery response for the selected output format.
fn discoveries_response(snapshot: DiscoveriesSnapshot, format: DiscoveryFormat) -> Value {
    let discoveries = match format {
        DiscoveryFormat::Full | DiscoveryFormat::Brief => snapshot.discoveries,
        DiscoveryFormat::Digest => snapshot
            .discoveries
            .into_iter()
            .map(|discovery| match discovery {
                Value::Object(mut map) => {
                    let title = map.remove("title");
                    let agent_id = map.remove("agent_id");
                    json!({ "title": title, "agent_id": agent_id })
                }
                other => other,
            })
            .collect(),
    };
    json!({
        "ok": true,
        "view": "discoveries",
        "format": match format {
            DiscoveryFormat::Full => "full",
            DiscoveryFormat::Brief => "brief",
            DiscoveryFormat::Digest => "digest",
        },
        "discoveries": discoveries,
        "next_steps": snapshot.next_steps,
    })
}

/// Build a simple collection response for one read view.
fn collection_view<T: Serialize>(
    view: &'static str,
    key: &'static str,
    value: T,
) -> anyhow::Result<Value> {
    let mut object = serde_json::Map::new();
    object.insert("ok".to_owned(), Value::Bool(true));
    object.insert("view".to_owned(), Value::String(view.to_owned()));
    object.insert(key.to_owned(), serde_json::to_value(value)?);
    Ok(Value::Object(object))
}

/// Extract free-text content from a stored message record.
fn message_record_content(record: &db::HistoryMessageRecord) -> String {
    record
        .body_json
        .parse::<Value>()
        .ok()
        .and_then(|body| body.get("text").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| record.body_json.clone())
}

/// Filter the TUI agent panel down to recent or still-relevant rows.
fn filter_tui_agents(
    mut agents: Vec<AgentPanelEntry>,
    claims: &[ClaimListEntry],
    blocks: &[BlockListEntry],
    now_ms: i64,
) -> Vec<AgentPanelEntry> {
    let cutoff_ms = now_ms.saturating_sub(TUI_AGENT_STALE_MS);
    let agents_with_claims: BTreeSet<&str> =
        claims.iter().map(|claim| claim.agent_id.as_str()).collect();
    let agents_with_blocks: BTreeSet<&str> =
        blocks.iter().map(|block| block.agent_id.as_str()).collect();
    agents.retain(|agent| {
        agent.last_progress_at_ms >= cutoff_ms
            || agents_with_claims.contains(agent.agent_id.as_str())
            || agents_with_blocks.contains(agent.agent_id.as_str())
    });
    agents
}
