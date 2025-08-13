use anyhow::Context;
use bstr::{BStr, BString, ByteSlice};
use serde::Serialize;

/// Utilities for diffing, with workspace integration.
pub mod diff;

/// This code is a fork of [`gitbutler_branch_actions::author`] to avoid depending on the `gitbutler_branch_actions` crate.
mod author {
    use bstr::ByteSlice;
    use serde::Serialize;

    /// Represents the author of a commit.
    #[derive(Serialize, Hash, Clone, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub struct Author {
        /// The name from the git commit signature
        pub name: String,
        /// The email from the git commit signature
        pub email: String,
        /// A URL to a gravatar image for the email from the commit signature
        pub gravatar_url: url::Url,
    }

    impl std::fmt::Debug for Author {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{} <{}>", self.name, self.email)
        }
    }

    impl From<git2::Signature<'_>> for Author {
        fn from(value: git2::Signature<'_>) -> Self {
            let name = value.name().unwrap_or_default().to_string();
            let email = value.email().unwrap_or_default().to_string();
            let gravatar_url = gravatar_url_from_email(email.as_str());
            Author {
                name,
                email,
                gravatar_url,
            }
        }
    }

    impl From<gix::actor::SignatureRef<'_>> for Author {
        fn from(value: gix::actor::SignatureRef<'_>) -> Self {
            let gravatar_url = gravatar_url_from_email(&value.email.to_str_lossy());

            Author {
                name: value.name.to_string(),
                email: value.email.to_string(),
                gravatar_url,
            }
        }
    }

    pub fn gravatar_url_from_email(email: &str) -> url::Url {
        let gravatar_url = format!(
            "https://www.gravatar.com/avatar/{:x}?s=100&r=g&d=retro",
            md5::compute(email.to_lowercase())
        );
        url::Url::parse(gravatar_url.as_str()).expect("an MD5 as part of the URl is always valid")
    }
}
pub use author::Author;
use gitbutler_stack::{Stack, StackId};

/// The information about the branch inside a stack
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackHeadInfo {
    /// The name of the branch.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub name: BString,
    /// The tip of the branch.
    #[serde(with = "gitbutler_serde::object_id")]
    pub tip: gix::ObjectId,
    /// If `true`, then this head is checked directly so `HEAD` points to it, and this is only ever `true` for a single head.
    /// This is `false` if the worktree is checked out.
    pub is_checked_out: bool,
}

/// Represents a lightweight version of a [`Stack`] for listing.
/// NOTE: this is a UI type mostly because it's still modeled after the legacy stack with StackId, something that doesn't exist anymore.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackEntry {
    /// The ID of the stack.
    pub id: Option<StackId>,
    /// The list of the branch information that are part of the stack.
    /// The list is never empty.
    /// The first entry in the list is always the most recent branch on top the stack.
    pub heads: Vec<StackHeadInfo>,
    /// The tip of the top-most branch, i.e., the most recent commit that would become the parent of new commits of the topmost stack branch.
    #[serde(with = "gitbutler_serde::object_id")]
    pub tip: gix::ObjectId,
    /// The zero-based index for sorting stacks.
    pub order: Option<usize>,
    /// If `true`, then any head in this stack is checked directly so `HEAD` points to it, and this is only ever `true` for a single stack.
    pub is_checked_out: bool,
}

/// **Temporary type to help transitioning to the optional version of stack-entry** and ultimately, to [`crate::RefInfo`].
/// WARNING: for use by parts in the code that can rely on having a non-optional `stack_id`. The goal is to have none of these.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackEntryNoOpt {
    /// The ID of the stack.
    pub id: StackId,
    /// The list of the branch information that are part of the stack.
    /// The list is never empty.
    /// The first entry in the list is always the most recent branch on top the stack.
    pub heads: Vec<StackHeadInfo>,
    /// The tip of the top-most branch, i.e., the most recent commit that would become the parent of new commits of the topmost stack branch.
    #[serde(with = "gitbutler_serde::object_id")]
    pub tip: gix::ObjectId,
    /// The zero-based index for sorting stacks.
    pub order: Option<usize>,
    /// If `true`, then any head in this stack is checked directly so `HEAD` points to it, and this is only ever `true` for a single stack.
    pub is_checked_out: bool,
}

impl StackEntry {
    /// The name of the stack, which is the name of the top-most branch.
    pub fn name(&self) -> Option<&BStr> {
        self.heads.first().map(|head| head.name.as_ref())
    }
}

impl StackEntryNoOpt {
    /// The name of the stack, which is the name of the top-most branch.
    pub fn name(&self) -> Option<&BStr> {
        self.heads.first().map(|head| head.name.as_ref())
    }
}

impl TryFrom<StackEntry> for StackEntryNoOpt {
    type Error = anyhow::Error;

    fn try_from(
        StackEntry {
            id,
            heads,
            tip,
            order,
            is_checked_out,
        }: StackEntry,
    ) -> Result<Self, Self::Error> {
        let id = id.context("BUG(opt-stack-id)")?;
        Ok(StackEntryNoOpt {
            id,
            heads,
            tip,
            order,
            is_checked_out,
        })
    }
}

impl From<StackEntryNoOpt> for StackEntry {
    fn from(
        StackEntryNoOpt {
            id,
            heads,
            tip,
            order,
            is_checked_out,
        }: StackEntryNoOpt,
    ) -> Self {
        StackEntry {
            id: Some(id),
            heads,
            tip,
            order,
            is_checked_out,
        }
    }
}

impl StackEntry {
    pub(crate) fn try_new(repo: &gix::Repository, stack: &Stack) -> anyhow::Result<Self> {
        Ok(StackEntry {
            id: Some(stack.id),
            heads: crate::stack_heads_info(stack, repo)?,
            tip: stack.head_oid(repo)?,
            order: Some(stack.order),
            is_checked_out: false,
        })
    }
}

impl StackEntryNoOpt {
    pub(crate) fn try_new(repo: &gix::Repository, stack: &Stack) -> anyhow::Result<Self> {
        Ok(StackEntryNoOpt {
            id: stack.id,
            heads: crate::stack_heads_info(stack, repo)?,
            tip: stack.head_oid(repo)?,
            order: Some(stack.order),
            is_checked_out: false,
        })
    }
}

/// Represents the state a commit could be in.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "subject")]
pub enum CommitState {
    /// The commit is only local
    LocalOnly,
    /// The commit is also present at the remote tracking branch.
    /// This is the commit state if:
    ///  - The commit has been pushed to the remote
    ///  - The commit has been copied from a remote commit (when applying a remote branch)
    ///
    /// This variant carries the remote commit id.
    /// The `remote_commit_id` may be the same as the `id` or it may be different if the local commit has been rebased or updated in another way.
    #[serde(with = "gitbutler_serde::object_id")]
    LocalAndRemote(gix::ObjectId),
    /// The commit is considered integrated.
    /// This should happen when this commit or the contents of this commit is already part of the base.
    Integrated,
}

impl CommitState {
    fn display(&self, id: gix::ObjectId) -> &'static str {
        match self {
            CommitState::LocalOnly => "local",
            CommitState::LocalAndRemote(remote_id) => {
                if *remote_id == id {
                    "local/remote(identity)"
                } else {
                    "local/remote(similarity)"
                }
            }
            CommitState::Integrated => "integrated",
        }
    }
}

/// Commit that is a part of a [`StackBranch`](gitbutler_stack::StackBranch) and, as such, containing state derived in relation to the specific branch.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    /// The OID of the commit.
    #[serde(with = "gitbutler_serde::object_id")]
    pub id: gix::ObjectId,
    /// The parent OIDs of the commit.
    #[serde(with = "gitbutler_serde::object_id_vec")]
    pub parent_ids: Vec<gix::ObjectId>,
    /// The message of the commit.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub message: BString,
    /// Whether the commit is in a conflicted state.
    /// The Conflicted state of a commit is a GitButler concept.
    /// GitButler will perform rebasing/reordering etc without interruptions and flag commits as conflicted if needed.
    /// Conflicts are resolved via the Edit Mode mechanism.
    pub has_conflicts: bool,
    /// Represents whether the commit is considered integrated, local only,
    /// or local and remote with respect to the branch it belongs to.
    /// Note that remote only commits in the context of a branch are expressed with the [`UpstreamCommit`] struct instead of this.
    pub state: CommitState,
    /// Commit creation time in Epoch milliseconds.
    pub created_at: i128,
    /// The author of the commit.
    pub author: Author,
}

impl TryFrom<gix::Commit<'_>> for Commit {
    type Error = anyhow::Error;
    fn try_from(commit: gix::Commit<'_>) -> Result<Self, Self::Error> {
        Ok(Commit {
            id: commit.id,
            parent_ids: commit.parent_ids().map(|id| id.detach()).collect(),
            message: commit.message_raw_sloppy().into(),
            has_conflicts: false,
            state: CommitState::LocalAndRemote(commit.id),
            created_at: i128::from(commit.time()?.seconds) * 1000,
            author: commit.author()?.into(),
        })
    }
}

impl std::fmt::Debug for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Commit({short_hex}, {message:?}, {state})",
            short_hex = self.id.to_hex_with_len(7),
            message = self.message.trim().as_bstr(),
            state = self.state.display(self.id)
        )
    }
}

/// Commit that is only at the remote.
/// Unlike the `Commit` struct, there is no knowledge of GitButler concepts like conflicted state etc.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamCommit {
    /// The OID of the commit.
    #[serde(with = "gitbutler_serde::object_id")]
    pub id: gix::ObjectId,
    /// The message of the commit.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub message: BString,
    /// Commit creation time in Epoch milliseconds.
    pub created_at: i128,
    /// The author of the commit.
    pub author: Author,
}

impl std::fmt::Debug for UpstreamCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "UpstreamCommit({short_hex}, {message:?})",
            short_hex = self.id.to_hex_with_len(7),
            message = self.message.trim().as_bstr()
        )
    }
}

/// Represents the pushable status for the current stack.
#[derive(Debug, Clone, PartialEq, Eq, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PushStatus {
    /// Can push, but there are no changes to be pushed
    NothingToPush,
    /// Can push. This is the case when there are local changes that can be pushed to the remote.
    UnpushedCommits,
    /// Can push, but requires a force push to the remote because commits were rewritten.
    UnpushedCommitsRequiringForce,
    /// Completely unpushed - there is no remote tracking branch so Git never interacted with the remote.
    CompletelyUnpushed,
    /// Fully integrated, no changes to push.
    Integrated,
}

/// Information about the current state of a branch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDetails {
    /// The name of the branch.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub name: BString,
    /// Upstream reference, e.g. `refs/remotes/origin/base-branch-improvements`
    #[serde(with = "gitbutler_serde::bstring_opt_lossy")]
    pub remote_tracking_branch: Option<BString>,
    /// Description of the branch.
    /// Can include arbitrary utf8 data, eg. markdown etc.
    pub description: Option<String>,
    /// The pull(merge) request associated with the branch, or None if no such entity has not been created.
    pub pr_number: Option<usize>,
    /// A unique identifier for the GitButler review associated with the branch, if any.
    pub review_id: Option<String>,
    /// This is the last commit in the branch, aka the tip of the branch.
    /// If this is the only branch in the stack or the top-most branch, this is the tip of the stack.
    #[serde(with = "gitbutler_serde::object_id")]
    pub tip: gix::ObjectId,
    /// This is the base commit from the perspective of this branch.
    /// If the branch is part of a stack and is on top of another branch, this is the head of the branch below it.
    /// If this branch is at the bottom of the stack, this is the merge base of the stack.
    #[serde(with = "gitbutler_serde::object_id")]
    pub base_commit: gix::ObjectId,
    /// The pushable status for the branch.
    pub push_status: PushStatus,
    /// Last time, the branch was updated in Epoch milliseconds.
    pub last_updated_at: Option<i128>,
    /// All authors of the commits in the branch.
    pub authors: Vec<Author>,
    /// Whether the branch is conflicted.
    pub is_conflicted: bool,
    /// The commits contained in the branch, excluding the upstream commits.
    pub commits: Vec<Commit>,
    /// The commits that are only at the remote.
    pub upstream_commits: Vec<UpstreamCommit>,
    /// Whether it's representing a remote head
    pub is_remote_head: bool,
}

/// Information about the current state of a stack
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackDetails {
    /// This is the name of the top-most branch, provided by the API for convenience
    pub derived_name: String,
    /// The pushable status for the stack
    pub push_status: PushStatus,
    /// The details about the contained branches
    pub branch_details: Vec<BranchDetails>,
    /// Whether the stack is conflicted.
    pub is_conflicted: bool,
}

/// Represents a branch in a [`Stack`]. It contains commits derived from the local pseudo branch and it's respective remote
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// The name of the branch.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub name: BString,
    /// Upstream reference, e.g. `refs/remotes/origin/base-branch-improvements`
    #[serde(with = "gitbutler_serde::bstring_opt_lossy")]
    pub remote_tracking_branch: Option<BString>,
    /// Description of the branch.
    /// Can include arbitrary utf8 data, eg. markdown etc.
    pub description: Option<String>,
    /// The pull(merge) request associated with the branch, or None if no such entity has not been created.
    pub pr_number: Option<usize>,
    /// A unique identifier for the GitButler review associated with the branch, if any.
    pub review_id: Option<String>,
    /// Indicates that the branch was previously part of a stack but it has since been integrated.
    /// In other words, the merge base of the stack is now above this branch.
    /// This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
    /// An archived branch will not have any commits associated with it.
    pub archived: bool,
    /// This is the last commit in the branch, aka the tip of the branch.
    /// If this is the only branch in the stack or the top-most branch, this is the tip of the stack.
    #[serde(with = "gitbutler_serde::object_id")]
    pub tip: gix::ObjectId,
    /// This is the base commit from the perspective of this branch.
    /// If the branch is part of a stack and is on top of another branch, this is the head of the branch below it.
    /// If this branch is at the bottom of the stack, this is the merge base of the stack.
    #[serde(with = "gitbutler_serde::object_id")]
    pub base_commit: gix::ObjectId,
}
