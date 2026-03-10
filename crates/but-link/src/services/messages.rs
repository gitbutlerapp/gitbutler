//! Service handlers for free-text and typed coordination publishing.

use rusqlite::Connection;

use crate::db;
use crate::payloads::{
    DiscoveryEvidence, DiscoveryPayload, DiscoverySuggestedAction, SurfacePayload,
};

/// Insert a free-text message.
pub(crate) fn post(conn: &Connection, agent_id: &str, message: &str) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let body_json = serde_json::json!({ "text": message }).to_string();
    db::insert_history_message(conn, now_ms, agent_id, "message", &body_json)?;
    db::touch_agent_progress_at(conn, agent_id, now_ms)?;
    Ok(())
}

/// Insert a typed discovery into dedicated storage.
pub(crate) fn discovery(
    conn: &mut Connection,
    agent_id: &str,
    title: &str,
    evidence: &[String],
    action: &str,
    signal: Option<&str>,
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let payload = DiscoveryPayload {
        title: title.to_owned(),
        evidence: evidence
            .iter()
            .map(|detail| DiscoveryEvidence {
                detail: detail.clone(),
            })
            .collect(),
        suggested_action: DiscoverySuggestedAction {
            cmd: action.to_owned(),
        },
        signal: Some(signal.unwrap_or("high").to_owned()),
    };
    payload.validate()?;

    let tx = conn.transaction()?;
    db::insert_discovery(&tx, now_ms, agent_id, &payload)?;
    db::touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;
    Ok(())
}

/// Insert an intent or declaration into dedicated typed storage.
pub(crate) fn surface(
    conn: &mut Connection,
    agent_id: &str,
    kind: &str,
    scope: &str,
    tags: &[String],
    surface: &[String],
    paths: &[String],
) -> anyhow::Result<()> {
    let now_ms = db::now_unix_ms()?;
    let payload = SurfacePayload {
        scope: scope.to_owned(),
        tags: tags.to_vec(),
        surface: surface.to_vec(),
        paths: paths.to_vec(),
    };
    payload.validate()?;

    let tx = conn.transaction()?;
    db::insert_surface_declaration(&tx, now_ms, agent_id, kind, &payload)?;
    db::touch_agent_progress_tx(&tx, agent_id, now_ms)?;
    tx.commit()?;
    Ok(())
}
