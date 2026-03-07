//! Read-side response shaping for `but link`.

use rusqlite::Connection;
use serde_json::{Value, json};

use crate::claiming::block_to_advisory_value;
use crate::db;

/// Build the inbox view for a specific agent.
pub(crate) fn inbox_view(conn: &Connection, agent_id: &str) -> anyhow::Result<Value> {
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

    let (mentions_or_directed_updates, prev_cursor, new_cursor) =
        db::unread_inbox_updates(conn, agent_id, now_ms)?;
    let dependency_hints_relevant_to_requested_paths =
        db::dependency_hints_for_paths(conn, agent_id, &my_claim_paths)?;
    let stale_agents_holding_relevant_claims = if my_claim_paths.is_empty() {
        Vec::new()
    } else {
        db::stale_agents_for_paths(conn, &my_claim_paths, now_ms)?
    };

    let mut recent_advisories: Vec<Value> = open_blocks
        .iter()
        .filter(|block| block.mode == "advisory")
        .map(block_to_advisory_value)
        .collect();
    if my_claim_paths.is_empty() {
        recent_advisories.clear();
    } else {
        recent_advisories.extend(db::recent_discoveries_for_paths(conn, &my_claim_paths, 10)?);
    }

    Ok(json!({
        "ok": true,
        "view": "inbox",
        "mentions_or_directed_updates": mentions_or_directed_updates,
        "open_blocks_relevant_to_me": open_blocks_relevant_to_me,
        "my_active_claims": my_active_claims,
        "pending_acks": pending_acks,
        "dependency_hints_relevant_to_requested_paths": dependency_hints_relevant_to_requested_paths,
        "stale_agents_holding_relevant_claims": stale_agents_holding_relevant_claims,
        "recent_advisories": recent_advisories,
        "cursor": {
            "topic": "inbox",
            "prev": prev_cursor,
            "next": new_cursor,
        }
    }))
}

/// Build the full transcript-style snapshot.
pub(crate) fn full_view(conn: &Connection) -> anyhow::Result<Value> {
    let (discoveries, next_steps) = db::load_discoveries_and_next_steps(conn, "discovery", true)?;
    Ok(json!({
        "ok": true,
        "view": "full",
        "messages": db::load_messages_since(conn, None, 0)?,
        "discoveries": discoveries,
        "next_steps": next_steps,
        "claims": db::load_active_claims(conn, None)?,
        "agents": db::load_agent_snapshots(conn)?,
        "blocks": db::load_open_blocks(conn, None, None)?,
    }))
}

/// Build the discovery-only view.
pub(crate) fn discoveries_view(conn: &Connection) -> anyhow::Result<Value> {
    let (discoveries, next_steps) = db::load_discoveries_and_next_steps(conn, "discovery", true)?;
    Ok(json!({
        "ok": true,
        "view": "discoveries",
        "discoveries": discoveries,
        "next_steps": next_steps,
    }))
}

/// Build the messages-only view.
pub(crate) fn messages_view(conn: &Connection) -> anyhow::Result<Value> {
    Ok(json!({
        "ok": true,
        "view": "messages",
        "messages": db::load_messages_since(conn, None, 0)?,
    }))
}

/// Build the claims-only view.
pub(crate) fn claims_view(conn: &Connection) -> anyhow::Result<Value> {
    Ok(json!({
        "ok": true,
        "view": "claims",
        "claims": db::load_active_claims(conn, None)?,
    }))
}

/// Build the agents-only view.
pub(crate) fn agents_view(conn: &Connection) -> anyhow::Result<Value> {
    Ok(json!({
        "ok": true,
        "view": "agents",
        "agents": db::load_agent_snapshots(conn)?,
    }))
}

/// Build the discovery brief view.
pub(crate) fn brief_view(conn: &Connection, kind: &str, all: bool) -> anyhow::Result<Value> {
    let (discoveries, next_steps) = db::load_discoveries_and_next_steps(conn, kind, all)?;
    Ok(json!({
        "ok": true,
        "mode": "brief",
        "kind": kind,
        "discoveries": discoveries,
        "next_steps": next_steps,
    }))
}

/// Build the discovery digest view.
pub(crate) fn digest_view(conn: &Connection, kind: &str, all: bool) -> anyhow::Result<Value> {
    let (discoveries, next_steps) = db::load_discoveries_and_next_steps(conn, kind, all)?;
    let discoveries: Vec<Value> = discoveries
        .into_iter()
        .map(|discovery| match discovery {
            Value::Object(mut map) => {
                let title = map.remove("title");
                let agent_id = map.remove("agent_id");
                json!({ "title": title, "agent_id": agent_id })
            }
            other => other,
        })
        .collect();
    Ok(json!({
        "ok": true,
        "mode": "digest",
        "kind": kind,
        "discoveries": discoveries,
        "next_steps": next_steps,
    }))
}
