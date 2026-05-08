//! This module contains the serializable types for the watcher events.
//!
//! These are intended for export into type bindings for e.g. the but-sdk.

use but_hunk_assignment::WorktreeChanges;
use gitbutler_operating_modes::OperatingMode;
use serde::Serialize;

/// The type of payloads a watcher event can have
#[but_api_macros::but_transport(tag = "type", content = "subject")]
#[derive(Clone)]
pub enum WatcherPayload {
    /// Git remote information was fetched.
    GitFetch(WatcherGitFetchPayload),
    /// Git HEAD and/or operating mode of GitButler changed.
    GitHead(WatcherGitHeadPayload),
    /// Git HEAD changed or there were changes to ref files.
    GitActivity(WatcherGitActivityPayload),
    /// Remote tracking refs changed (e.g. after a push).
    GitRemoteActivity(WatcherGitRemoteActivityPayload),
    /// There were changes in the files inside of the repository.
    WorktreeChanges(WatcherWorktreeChangesPayload),
}

/// Git fetch event
#[but_api_macros::but_transport]
#[derive(Clone)]
pub struct WatcherGitFetchPayload;

/// Git head (and operating mode) change event
#[but_api_macros::but_transport]
#[derive(Clone)]
pub struct WatcherGitHeadPayload {
    /// The SHA of the repository's HEAD.
    pub head: String,
    /// The GitButler operating mode (edit mode, oper workspace, ...).
    pub operating_mode: OperatingMode,
}

/// Git files activity. Supplies the head sha
#[but_api_macros::but_transport]
#[derive(Clone)]
pub struct WatcherGitActivityPayload {
    /// The SHA of the repository's HEAD.
    pub head_sha: String,
}

/// Remote tracking refs changed (e.g. after a push or external git operation).
#[but_api_macros::but_transport]
#[derive(Clone)]
pub struct WatcherGitRemoteActivityPayload;

/// Worktree files changes.
#[but_api_macros::but_transport]
#[derive(Clone)]
pub struct WatcherWorktreeChangesPayload {
    /// The file changes in the repository.
    pub changes: WorktreeChanges,
}
