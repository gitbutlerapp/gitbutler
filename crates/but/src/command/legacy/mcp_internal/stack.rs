use std::path::Path;

use bstr::{BString, ByteSlice};
use but_ctx::Context;
use gitbutler_stack::{PatchReferenceUpdate, VirtualBranchesHandle};
use serde::Serialize;

/// Get the details of a branch by its name.
///
/// This includes information about the branch itself and its commits
pub fn branch_details(ref_name: &str, current_dir: &Path) -> anyhow::Result<BranchDetails> {
    let project = super::project::project_from_path(current_dir)?;
    let ctx = Context::new_from_legacy_project(project.clone())?;
    let meta = super::project::ref_metadata_toml(&ctx.legacy_project)?;
    let repo = ctx.clone_repo_for_merging_non_persisting()?;
    let ref_name = repo.find_reference(ref_name)?.name().to_owned();

    let details = but_workspace::branch_details(&repo, ref_name.as_ref(), &meta)?;
    Ok(parse_branch_details(&repo, details))
}

/// Create a new stack containing only a branch with the given name.
pub fn create_stack_with_branch(
    name: &str,
    description: &str,
    current_dir: &Path,
) -> anyhow::Result<but_workspace::legacy::ui::StackEntryNoOpt> {
    let project = super::project::project_from_path(current_dir)?;
    let ctx = Context::new_from_legacy_project(project.clone())?;

    let creation_request = gitbutler_branch::BranchCreateRequest {
        name: Some(name.to_string()),
        ..Default::default()
    };

    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        &ctx,
        &creation_request,
        ctx.exclusive_worktree_access().write_permission(),
    )?;

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let mut stack = vb_state.get_stack(stack_entry.id)?;
    stack.update_branch(
        &ctx,
        name.to_string(),
        &PatchReferenceUpdate {
            description: Some(Some(description.to_string())),
            ..Default::default()
        },
    )?;

    Ok(stack_entry)
}

/// Commit that is a part of a [`StackBranch`](gitbutler_stack::StackBranch) and, as such, containing state derived in relation to the specific branch.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    /// The OID of the commit.
    #[serde(with = "but_serde::object_id")]
    pub id: gix::ObjectId,
    /// The message of the commit.
    #[serde(with = "but_serde::bstring_lossy")]
    pub message: BString,
    /// The author of the commit.
    pub author: String,
    /// The files changed in the commit, if any.
    pub files: Vec<String>,
}

impl std::fmt::Debug for Commit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Commit({short_hex}, {message:?}",
            short_hex = self.id.to_hex_with_len(7),
            message = self.message.trim().as_bstr(),
        )
    }
}
/// Commit that is only at the remote.
/// Unlike the `Commit` struct, there is no knowledge of GitButler concepts like conflicted state etc.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamCommit {
    /// The OID of the commit.
    #[serde(with = "but_serde::object_id")]
    pub id: gix::ObjectId,
    /// The message of the commit.
    #[serde(with = "but_serde::bstring_lossy")]
    pub message: BString,
    /// The author of the commit.
    pub author: String,
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

impl From<but_workspace::ui::UpstreamCommit> for UpstreamCommit {
    fn from(commit: but_workspace::ui::UpstreamCommit) -> Self {
        UpstreamCommit {
            id: commit.id,
            message: commit.message,
            author: format!("{} <{}>", commit.author.name, commit.author.email),
        }
    }
}

/// Information about the current state of a branch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDetails {
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
    /// This is the last commit in the branch, aka the tip of the branch.
    /// If this is the only branch in the stack or the top-most branch, this is the tip of the stack.
    #[serde(with = "but_serde::object_id")]
    pub tip: gix::ObjectId,
    /// All authors of the commits in the branch.
    pub authors: Vec<String>,
    /// The commits contained in the branch, excluding the upstream commits.
    pub commits: Vec<Commit>,
    /// The commits that are only at the remote.
    pub upstream_commits: Vec<UpstreamCommit>,
}
fn parse_branch_details(
    repo: &gix::Repository,
    details: but_workspace::ui::BranchDetails,
) -> BranchDetails {
    let authors = details
        .authors
        .into_iter()
        .map(|author| format!("{} <{}>", author.name, author.email))
        .collect();
    BranchDetails {
        name: details.name,
        remote_tracking_branch: details.remote_tracking_branch,
        description: details.description,
        pr_number: details.pr_number,
        review_id: details.review_id,
        tip: details.tip,
        authors,
        commits: parse_commits(repo, details.commits, details.base_commit),
        upstream_commits: details
            .upstream_commits
            .into_iter()
            .map(UpstreamCommit::from)
            .collect(),
    }
}

fn parse_commits(
    repo: &gix::Repository,
    commits: Vec<but_workspace::ui::Commit>,
    base_commit: gix::ObjectId,
) -> Vec<Commit> {
    let mut prev = base_commit;
    commits
        .into_iter()
        .map(|commit| {
            let id = commit.id;
            let message = commit.message;
            let author = format!("{} <{}>", commit.author.name, commit.author.email);
            let changes =
                but_core::diff::tree_changes(repo, Some(prev), commit.id).unwrap_or_default();

            let files = changes
                .into_iter()
                .map(|change| change.path.to_string())
                .collect::<Vec<String>>();

            let commit_obj = Commit {
                id,
                message,
                author,
                files,
            };
            prev = id;
            commit_obj
        })
        .collect()
}
