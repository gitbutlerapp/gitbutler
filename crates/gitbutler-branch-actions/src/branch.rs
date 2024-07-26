use std::cmp::max;
use std::collections::HashMap;
use std::collections::HashSet;
use std::vec;

use anyhow::Context;
use anyhow::Result;
use bstr::{BString, ByteSlice};
use gitbutler_branch::Branch as GitButlerBranch;
use gitbutler_branch::BranchId;
use gitbutler_branch::ReferenceExt;
use gitbutler_branch::Target;
use gitbutler_branch::VirtualBranchesHandle;
use gitbutler_command_context::ProjectRepository;

use gitbutler_reference::normalize_branch_name;
use serde::Serialize;

use crate::{VirtualBranch, VirtualBranchesExt};

/// Returns a list of branches associated with this project.
// TODO: Implement pagination for this thing
pub fn list_branches(ctx: &ProjectRepository) -> Result<Vec<BranchListing>> {
    let vb_handle = ctx.project().virtual_branches();
    let branches = ctx.repo().branches(None)?;

    // virtual branches from the application state
    let virtual_branches = ctx
        .project()
        .virtual_branches()
        .list_all_branches()?
        .into_iter();

    combine_branches(branches, virtual_branches, ctx.repo(), &vb_handle)
}

fn combine_branches(
    branches: git2::Branches,
    virtual_branches: impl Iterator<Item = GitButlerBranch>,
    repo: &git2::Repository,
    vb_handle: &VirtualBranchesHandle,
) -> Result<Vec<BranchListing>> {
    let mut group_branches: Vec<GroupBranch> = vec![];
    for branch in virtual_branches {
        group_branches.push(GroupBranch::Virtual(branch));
    }
    for result in branches {
        match result {
            Ok((branch, branch_type)) => match branch_type {
                git2::BranchType::Local => {
                    group_branches.push(GroupBranch::Local(branch));
                }
                git2::BranchType::Remote => {
                    group_branches.push(GroupBranch::Remote(branch));
                }
            },
            Err(_) => {
                continue;
            }
        }
    }
    let remotes = repo.remotes()?;
    let target_branch = vb_handle.get_default_target().ok();
    // Group branches by identity
    let mut groups: HashMap<Option<String>, Vec<&GroupBranch>> = HashMap::new();
    for branch in group_branches.iter() {
        let identity = branch.identity(&remotes);
        // Skip branches that should not be listed, e.g. the target 'main' or the gitbutler technical branches like 'gitbutler/integration'
        if !should_list_git_branch(&identity, &target_branch) {
            continue;
        }
        if let Some(group) = groups.get_mut(&identity) {
            group.push(branch);
        } else {
            groups.insert(identity, vec![branch]);
        }
    }
    let config = repo.config()?;
    let local_author = Author {
        name: config.get_string("user.name").ok(),
        email: config.get_string("user.email").ok(),
    };

    // Convert to Branch entries for the API response, filtering out any errors
    let branches: Vec<BranchListing> = groups
        .iter()
        .filter_map(|(identity, group_branches)| {
            let branch_entry = branch_group_to_branch(
                identity.clone(),
                group_branches.clone(),
                repo,
                &local_author,
            );
            if branch_entry.is_err() {
                tracing::warn!(
                    "Failed to process branch group {:?} to branch entry: {:?}",
                    identity,
                    branch_entry
                );
            }
            branch_entry.ok()
        })
        .collect();
    Ok(branches)
}

/// Converts a group of branches with the same identity into a single branch entry
fn branch_group_to_branch(
    identity: Option<String>,
    group_branches: Vec<&GroupBranch>,
    repo: &git2::Repository,
    local_author: &Author,
) -> Result<BranchListing> {
    let virtual_branch = group_branches
        .iter()
        .filter_map(|branch| match branch {
            GroupBranch::Virtual(vb) => Some(vb),
            _ => None,
        })
        .next();
    let remote_branches: Vec<&git2::Branch> = group_branches
        .iter()
        .filter_map(|branch| match branch {
            GroupBranch::Remote(gb) => Some(gb),
            _ => None,
        })
        .collect();
    let local_branches: Vec<&git2::Branch> = group_branches
        .iter()
        .filter_map(|branch| match branch {
            GroupBranch::Local(gb) => Some(gb),
            _ => None,
        })
        .collect();

    // Virtual branch associated with this branch
    let virtual_branch_reference = virtual_branch.map(|branch| VirtualBranchReference {
        given_name: branch.name.clone(),
        id: branch.id,
        in_workspace: branch.in_workspace,
    });

    let mut remotes: Vec<BString> = Vec::new();
    for branch in remote_branches.iter() {
        if let Some(name) = branch.get().name() {
            if let Ok(remote_name) = repo.branch_remote_name(name) {
                remotes.push(remote_name.as_bstr().into());
            }
        }
    }

    // The head commit for which we calculate statistics.
    // If there is a virtual branch let's get it's head. Alternatively, pick the first local branch and use it's head.
    // If there are no local branches, pick the first remote branch.
    let head = if let Some(vbranch) = virtual_branch {
        Some(vbranch.head)
    } else if let Some(branch) = local_branches.first().cloned() {
        branch.get().peel_to_commit().ok().map(|c| c.id())
    } else if let Some(branch) = remote_branches.first().cloned() {
        branch.get().peel_to_commit().ok().map(|c| c.id())
    } else {
        None
    }
    .context("Could not get any valid reference in order to build branch stats")?;

    // If this was a virtual branch and there was never any remote set, use the virtual branch name as the identity
    let identity = identity.unwrap_or(
        virtual_branch
            .map(|vb| normalize_branch_name(&vb.name))
            .unwrap_or_default(),
    );
    let repo_head = repo.head()?.peel_to_commit()?;
    // If no merge base can be found, return with zero stats
    let branch = if let Ok(base) = repo.merge_base(repo_head.id(), head) {
        let base_tree = repo.find_commit(base)?.tree()?;
        let head_tree = repo.find_commit(head)?.tree()?;
        let diff_stats = repo
            .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), None)?
            .stats()?;

        let mut revwalk = repo.revwalk()?;
        revwalk.push(head)?;
        revwalk.hide(base)?;
        let mut commits = Vec::new();
        let mut authors = HashSet::new();
        let mut last_commit_time_ms = i64::MIN;
        for oid in revwalk {
            let commit = repo.find_commit(oid?)?;
            last_commit_time_ms = max(last_commit_time_ms, commit.time().seconds() * 1000);
            authors.insert(commit.author().into());
            commits.push(commit);
        }

        let own_branch = commits
            .last()
            .map_or(false, |commit| local_author == &commit.author().into());

        let last_modified_ms = max(
            last_commit_time_ms as u128,
            virtual_branch.map_or(0, |x| x.updated_timestamp_ms),
        );

        BranchListing {
            name: identity,
            remotes,
            virtual_branch: virtual_branch_reference,
            lines_added: diff_stats.insertions(),
            lines_removed: diff_stats.deletions(),
            number_of_files: diff_stats.files_changed(),
            number_of_commits: commits.len(),
            updated_at: last_modified_ms,
            authors: authors.into_iter().collect(),
            own_branch,
        }
    } else {
        let last_modified_ms = (repo.find_commit(head)?.time().seconds() * 1000) as u128;
        BranchListing {
            name: identity,
            remotes,
            virtual_branch: virtual_branch_reference,
            lines_added: 0,
            lines_removed: 0,
            number_of_files: 0,
            number_of_commits: 0,
            updated_at: last_modified_ms,
            authors: Vec::new(),
            own_branch: false,
        }
    };
    Ok(branch)
}

/// A sum type of a branch that can be a plain git branch or a virtual branch
#[allow(clippy::large_enum_variant)]
enum GroupBranch<'a> {
    Local(git2::Branch<'a>),
    Remote(git2::Branch<'a>),
    Virtual(GitButlerBranch),
}

impl GroupBranch<'_> {
    /// A name identifier for the branch. When multiple branches (e.g. virtual, local, reomte) have the same identity,
    /// they are grouped together under the same `Branch` entry.
    fn identity(&self, remotes: &git2::string_array::StringArray) -> Option<String> {
        match self {
            GroupBranch::Local(branch) => branch.get().given_name(remotes).ok(),
            GroupBranch::Remote(branch) => branch.get().given_name(remotes).ok(),
            // The identity of a Virtual branch is derived from the source refname, upstream or the branch given name, in that order
            GroupBranch::Virtual(branch) => {
                let name_from_source = branch.source_refname.as_ref().and_then(|n| n.branch());
                let name_from_upstream = branch.upstream.as_ref().map(|n| n.branch());
                let rich_name = branch.name.clone();
                let rich_name = &normalize_branch_name(&rich_name);
                let identity = name_from_source.unwrap_or(name_from_upstream.unwrap_or(rich_name));
                Some(identity.to_string())
            }
        }
    }
}

/// Determines if a branch should be listed in the UI.
/// This excludes the target branch as well as gitbutler specific branches.
fn should_list_git_branch(identity: &Option<String>, target: &Option<Target>) -> bool {
    // Exclude the target branch
    if let Some(target) = target {
        if identity == &Some(target.branch.branch().to_owned()) {
            return false;
        }
    }
    // Exclude gitbutler technical branches (not useful for the user)
    if identity == &Some("gitbutler/integration".to_string())
        || identity == &Some("gitbutler/target".to_string())
        || identity == &Some("gitbutler/oplog".to_string())
        || identity == &Some("HEAD".to_string())
    {
        return false;
    }
    true
}

/// Represents a branch that exists for the repository
/// This also combines the concept of a remote, local and virtual branch in order to provide a unified interface for the UI
/// Branch entry is not meant to contain all of the data a branch can have (e.g. full commit history, all files and diffs, etc.).
/// It is intended a summary that can be quickly retrieved and displayed in the UI.
/// For more detailed information, each branch can be queried individually for it's `BranchData`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchListing {
    /// The name of the branch (e.g. `main`, `feature/branch`), excluding the remote name
    pub name: String,
    /// This is a list of remote that this branch can be found on (e.g. `origin`, `upstream` etc.).
    /// If this branch is a local branch, this list will be empty.
    #[serde(serialize_with = "gitbutler_serde::serde::as_string_lossy_vec")]
    pub remotes: Vec<BString>,
    /// The branch may or may not have a virtual branch associated with it
    pub virtual_branch: Option<VirtualBranchReference>,
    /// The number of lines added within the branch
    /// Since the virtual branch, local branch and the remote one can have different number of lines removed,
    /// the value from the virtual branch (if present) takes the highest precedence,
    /// followed by the local branch and then the remote branches (taking the max if there are multiple).
    /// If this branch has a virutal branch, lines_added does NOT include the uncommitted lines.
    pub lines_added: usize,
    /// The number of lines removed within the branch
    /// Since the virtual branch, local branch and the remote one can have different number of lines removed,
    /// the value from the virtual branch (if present) takes the highest precedence,
    /// followed by the local branch and then the remote branches (taking the max if there are multiple)
    /// If this branch has a virutal branch, lines_removed does NOT include the uncommitted lines.
    pub lines_removed: usize,
    /// The number of files that were modified within the branch
    /// Since the virtual branch, local branch and the remote one can have different number files modified,
    /// the value from the virtual branch (if present) takes the highest precedence,
    /// followed by the local branch and then the remote branches (taking the max if there are multiple)
    pub number_of_files: usize,
    /// The number of commits associated with a branch
    /// Since the virtual branch, local branch and the remote one can have different number of commits,
    /// the value from the virtual branch (if present) takes the highest precedence,
    /// followed by the local branch and then the remote branches (taking the max if there are multiple)
    pub number_of_commits: usize,
    /// Timestamp in milliseconds since the branch was last updated.
    /// This includes any commits, uncommited changes or even updates to the branch metadata (e.g. renaming).
    pub updated_at: u128,
    /// A list of authors that have contributes commits to this branch.
    /// In the case of multiple remote tracking branches, it takes the full list of unique authors.
    pub authors: Vec<Author>,
    /// Determines if the branch is considered one created by the user
    /// A branch is considered created by the user if they were the author of the first commit in the branch.
    pub own_branch: bool,
}

/// Represents a "commit author" or "signature", based on the data from ther git history
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub struct Author {
    /// The name of the author as configured in the git config
    pub name: Option<String>,
    /// The email of the author as configured in the git config
    pub email: Option<String>,
}

impl From<git2::Signature<'_>> for Author {
    fn from(value: git2::Signature) -> Self {
        let name = value.name().map(str::to_string);
        let email = value.email().map(str::to_string);
        Author { name, email }
    }
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
    pub branch: BranchListing,
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
