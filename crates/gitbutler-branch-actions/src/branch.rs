use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    vec,
};

use anyhow::{Context, Result};
use bstr::{BString, ByteSlice};
use gitbutler_branch::{
    Branch as GitButlerBranch, BranchId, ReferenceExt, Target, VirtualBranchesHandle,
};
use gitbutler_command_context::CommandContext;
use gitbutler_reference::normalize_branch_name;
use gitbutler_repo::RepoActionsExt;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::VirtualBranchesExt;

/// Returns a list of branches associated with this project.
pub fn list_branches(
    ctx: &CommandContext,
    filter: Option<BranchListingFilter>,
) -> Result<Vec<BranchListing>> {
    let vb_handle = ctx.project().virtual_branches();
    // The definition of "own_branch" is based if the current user made the first commit on the branch
    // However, because getting that info is both expensive and also we cant filter ahead of time,
    // here we assume that all of the "own_branches" will be local.
    let branch_filter = filter
        .as_ref()
        .and_then(|filter| match filter.own_branches {
            Some(true) => Some(git2::BranchType::Local),
            _ => None,
        });
    let mut git_branches: Vec<GroupBranch> = vec![];
    for result in ctx.repository().branches(branch_filter)? {
        match result {
            Ok((branch, branch_type)) => match branch_type {
                git2::BranchType::Local => {
                    if branch_filter
                        .map(|branch_type| branch_type == git2::BranchType::Local)
                        .unwrap_or(false)
                    {
                        // If we had an "own_branch" filter, we skipped getting the remote branches, however we still want the remote
                        // tracking branches for the ones that are local
                        if let Ok(upstream) = branch.upstream() {
                            git_branches.push(GroupBranch::Remote(upstream));
                        }
                    }
                    git_branches.push(GroupBranch::Local(branch));
                }
                git2::BranchType::Remote => {
                    git_branches.push(GroupBranch::Remote(branch));
                }
            },
            Err(_) => {
                continue;
            }
        }
    }

    // virtual branches from the application state
    let virtual_branches = ctx
        .project()
        .virtual_branches()
        .list_all_branches()?
        .into_iter();

    let branches = combine_branches(git_branches, virtual_branches, ctx, &vb_handle)?;
    // Apply the filter
    let branches: Vec<BranchListing> = branches
        .into_iter()
        .filter(|branch| matches_all(branch, &filter))
        .sorted_by(|a, b| b.updated_at.cmp(&a.updated_at))
        .collect();
    Ok(branches)
}

fn matches_all(branch: &BranchListing, filter: &Option<BranchListingFilter>) -> bool {
    if let Some(filter) = filter {
        let mut conditions: Vec<bool> = vec![];
        if let Some(applied) = filter.applied {
            if let Some(vb) = branch.virtual_branch.as_ref() {
                conditions.push(applied == vb.in_workspace);
            } else {
                conditions.push(!applied);
            }
        }
        if let Some(own) = filter.own_branches {
            conditions.push(own == branch.own_branch);
        }
        return conditions.iter().all(|&x| x);
    } else {
        true
    }
}

fn combine_branches(
    mut group_branches: Vec<GroupBranch>,
    virtual_branches: impl Iterator<Item = GitButlerBranch>,
    ctx: &CommandContext,
    vb_handle: &VirtualBranchesHandle,
) -> Result<Vec<BranchListing>> {
    let repo = ctx.repository();
    for branch in virtual_branches {
        group_branches.push(GroupBranch::Virtual(branch));
    }
    let remotes = repo.remotes()?;
    let target_branch = vb_handle.get_default_target()?;

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
    let (local_author, _committer) = ctx.signatures()?;

    // Convert to Branch entries for the API response, filtering out any errors
    let branches: Vec<BranchListing> = groups
        .iter()
        .filter_map(|(identity, group_branches)| {
            let branch_entry = branch_group_to_branch(
                identity.clone(),
                group_branches.clone(),
                repo,
                &local_author,
                target_branch.sha,
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
    local_author: &git2::Signature,
    target_sha: git2::Oid,
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
    let last_modified_ms = max(
        (repo.find_commit(head)?.time().seconds() * 1000) as u128,
        virtual_branch.map_or(0, |x| x.updated_timestamp_ms),
    );
    // If no merge base can be found, return with zero stats
    let branch = if let Ok(base) = repo.merge_base(target_sha, head) {
        let mut revwalk = repo.revwalk()?;
        revwalk.push(head)?;
        revwalk.hide(base)?;
        let mut commits = Vec::new();
        let mut authors = HashSet::new();
        for oid in revwalk {
            let commit = repo.find_commit(oid?)?;
            authors.insert(commit.author().into());
            commits.push(commit);
        }
        // If there are no commits (i.e. virtual branch only) it is considered the users own
        let own_branch = commits.is_empty()
            || commits.iter().any(|commit| {
                let commit_author = commit.author();
                local_author.name_bytes() == commit_author.name_bytes()
                    && local_author.email_bytes() == commit_author.email_bytes()
            });

        BranchListing {
            name: identity,
            remotes,
            virtual_branch: virtual_branch_reference,
            number_of_commits: commits.len(),
            updated_at: last_modified_ms,
            authors: authors.into_iter().collect(),
            own_branch,
            head,
        }
    } else {
        BranchListing {
            name: identity,
            remotes,
            virtual_branch: virtual_branch_reference,
            number_of_commits: 0,
            updated_at: last_modified_ms,
            authors: Vec::new(),
            own_branch: false,
            head,
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
fn should_list_git_branch(identity: &Option<String>, target: &Target) -> bool {
    // Exclude the target branch
    if identity == &Some(target.branch.branch().to_owned()) {
        return false;
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

/// A filter that can be applied to the branch listing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchListingFilter {
    /// If the value is true, the listing will only include branches that have the same author as the current user.
    /// If the value is false, the listing will include only branches that are not created by the user.
    pub own_branches: Option<bool>,
    /// If the value is true, the listing will only include branches that are applied in the workspace.
    /// If the value is false, the listing will only include branches that are not applied in the workspace.
    pub applied: Option<bool>,
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
    /// Determines if the current user is involved with this branch.
    /// Returns true if the author has created a commit on this branch
    /// If it is a virtual branch, if it has zero commits it is also considered as the user's branch
    pub own_branch: bool,
    /// The head of interest for the branch group, used for calculating branch statistics.
    /// If there is a virtual branch, a local branch and remote branches, the head is determined in the following order:
    /// 1. The head of the virtual branch
    /// 2. The head of the local branch
    /// 3. The head of the first remote branch
    #[serde(skip)]
    head: git2::Oid,
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

/// Takes a list of branch names (the given name, as returned by `BranchListing`) and returns
/// a list of enriched branch data in the form of `BranchData`.
pub fn get_branch_listing_details(
    ctx: &CommandContext,
    branch_names: Vec<String>,
) -> Result<Vec<BranchListingDetails>> {
    let repo = ctx.repository();
    // Can we do this in a more efficient way?
    let branches = list_branches(ctx, None)?
        .into_iter()
        .filter(|branch| branch_names.contains(&branch.name))
        .collect::<Vec<_>>();
    let repo_head = repo.head()?.peel_to_commit()?;
    let mut enriched_branches = Vec::new();
    for branch in branches {
        if let Ok(base) = repo.merge_base(repo_head.id(), branch.head) {
            let base_tree = repo.find_commit(base)?.tree()?;
            let head_tree = repo.find_commit(branch.head)?.tree()?;
            let diff_stats = repo
                .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), None)?
                .stats()?;
            let branch_data = BranchListingDetails {
                name: branch.name,
                lines_added: diff_stats.insertions(),
                lines_removed: diff_stats.deletions(),
                number_of_files: diff_stats.files_changed(),
            };
            enriched_branches.push(branch_data);
        }
    }
    Ok(enriched_branches)
}

/// Represents a fat struct with all the data associated with a branch
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchListingDetails {
    /// The name of the branch (e.g. `main`, `feature/branch`), excluding the remote name
    pub name: String,
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
