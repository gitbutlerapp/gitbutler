//! This crate implements various automations that GitButler can perform.

use gitbutler_command_context::CommandContext;
use serde::{Deserialize, Serialize};

/// This is a GitButler automation which allows easy handling of uncommitted changes in a repository.
/// At a high level, it will:
///   - Checkout GitButler's workspace branch if not already checked out
///   - Create a new branch if necessary (using a generic canned branch name)
///   - Create a new commit with all uncommitted changes found in the worktree (the request context is used as the commit message)
///
/// Avery time this automation is ran, GitButler will aslo:
///   - Create an oplog snaposhot entry _before_ the automation is executed
///   - Create an oplog snapshot entry _after_ the automation is executed
///   - Create a separate persisted entry recording the request context and IDs for the two oplog snapshots
#[allow(unused)]
pub fn handle_changes_simple(
    ctx: &mut CommandContext,
    request_ctx: &str,
) -> anyhow::Result<HandleChangesResponse> {
    Ok(HandleChangesResponse {})
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandleChangesResponse {}
