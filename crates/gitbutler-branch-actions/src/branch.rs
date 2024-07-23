use std::collections::HashMap;
use std::vec;

use anyhow::Result;
use bstr::BString;
use gitbutler_branch::Branch as GitButlerBranch;
use gitbutler_branch::BranchId;
use gitbutler_branch::VirtualBranchesHandle;
use gitbutler_command_context::ProjectRepository;

use serde::Serialize;

use crate::{author::Author, VirtualBranch, VirtualBranchesExt};

/// Returns a list of branches associated with this project.
// TODO: Implement pagination for this thing
pub fn list_branches(project_repository: &ProjectRepository) -> Result<Vec<Branch>> {
    let vb_handle = project_repository.project().virtual_branches();
    let repo = gix::open(project_repository.project().path.clone())?;
    let refs = repo.references()?;

    // all branches that git knows about
    let branches = refs
        .local_branches()?
        .chain(refs.remote_branches()?)
        .filter_map(Result::ok)
        .filter(|branch| should_list_git_branch(branch, &vb_handle));

    // virtual branches from the application state
    let virtual_branches = project_repository
        .project()
        .virtual_branches()
        .list_all_branches()?
        .into_iter();

    combine_branches(branches, virtual_branches)
}

fn combine_branches<'a>(
    git_branches: impl Iterator<Item = gix::Reference<'a>>,
    virtual_branches: impl Iterator<Item = GitButlerBranch>,
) -> Result<Vec<Branch>> {
    let mut group_branches: Vec<GroupBranch> = vec![];
    for branch in virtual_branches {
        group_branches.push(GroupBranch::Virtual(branch));
    }
    for branch in git_branches {
        group_branches.push(GroupBranch::Git(branch));
    }
    // Group branches by identity
    let mut groups: HashMap<BString, Vec<&GroupBranch>> = HashMap::new();
    for branch in group_branches.iter() {
        let identity = branch.identity();
        if let Some(group) = groups.get_mut(&identity) {
            group.push(branch);
        } else {
            groups.insert(identity, vec![branch]);
        }
    }
    // Convert to Branch entries for the API response
    let branches: Vec<Branch> = groups
        .iter()
        .filter_map(|(identity, group_branches)| {
            branch_group_to_branch(identity.clone(), group_branches.clone())
        })
        .collect();
    Ok(branches)
}

fn branch_group_to_branch(identity: BString, group_branches: Vec<&GroupBranch>) -> Option<Branch> {
    if group_branches.is_empty() {
        // Nothing to do - this should not be reachable.
        return None;
    }
    // Virtual branch associated with this branch
    let virtual_branch_reference = group_branches.iter().find_map(|branch| match branch {
        GroupBranch::Virtual(vb) => Some(VirtualBranchReference {
            given_name: vb.name.clone(),
            id: vb.id,
            in_workspace: vb.in_workspace,
        }),
        _ => None,
    });
    let remotes: Vec<BString> = group_branches
        .iter()
        .filter_map(|branch| match branch {
            GroupBranch::Git(git_branch) => Some(git_branch),
            _ => None,
        })
        .filter(|reference| {
            reference.remote(gix::remote::Direction::Fetch).is_some()
                || reference.remote(gix::remote::Direction::Push).is_some()
        })
        .map(|reference| {
            let fetch = reference.remote_name(gix::remote::Direction::Fetch);
            let push = reference.remote_name(gix::remote::Direction::Push);
            fetch.unwrap_or(push.unwrap()).as_bstr().into() // Already checked that at least one of them is present
        })
        .collect();

    let branch = Branch {
        name: identity.to_string(),
        remotes,
        virtual_branch: virtual_branch_reference,
        lines_added: 0,       // TODO
        lines_removed: 0,     // TODO
        number_of_commits: 0, // TODO
        updated_at: 0,        // TODO
        authors: vec![],      // TODO
        own_branch: false,    // TODO
    };
    Some(branch)
}

/// A sum type of a branch that can be a plain git branch or a virtual branch
#[allow(clippy::large_enum_variant)]
enum GroupBranch<'a> {
    Git(gix::Reference<'a>),
    Virtual(GitButlerBranch),
}

impl GroupBranch<'_> {
    /// A name identifier for the branch. When multiple branches (e.g. virtual, local, reomte) have the same identity,
    /// they are grouped together under the same `Branch` entry.
    fn identity(&self) -> BString {
        match self {
            GroupBranch::Git(branch) => branch.name().shorten().into(),
            // When a user changes the remote name via the "set remote branch name" in the UI,
            // the virtual branch will be in a different group. This is probably the desired behavior.
            GroupBranch::Virtual(branch) => branch
                .upstream
                .clone()
                .map_or(BString::default(), |x| x.branch().into()),
        }
    }
}

/// Determines if a branch should be listed in the UI.
/// This excludes the target branch as well as gitbutler specific branches.
fn should_list_git_branch(branch: &gix::Reference, vb_handle: &VirtualBranchesHandle) -> bool {
    let name: BString = branch.name().shorten().into();
    // Exclude the target branch
    if let Ok(target) = vb_handle.get_default_target() {
        if name == target.branch.branch() && name == target.branch.remote() {
            return false;
        }
    }
    // Exclude gitbutler technical branches (not useful for the user)
    if name == "gitbutler/integration" || name == "gitbutler/target" {
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
pub struct Branch {
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
