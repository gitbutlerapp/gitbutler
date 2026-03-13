//! The API layer is what can be used to create GitButler applications.
//!
//! ### Coordinating Filesystem Access
//!
//! For them to behave correctly in multi-threaded scenarios, be sure to use an *exclusive or shared* lock
//! on this level.
//! Lower-level crates like `but-workspace` won't use filesystem-based locking beyond what Git offers natively.
#![cfg_attr(not(feature = "napi"), forbid(unsafe_code))]
#![cfg_attr(feature = "napi", deny(unsafe_code))]
#![deny(missing_docs)]

#[cfg(feature = "legacy")]
pub mod legacy;

/// Functions for GitHub authentication.
pub mod github;

/// Functions for GitLab authentication.
pub mod gitlab;

/// Functions that take a branch as input.
pub mod branch;

/// Functions that operate commits
pub mod commit;

/// Functions that show what changed in various Git entities, like trees, commits and the worktree.
pub mod diff;

/// Types meant to be serialised to JSON, without degenerating information despite the need to be UTF-8 encodable.
/// EXPERIMENTAL
pub mod json;

/// Functions releated to platform detection and information.
pub mod platform;

pub mod panic_capture;

/// The types for watcher events
#[cfg(feature = "export-schema")]
pub mod watcher {
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
}
