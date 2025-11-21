use bstr::{BString, ByteSlice};
use serde::Serialize;

/// Utilities for diffing, with workspace integration.
pub mod diff;
/// RefInfo types for the UI.
pub mod ref_info;

pub use ref_info::inner::RefInfo;

/// This code is a fork of [`gitbutler_branch_actions::author`] to avoid depending on the `gitbutler_branch_actions` crate.
mod author;
pub use author::Author;
use ts_rs::TS;

use crate::{
    ref_info::{LocalCommit, LocalCommitRelation},
    ui,
};

/// Represents the state a commit could be in.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type", content = "subject")]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./workspace/index.ts"))]
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
    #[serde(with = "but_serde::object_id")]
    LocalAndRemote(#[ts(type = "string")] gix::ObjectId),
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
#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./workspace/index.ts"))]
pub struct Commit {
    /// The OID of the commit.
    #[serde(with = "but_serde::object_id")]
    #[ts(type = "string")]
    pub id: gix::ObjectId,
    /// The parent OIDs of the commit.
    #[serde(with = "but_serde::object_id_vec")]
    #[ts(type = "string[]")]
    pub parent_ids: Vec<gix::ObjectId>,
    /// The message of the commit.
    #[serde(with = "but_serde::bstring_lossy")]
    #[ts(type = "string")]
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
    /// Optional URL to the Gerrit review for this commit, if applicable.
    /// Only populated if Gerrit mode is enabled and the commit has an associated review.
    pub gerrit_review_url: Option<String>,
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
            gerrit_review_url: None,
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
#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./workspace/index.ts"))]
pub struct UpstreamCommit {
    /// The OID of the commit.
    #[serde(with = "but_serde::object_id")]
    #[ts(type = "string")]
    pub id: gix::ObjectId,
    /// The message of the commit.
    #[serde(with = "but_serde::bstring_lossy")]
    #[ts(type = "string")]
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
#[derive(Debug, Clone, PartialEq, Eq, Copy, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-ts", ts(export, export_to = "./workspace/index.ts"))]
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
    #[serde(with = "but_serde::bstring_lossy")]
    pub name: BString,
    /// The id of the linked worktree that has the reference of `name` checked out.
    /// Note that we don't list the main worktree here.
    #[serde(with = "but_serde::bstring_opt_lossy")]
    pub linked_worktree_id: Option<BString>,
    /// Upstream reference, e.g. `refs/remotes/origin/base-branch-improvements`
    #[serde(with = "but_serde::bstring_opt_lossy")]
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
    #[serde(with = "but_serde::object_id")]
    pub tip: gix::ObjectId,
    /// This is the base commit from the perspective of this branch.
    /// If the branch is part of a stack and is on top of another branch, this is the head of the branch below it.
    /// If this branch is at the bottom of the stack, this is the merge base of the stack.
    #[serde(with = "but_serde::object_id")]
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
    #[serde(with = "but_serde::bstring_lossy")]
    pub name: BString,
    /// Upstream reference, e.g. `refs/remotes/origin/base-branch-improvements`
    #[serde(with = "but_serde::bstring_opt_lossy")]
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
    #[serde(with = "but_serde::object_id")]
    pub tip: gix::ObjectId,
    /// This is the base commit from the perspective of this branch.
    /// If the branch is part of a stack and is on top of another branch, this is the head of the branch below it.
    /// If this branch is at the bottom of the stack, this is the merge base of the stack.
    #[serde(with = "but_serde::object_id")]
    pub base_commit: gix::ObjectId,
}

impl From<&crate::ref_info::Commit> for ui::UpstreamCommit {
    fn from(
        crate::ref_info::Commit {
            id,
            parent_ids: _,
            tree_id: _,
            message,
            author,
            // TODO: also pass refs for the frontend.
            refs: _,
            // TODO: also pass flags for the frontend.
            flags: _,
            // TODO: Represent this in the UI (maybe) and/or deal with divergence of the local and remote tracking branch.
            has_conflicts: _,
            change_id: _,
        }: &crate::ref_info::Commit,
    ) -> Self {
        ui::UpstreamCommit {
            id: *id,
            message: message.clone(),
            created_at: author.time.seconds as i128 * 1000,
            author: author
                .to_ref(&mut gix::date::parse::TimeBuf::default())
                .into(),
        }
    }
}

impl From<&LocalCommit> for ui::Commit {
    fn from(
        LocalCommit {
            inner:
                crate::ref_info::Commit {
                    id,
                    tree_id: _,
                    parent_ids,
                    message,
                    author,
                    // TODO: also pass refs
                    refs: _,
                    // TODO: also flags refs
                    flags: _,
                    has_conflicts,
                    change_id: _,
                },
            relation,
        }: &LocalCommit,
    ) -> Self {
        ui::Commit {
            id: *id,
            parent_ids: parent_ids.clone(),
            message: message.clone(),
            has_conflicts: *has_conflicts,
            state: (*relation).into(),
            created_at: author.time.seconds as i128 * 1000,
            author: author
                .to_ref(&mut gix::date::parse::TimeBuf::default())
                .into(),
            gerrit_review_url: None,
        }
    }
}

impl From<LocalCommitRelation> for ui::CommitState {
    fn from(value: LocalCommitRelation) -> Self {
        use crate::ui::CommitState as E;
        match value {
            LocalCommitRelation::LocalOnly => E::LocalOnly,
            LocalCommitRelation::LocalAndRemote(id) => E::LocalAndRemote(id),
            LocalCommitRelation::Integrated(_) => E::Integrated,
        }
    }
}
