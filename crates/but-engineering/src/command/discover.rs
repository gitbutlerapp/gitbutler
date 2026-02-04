//! Discover command implementation.
//!
//! Posts a discovery to the shared channel. Discoveries are findings,
//! gotchas, or insights that other agents should know about. They get
//! higher priority in hook summaries — always shown, with longer previews.

use crate::db::DbHandle;
use crate::types::{Message, MessageKind};

/// Post a discovery to the channel.
pub fn execute(db: &DbHandle, content: String, agent_id: String) -> anyhow::Result<Message> {
    super::post::execute_with_kind(db, content, agent_id, MessageKind::Discovery)
}
