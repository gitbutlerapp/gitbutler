//! The tag vocabulary clients cache API results under.
//!
//! A tag names one kind of cached state. Three declarations, all in Rust,
//! describe everything that happens to it:
//!
//! * a read endpoint says what its result is made of: `#[but_api(provides = [Reviews])]`
//! * a mutation says what it makes stale: `#[but_api(invalidates = [Reviews])]`
//! * a watcher event says what it makes stale: [`crate::watcher::WatcherEventKind::invalidates`]
//!
//! Clients derive every cache refresh from those three, so which caches to
//! drop after a mutation or an event is never guessed on the frontend.
//! Mutations that only write to the repository declare nothing: the watcher
//! observes the repository, and the event carries the invalidation.

macro_rules! cache_tags {
    ($($(#[$doc:meta])+ $name:ident,)+) => {
        /// One kind of cached state a client may hold. See the module docs.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum CacheTag {
            $($(#[$doc])+ $name,)+
        }

        impl CacheTag {
            /// Every tag, for the SDK generator to enumerate.
            pub const ALL: &'static [CacheTag] = &[$(CacheTag::$name,)+];

            /// The tag's name as clients see it.
            pub fn name(self) -> &'static str {
                match self {
                    $(CacheTag::$name => stringify!($name),)+
                }
            }

            /// The tag a client-facing name spells, or `None` for a name no tag has.
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $(stringify!($name) => Some(CacheTag::$name),)+
                    _ => None,
                }
            }
        }
    };
}

cache_tags! {
    /// The branch listing and per-branch details and diffs.
    Branches,
    /// Commits on the workspace's target branch.
    TargetCommits,
    /// The workspace head: applied stacks and their segments.
    Workspace,
    /// A single commit's details.
    Commits,
    /// Diffs of individual changes, committed or not.
    Diffs,
    /// Uncommitted file changes with their assignments.
    WorktreeChanges,
    /// The linked-worktree listing with its archived state.
    Worktrees,
    /// Where uncommitted changes would absorb into existing commits.
    AbsorptionPlan,
    /// GitButler's own diff comments.
    Comments,
    /// When the workspace last fetched.
    FetchStatus,
    /// Forge reviews, listed or single.
    Reviews,
    /// Comments on a forge review.
    ReviewComments,
    /// A forge review's timeline.
    ReviewTimeline,
    /// A forge review's submissions.
    ReviewSubmissions,
    /// A forge review's diff-anchored comment threads.
    ReviewThreads,
    /// Whether a forge review can merge.
    MergeStatus,
    /// CI check runs.
    Checks,
    /// Reactions on a forge review.
    ReviewReactions,
    /// Reactions on a forge review comment.
    CommentReactions,
    /// The labels a repository offers.
    RepoLabels,
    /// Who could review.
    ReviewerCandidates,
    /// Which forge the repository talks to.
    ForgeInfo,
    /// Repo-level metadata from the forge: permissions, fork and visibility.
    RepoInfo,
    /// Who the current project is logged in as on its forge.
    ForgeLogin,
    /// The forge accounts known to the app.
    ForgeAccounts,
    /// The project's GitButler configuration.
    GbConfig,
    /// Whether the repository's signing configuration produces a signature.
    SigningSettings,
    /// The projects known to the app.
    Projects,
    /// The app's AI provider configuration.
    AiConfiguration,
    /// Which mode the repository is in (open workspace, edit mode, ...) and
    /// the edit session's own state.
    OperatingMode,
}

/// Record that a successful mutation made `tags` stale, for the watchers of
/// other processes: a `but` command reaches the desktop app this way. The
/// `#[but_api]` expansion calls this for every endpoint declaring
/// `invalidates` and taking a context; a primitive the CLI runs in place of
/// an endpoint passes the endpoint's `*_INVALIDATES` constant by hand.
pub fn signal_invalidation(project_data_dir: &std::path::Path, tags: &[&str]) {
    but_project_handle::write_invalidation_sentinel(project_data_dir, tags);
}

#[cfg(test)]
mod tests {
    use but_api_macros::but_api;
    use but_testsupport::{CommandExt, git_at_dir, open_repo};

    #[but_api(napi, invalidates = [Reviews, Checks])]
    pub fn probe(_ctx: &but_ctx::Context, succeed: bool) -> anyhow::Result<()> {
        if succeed {
            Ok(())
        } else {
            Err(anyhow::anyhow!("the mutation failed"))
        }
    }

    #[test]
    fn a_declared_mutation_records_its_tags_only_on_success() -> anyhow::Result<()> {
        let tmp = tempfile::tempdir()?;
        git_at_dir(tmp.path()).args(["init"]).run();
        let ctx = but_ctx::Context::from_repo_for_testing(open_repo(tmp.path())?)?;
        let sentinel = ctx.project_data_dir.join("INVALIDATE");

        probe(&ctx, false).unwrap_err();
        assert!(!sentinel.exists(), "a failed mutation made nothing stale");

        probe(&ctx, true)?;
        let content = std::fs::read_to_string(&sentinel)?;
        assert_eq!(
            but_project_handle::invalidation_by_others(&content, ""),
            ["Reviews", "Checks"]
        );
        assert!(
            but_project_handle::invalidation_by_others(
                &content,
                &but_project_handle::process_sentinel_token()
            )
            .is_empty(),
            "the writer signs the sentinel so its own watcher can skip it"
        );
        assert_eq!(PROBE_INVALIDATES, &["Reviews", "Checks"]);
        Ok(())
    }
}
