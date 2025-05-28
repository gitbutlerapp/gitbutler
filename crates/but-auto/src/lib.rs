//! This crate implements various automations that GitButler can perform.

use gitbutler_command_context::CommandContext;
use serde::{Deserialize, Serialize};

#[allow(unused)]
pub fn handle_changes_simple(
    ctx: &mut CommandContext,
    request_ctx: &str,
) -> anyhow::Result<HandleChangesResponse> {
    Ok(HandleChangesResponse {})
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleChangesResponse {}
