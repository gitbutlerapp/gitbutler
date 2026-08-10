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
}
