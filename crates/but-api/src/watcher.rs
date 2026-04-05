//! This module contains the serializable types for the watcher events.
//!
//! These are intended for export into type bindings for e.g. the but-sdk.

use but_hunk_assignment::WorktreeChanges;
use gitbutler_operating_modes::OperatingMode;
use schemars::JsonSchema;
use serde::Serialize;

/// The type of payloads a watcher event can have
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum WatcherPayload {
    /// Git remote information was fetched.
    GitFetch(WatcherGitFetchPayload),
    /// Git HEAD and/or operating mode of GitButler changed.
    GitHead(WatcherGitHeadPayload),
    /// Git HEAD changed or there were changes to ref files.
    GitActivity(WatcherGitActivityPayload),
    /// There were changes in the files inside of the repository.
    WorktreeChanges(WatcherWorktreeChangesPayload),
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WatcherPayload);

/// Git fetch event
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WatcherGitFetchPayload;

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WatcherGitFetchPayload);

/// Git head (and operating mode) change event
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WatcherGitHeadPayload {
    /// The SHA of the repository's HEAD.
    pub head: String,
    /// The GitButler operating mode (edit mode, oper workspace, ...).
    pub operating_mode: OperatingMode,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WatcherGitHeadPayload);

/// Git files activity. Supplies the head sha
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WatcherGitActivityPayload {
    /// The SHA of the repository's HEAD.
    pub head_sha: String,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WatcherGitActivityPayload);

/// Worktree files changes.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WatcherWorktreeChangesPayload {
    /// The file changes in the repository.
    pub changes: WorktreeChanges,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WatcherWorktreeChangesPayload);
