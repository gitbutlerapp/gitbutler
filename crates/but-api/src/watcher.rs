//! This module contains the serializable types for the watcher events.
//!
//! These are intended for export into type bindings for e.g. the but-sdk.

use crate::tags::CacheTag;
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
    /// External activity requiring the UI to re-read workspace state (stacks,
    /// branches, PR numbers) — remote-ref updates or external metadata writes.
    WorkspaceActivity(WatcherWorkspaceActivityPayload),
    /// Another process's mutation declared cache tags stale — see
    /// [`crate::tags::signal_invalidation`].
    ExternalInvalidation(WatcherExternalInvalidationPayload),
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WatcherPayload);

/// Which watcher event happened, without the payload it carried.
///
/// Each kind declares the tags it makes stale in [`WatcherEventKind::invalidates`];
/// the SDK exports that table so clients can derive invalidation from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherEventKind {
    /// See [`WatcherPayload::GitFetch`].
    GitFetch,
    /// See [`WatcherPayload::GitHead`].
    GitHead,
    /// See [`WatcherPayload::GitActivity`].
    GitActivity,
    /// See [`WatcherPayload::WorktreeChanges`].
    WorktreeChanges,
    /// See [`WatcherPayload::WorkspaceActivity`].
    WorkspaceActivity,
    /// See [`WatcherPayload::ExternalInvalidation`].
    ExternalInvalidation,
}

impl WatcherEventKind {
    /// Every kind, for the SDK generator to enumerate.
    pub const ALL: &'static [WatcherEventKind] = &[
        WatcherEventKind::GitFetch,
        WatcherEventKind::GitHead,
        WatcherEventKind::GitActivity,
        WatcherEventKind::WorktreeChanges,
        WatcherEventKind::WorkspaceActivity,
        WatcherEventKind::ExternalInvalidation,
    ];

    /// The event's name as clients see it, matching the payload's serde tag.
    pub fn name(self) -> &'static str {
        match self {
            WatcherEventKind::GitFetch => "gitFetch",
            WatcherEventKind::GitHead => "gitHead",
            WatcherEventKind::GitActivity => "gitActivity",
            WatcherEventKind::WorktreeChanges => "worktreeChanges",
            WatcherEventKind::WorkspaceActivity => "workspaceActivity",
            WatcherEventKind::ExternalInvalidation => "externalInvalidation",
        }
    }

    /// The tags this event makes stale.
    ///
    /// This is the event-side third of the tag declarations — see
    /// [`crate::tags`]. A fetch moves remote-tracking refs and refreshes the
    /// forge cache; activity means the repository changed; worktree changes
    /// mean the files did.
    pub fn invalidates(self) -> &'static [CacheTag] {
        use CacheTag as T;
        match self {
            WatcherEventKind::GitFetch => {
                &[T::Branches, T::TargetCommits, T::FetchStatus, T::Reviews]
            }
            WatcherEventKind::GitHead => &[T::OperatingMode],
            WatcherEventKind::GitActivity | WatcherEventKind::WorkspaceActivity => &[
                T::Branches,
                T::TargetCommits,
                T::Workspace,
                T::Commits,
                T::Diffs,
                T::WorktreeChanges,
                T::Worktrees,
                T::AbsorptionPlan,
                T::Comments,
            ],
            WatcherEventKind::WorktreeChanges => {
                &[T::Diffs, T::WorktreeChanges, T::AbsorptionPlan, T::Comments]
            }
            // The tags ride in the payload; the table cannot know them.
            WatcherEventKind::ExternalInvalidation => &[],
        }
    }
}

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
    /// The symbolic ref HEAD points at, or `null` when HEAD is detached.
    pub head: Option<String>,
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

/// Workspace activity that requires the UI to re-read branch/stack state.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WatcherWorkspaceActivityPayload;

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WatcherWorkspaceActivityPayload);

/// Cache tags another process declared stale, spelled as `cache-tags` exports them.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WatcherExternalInvalidationPayload {
    /// A client drops every cache providing one of these.
    pub tags: Vec<String>,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WatcherExternalInvalidationPayload);

/// Worktree files changes.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WatcherWorktreeChangesPayload {
    /// Worktree-relative paths that triggered the event, using the same lossy
    /// encoding as UI change paths. Empty for index changes or unknown paths.
    pub changed_paths: Vec<String>,
    /// The file changes in the repository.
    pub changes: WorktreeChanges,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WatcherWorktreeChangesPayload);
