use crate::VirtualBranchesExt;
use anyhow::{Context, Result};
use bstr::{BStr, BString, ByteSlice};
use core::fmt;
use gitbutler_branch::{Branch as GitButlerBranch, BranchId, ReferenceExtGix, Target};
use gitbutler_command_context::CommandContext;
use gitbutler_reference::normalize_branch_name;
use gix::prelude::ObjectIdExt;
use gix::reference::Category;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    fmt::Debug,
    vec,
};

/// Returns a list of branches associated with this project.
pub fn list_branches(
    ctx: &CommandContext,
    filter: Option<BranchListingFilter>,
    filter_branch_names: Option<Vec<BranchIdentity>>,
) -> Result<Vec<BranchListing>> {
    let mut repo = gix::open(ctx.repository().path())?;
    repo.object_cache_size_if_unset(1024 * 1024);
    let has_filter = filter.is_some();
    let filter = filter.unwrap_or_default();
    let vb_handle = ctx.project().virtual_branches();
    let platform = repo.references()?;
    let mut branches: Vec<GroupBranch> = vec![];
    for reference in platform.all()?.filter_map(Result::ok) {
        // Loosely match on branch names
        if let Some(branch_names) = &filter_branch_names {
            let has_matching_name = branch_names
                .iter()
                .any(|branch_name| reference.name().as_bstr().ends_with_str(&branch_name.0));

            if !has_matching_name {
                continue;
            }
        }

        let is_local_branch = match reference.name().category() {
            Some(Category::LocalBranch) => true,
            Some(Category::RemoteBranch) => false,
            _ => continue,
        };
        branches.push(if is_local_branch {
            GroupBranch::Local(reference)
        } else {
            GroupBranch::Remote(reference)
        });
    }

    let virtual_branches = vb_handle.list_all_branches()?;

    for branch in virtual_branches {
        branches.push(GroupBranch::Virtual(branch));
    }
    let mut branches = combine_branches(branches, &repo, vb_handle.get_default_target()?)?;

    // Apply the filter
    branches.retain(|branch| !has_filter || matches_all(branch, filter));

    // Filter out virtual branches which have no local or remote branches
    branches.retain(|branch| {
        // If there is no virtual branch, keep the grouping
        let Some(virtual_branch) = &branch.virtual_branch else {
            return true;
        };

        // If the virtual branch is applied, keep the grouping
        if virtual_branch.in_workspace {
            return true;
        }

        // If the virtual branch has a local branch, keep the grouping
        if branch.has_local {
            return true;
        };

        // If the virtual branch has remotes, keep the grouping
        if !branch.remotes.is_empty() {
            return true;
        };

        // Otherwise, drop the grouping
        false
    });

    if let Some(branch_names) = filter_branch_names {
        branches.retain(|branch_listing| branch_names.contains(&branch_listing.name))
    }

    Ok(branches)
}

fn matches_all(branch: &BranchListing, filter: BranchListingFilter) -> bool {
    let mut conditions = vec![];
    if let Some(applied) = filter.applied {
        if let Some(vb) = branch.virtual_branch.as_ref() {
            conditions.push(applied == vb.in_workspace);
        } else {
            conditions.push(!applied);
        }
    }
    if let Some(local) = filter.local {
        conditions.push((branch.has_local || branch.virtual_branch.is_some()) && local);
    }
    return conditions.iter().all(|&x| x);
}

fn combine_branches(
    group_branches: Vec<GroupBranch>,
    repo: &gix::Repository,
    target_branch: Target,
) -> Result<Vec<BranchListing>> {
    let remotes = repo.remote_names();
    let packed = repo.refs.cached_packed_buffer()?;

    // Group branches by identity
    let mut groups: HashMap<BranchIdentity, Vec<GroupBranch>> = HashMap::new();
    for branch in group_branches {
        let Some(identity) = branch.identity(&remotes) else {
            continue;
        };
        // Skip branches that should not be listed, e.g. the target 'main' or the gitbutler technical branches like 'gitbutler/integration'
        if !should_list_git_branch(&identity) {
            continue;
        }
        groups.entry(identity).or_default().push(branch);
    }

    // Convert to Branch entries for the API response, filtering out any errors
    Ok(groups
        .into_iter()
        .filter_map(|(identity, group_branches)| {
            let res = branch_group_to_branch(
                &identity,
                group_branches,
                repo,
                packed.as_ref().map(|p| &***p),
                &remotes,
                &target_branch,
            );
            match res {
                Ok(branch_entry) => branch_entry,
                Err(err) => {
                    tracing::warn!(
                        "Failed to process branch group {:?} to branch entry: {}",
                        identity,
                        err
                    );
                    None
                }
            }
        })
        .collect())
}

/// Converts a group of branches with the same identity into a single branch entry
fn branch_group_to_branch(
    identity: &BranchIdentity,
    group_branches: Vec<GroupBranch>,
    repo: &gix::Repository,
    packed: Option<&gix::refs::packed::Buffer>,
    remotes: &BTreeSet<Cow<'_, BStr>>,
    target: &Target,
) -> Result<Option<BranchListing>> {
    let (local_branches, remote_branches, mut vbranches) =
        group_branches
            .into_iter()
            .fold((Vec::new(), Vec::new(), Vec::new()), |mut acc, item| {
                match item {
                    GroupBranch::Local(branch) => acc.0.push(branch),
                    GroupBranch::Remote(branch) => acc.1.push(branch),
                    GroupBranch::Virtual(branch) => acc.2.push(branch),
                }
                acc
            });

    let virtual_branch = if vbranches.len() > 1 {
        vbranches.sort_by_key(|virtual_branch| virtual_branch.updated_timestamp_ms);
        vbranches.last()
    } else {
        vbranches.first()
    };

    if virtual_branch.is_none()
        && local_branches
            .iter()
            .any(|b| b.name().given_name(remotes).as_deref().ok() == Some(target.branch.branch()))
    {
        return Ok(None);
    }

    // Virtual branch associated with this branch
    let virtual_branch_reference = virtual_branch.map(|branch| VirtualBranchReference {
        given_name: branch.name.clone(),
        id: branch.id,
        in_workspace: branch.in_workspace,
    });

    let mut remotes: Vec<gix::remote::Name<'static>> = Vec::new();
    for branch in remote_branches.iter() {
        if let Some(remote_name) = branch.remote_name(gix::remote::Direction::Fetch) {
            remotes.push(remote_name.to_owned());
        }
    }

    let has_local = !local_branches.is_empty();

    // The head commit for which we calculate statistics.
    // If there is a virtual branch let's get it's head. Alternatively, pick the first local branch and use it's head.
    // If there are no local branches, pick the first remote branch.
    let head_commit = if let Some(vbranch) = virtual_branch {
        Some(git2_to_gix_object_id(vbranch.head).attach(repo))
    } else if let Some(mut branch) = local_branches.into_iter().next() {
        branch.peel_to_id_in_place_packed(packed).ok()
    } else if let Some(mut branch) = remote_branches.into_iter().next() {
        branch.peel_to_id_in_place_packed(packed).ok()
    } else {
        None
    }
    .context("Could not get any valid reference in order to build branch stats")?;

    let head = gix_to_git2_oid(head_commit.detach());
    let head_commit = head_commit.object()?.try_into_commit()?;
    let head_commit = head_commit.decode()?;
    let last_modified_ms = max(
        (head_commit.time().seconds * 1000) as u128,
        virtual_branch.map_or(0, |x| x.updated_timestamp_ms),
    );
    let last_commiter = head_commit.author().into();

    Ok(Some(BranchListing {
        name: identity.to_owned(),
        remotes,
        virtual_branch: virtual_branch_reference,
        updated_at: last_modified_ms,
        last_commiter,
        has_local,
        head,
    }))
}

fn gix_to_git2_oid(id: gix::ObjectId) -> git2::Oid {
    git2::Oid::from_bytes(id.as_bytes()).expect("always valid")
}

fn git2_to_gix_object_id(id: git2::Oid) -> gix::ObjectId {
    gix::ObjectId::try_from(id.as_bytes()).expect("git2 oid is always valid")
}

/// A sum type of branch that can be a plain git branch or a virtual branch
#[allow(clippy::large_enum_variant)]
enum GroupBranch<'a> {
    Local(gix::Reference<'a>),
    Remote(gix::Reference<'a>),
    Virtual(GitButlerBranch),
}

impl fmt::Debug for GroupBranch<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroupBranch::Local(branch) | GroupBranch::Remote(branch) => formatter
                .debug_struct("GroupBranch::Local/Remote")
                .field(
                    "0",
                    &format!(
                        "id: {:?}, name: {}",
                        branch.target(),
                        branch.name().as_bstr()
                    )
                    .as_str(),
                )
                .finish(),
            GroupBranch::Virtual(branch) => formatter
                .debug_struct("GroupBranch::Virtal")
                .field("0", branch)
                .finish(),
        }
    }
}

impl GroupBranch<'_> {
    /// A name identifier for the branch. When multiple branches (e.g. virtual, local, remote) have the same identity,
    /// they are grouped together under the same `Branch` entry.
    /// `None` means an identity could not be obtained, which makes this branch odd enough to ignore.
    fn identity(&self, remotes: &BTreeSet<Cow<'_, BStr>>) -> Option<BranchIdentity> {
        match self {
            GroupBranch::Local(branch) | GroupBranch::Remote(branch) => {
                branch.name().given_name(remotes).ok()
            }
            // The identity of a Virtual branch is derived from the source refname, upstream or the branch given name, in that order
            GroupBranch::Virtual(branch) => {
                let name_from_source = branch.source_refname.as_ref().and_then(|n| n.branch());
                let name_from_upstream = branch.upstream.as_ref().map(|n| n.branch());
                let rich_name = branch.name.clone();
                let rich_name = normalize_branch_name(&rich_name).ok()?;
                let identity = name_from_source.unwrap_or(name_from_upstream.unwrap_or(&rich_name));
                Some(identity.to_string())
            }
        }
        .map(BranchIdentity)
    }
}

/// Determines if a branch should be listed in the UI.
/// This excludes the target branch as well as gitbutler specific branches.
fn should_list_git_branch(identity: &BranchIdentity) -> bool {
    // Exclude gitbutler technical branches (not useful for the user)
    let is_technical = [
        "gitbutler/integration",
        "gitbutler/target",
        "gitbutler/oplog",
        "HEAD",
    ]
    .contains(&&*identity.0);
    !is_technical
}

/// A filter that can be applied to the branch listing
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchListingFilter {
    /// If the value is true, the listing will only include branches that have local references or virtual branches.
    /// If the value is false, the listing will include only branches that have local references or virtual branches.
    pub local: Option<bool>,
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
    /// The `identity` of the branch (e.g. `main`, `feature/branch`), excluding the remote name.
    pub name: BranchIdentity,
    /// This is a list of remotes that this branch can be found on (e.g. `origin`, `upstream` etc.),
    /// by collecting remotes from all local branches with the same identity that have a tracking setup.
    #[serde(serialize_with = "gitbutler_serde::as_string_lossy_vec_remote_name")]
    pub remotes: Vec<gix::remote::Name<'static>>,
    /// The branch may or may not have a virtual branch associated with it.
    pub virtual_branch: Option<VirtualBranchReference>,
    /// Timestamp in milliseconds since the branch was last updated.
    /// This includes any commits, uncommited changes or even updates to the branch metadata (e.g. renaming).
    pub updated_at: u128,
    /// The person who commited the head commit.
    pub last_commiter: Author,
    /// Whether there is a local branch under the name.
    pub has_local: bool,
    /// The head of interest for the branch group, used for calculating branch statistics.
    /// If there is a virtual branch, a local branch and remote branches, the head is determined in the following order:
    /// 1. The head of the virtual branch
    /// 2. The head of the first local branch
    /// 3. The head of the first remote branch
    #[serde(skip)]
    pub head: git2::Oid,
}

/// Represents a "commit author" or "signature", based on the data from the git history
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
pub struct Author {
    /// The name of the author as configured in the git config
    pub name: Option<BString>,
    /// The email of the author as configured in the git config
    pub email: Option<BString>,
}

/// The identity of a branch as to allow to group similar branches together.
///
/// * For *local* branches, it is what's left without the standard prefix, like `refs/heads`, e.g. `main`
///   for `refs/heads/main` or `feat/one` for `refs/heads/feat/one`.
/// * For *remote* branches, it is what's without the prefix and remote name, like `main` for `refs/remotes/origin/main`.
///   or `feat/one` for `refs/remotes/my/special/remote/feat/one`.
/// * For virtual branches, it's either the above if there is a `source_refname` or an `upstream`, or it's the normalized
///   name of the virtual branch.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct BranchIdentity(String);

/// Facilitate obtaining this type from the UI - otherwise it would be better not to have it as it should be
/// a particular thing, not any string.
impl From<String> for BranchIdentity {
    fn from(value: String) -> Self {
        BranchIdentity(value)
    }
}

/// Also not for testing.
impl From<&str> for BranchIdentity {
    fn from(value: &str) -> Self {
        BranchIdentity(value.into())
    }
}

impl From<git2::Signature<'_>> for Author {
    fn from(value: git2::Signature) -> Self {
        let name = value.name().map(str::to_string).map(Into::into);
        let email = value.email().map(str::to_string).map(Into::into);
        Author { name, email }
    }
}

impl From<gix::actor::SignatureRef<'_>> for Author {
    fn from(value: gix::actor::SignatureRef<'_>) -> Self {
        Author {
            name: Some(value.name.to_owned()),
            email: Some(value.email.to_owned()),
        }
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
    branch_names: impl IntoIterator<Item = impl Into<BranchIdentity>>,
) -> Result<Vec<BranchListingDetails>> {
    let branch_names: Vec<_> = branch_names.into_iter().map(Into::into).collect();
    let repo = ctx.repository();
    let branches = list_branches(ctx, None, Some(branch_names.clone()))?;
    let default_target = ctx
        .project()
        .virtual_branches()
        .get_default_target()
        .context("failed to get default target")?;
    let mut enriched_branches = Vec::new();

    let default_local_branch =
        repo.find_branch(default_target.branch.branch(), git2::BranchType::Local)?;
    let default_branch = default_local_branch.upstream()?;
    let head_commit = default_branch.get().peel_to_commit()?;

    for branch in branches {
        let merge_base_comparison = if let Some(virtual_branch) = branch.virtual_branch {
            if virtual_branch.in_workspace {
                default_target.sha
            } else {
                head_commit.id()
            }
        } else {
            head_commit.id()
        };
        if let Ok(base) = repo.merge_base(merge_base_comparison, branch.head) {
            let base_tree = repo.find_commit(base)?.tree()?;
            let head_tree = repo.find_commit(branch.head)?.tree()?;
            let diff_stats = repo
                .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), None)?
                .stats()?;

            let head = branch.head;

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
            let branch_data = BranchListingDetails {
                name: branch.name,
                lines_added: diff_stats.insertions(),
                lines_removed: diff_stats.deletions(),
                number_of_files: diff_stats.files_changed(),
                authors: authors.into_iter().collect(),
                number_of_commits: commits.len(),
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
    pub name: BranchIdentity,
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
    /// A list of authors that have contributes commits to this branch.
    /// In the case of multiple remote tracking branches, or branches whose commits are evaluated,
    /// it takes the full list of unique authors, without applying a mailmap.
    pub authors: Vec<Author>,
}
/// Represents a local branch
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BranchEntry {
    /// The name of the branch (e.g. `main`, `feature/branch`)
    pub name: String,
    /// The head commit of the branch
    #[serde(with = "gitbutler_serde::oid")]
    head: git2::Oid,
    /// The commit base of the branch
    #[serde(with = "gitbutler_serde::oid")]
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
    #[serde(with = "gitbutler_serde::oid")]
    pub id: git2::Oid,
    /// If the commit is referencing a specific change, this is its change id
    pub change_id: Option<String>,
    /// The commit message
    pub description: BString,
    /// The timestamp of the commit in milliseconds
    pub created_at: u128,
    /// The author of the commit
    pub authors: Vec<Author>,
    /// The parent commits of the commit
    #[serde(with = "gitbutler_serde::oid_vec")]
    pub parent_ids: Vec<git2::Oid>,
}
