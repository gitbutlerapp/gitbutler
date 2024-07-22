use bstr::BString;
use gitbutler_branch::BranchId;
use serde::Serialize;

use crate::{author::Author, VirtualBranch};

/// Represents a branch that exists for the repository
/// This also combines the concept of a remote, local and virtual branch in order to provide a unified interface for the UI
/// Branch entry is not meant to contain all of the data a branch can have (e.g. full commit history, all files and diffs, etc.).
/// It is intended a summary that can be quickly retrieved and displayed in the UI.
/// For more detailed information, each branch can be queried individually for it's `BranchData`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// The name of the branch (e.g. `main`, `feature/branch`), excluding the remote name
    pub name: String,
    /// This is a list of remote that this branch can be found on (e.g. `origin`, `upstream` etc.).
    /// If this branch is a local branch, this list will be empty.
    pub remotes: Vec<String>,
    /// The branch may or may not have a virtual branch associated with it
    pub virtual_branch: Option<VirtualBranchReference>,
    /// The number of lines added within the branch
    /// Since the virtual branch, local branch and the remote one can have different number of lines removed,
    /// the value from the virtual branch (if present) takes the highest precedence,
    /// followed by the local branch and then the remote branches (taking the max if there are multiple)
    pub lines_added: u32,
    /// The number of lines removed within the branch
    /// Since the virtual branch, local branch and the remote one can have different number of lines removed,
    /// the value from the virtual branch (if present) takes the highest precedence,
    /// followed by the local branch and then the remote branches (taking the max if there are multiple)
    pub lines_removed: u32,
    /// The number of commits associated with a branch
    /// Since the virtual branch, local branch and the remote one can have different number of commits,
    /// the value from the virtual branch (if present) takes the highest precedence,
    /// followed by the local branch and then the remote branches (taking the max if there are multiple)
    pub number_of_commits: u32,
    /// Timestamp in milliseconds since the branch was last updated.
    /// This includes any commits, uncommited changes or even updates to the branch metadata (e.g. renaming).
    pub updated_at: u128,
    /// A list of authors that have contributes commits to this branch.
    /// In the case of multiple remote tracking branches, it takes the full list of unique authors.
    pub authors: Vec<Author>,
    /// Determines if the branch is considered one created by the user
    pub own_branch: bool,
}

/// Represents a reference to an associated virtual branch
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchReference {
    /// A non-normalized name of the branch, set by the user
    pub given_name: String,
    /// Virtual Branch UUID identifier
    pub id: BranchId,
    /// Determines if the virtual branch is applied in the workspace
    pub in_workspace: bool,
}

/// Represents a fat struct with all the data associated with a branch
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchData {
    /// The branch that this data is associated with
    pub branch: Branch,
    /// Sometimes the app creates additional new branches when unapplying a virtual branch, usually suffixed with a counter.
    /// This is either done by the user to avoid overriding when unapplying or by the app when dealing with conflicts.
    /// TODO: In general we should make the app not need these and instead have only one associated local branch at any given time.
    pub local_branches: Vec<LocalBranchEntry>,
    /// A branch may have multiple remote tracking branches associated with it, from different remotes.
    /// The name of the branch is the same, but the remote could be different as well as the head commit.
    pub remote_branches: Vec<RemoteBranchEntry>,
    /// The virtual branch entry associated with the branch
    pub virtual_branch: Option<VirtualBranch>,
}
/// Represents a local branch
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchEntry {
    /// The name of the branch (e.g. `main`, `feature/branch`)
    pub name: String,
    /// The head commit of the branch
    #[serde(with = "gitbutler_serde::serde::oid")]
    head: git2::Oid,
    /// The commit base of the branch
    #[serde(with = "gitbutler_serde::serde::oid")]
    base: git2::Oid,
    /// The list of commits associated with the branch
    pub commits: Vec<CommitEntry>,
}

/// Represents a local branch
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalBranchEntry {
    #[serde(flatten)]
    pub base: BranchEntry,
}

/// Represents a branch that is from a remote
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchEntry {
    #[serde(flatten)]
    pub base: BranchEntry,
    /// The name of the remote (e.g. `origin`, `upstream` etc.)
    pub remote_name: String,
}

/// Commits associated with a branch
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitEntry {
    /// The commit sha that it can be referenced by
    #[serde(with = "gitbutler_serde::serde::oid")]
    pub id: git2::Oid,
    /// If the commit is referencing a specific change, this is its change id
    pub change_id: Option<String>,
    /// The commit message
    #[serde(serialize_with = "gitbutler_serde::serde::as_string_lossy")]
    pub description: BString,
    /// The timestamp of the commit in milliseconds
    pub created_at: u128,
    /// The author of the commit
    pub authors: Vec<Author>,
    /// The parent commits of the commit
    #[serde(with = "gitbutler_serde::serde::oid_vec")]
    pub parent_ids: Vec<git2::Oid>,
}
