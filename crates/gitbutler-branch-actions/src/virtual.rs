use gitbutler_branch::{dedup, BranchUpdateRequest, VirtualBranchesHandle};
use gitbutler_branch::{dedup_fmt, Branch, BranchId};
use gitbutler_branch::{reconcile_claims, BranchOwnershipClaims};
use gitbutler_branch::{OwnershipClaim, Target};
use gitbutler_command_context::ProjectRepository;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_commit::commit_headers::HasCommitHeaders;
use gitbutler_diff::{trees, FileDiff, GitHunk};
use gitbutler_diff::{Hunk, HunkHash};
use gitbutler_reference::{normalize_branch_name, Refname, RemoteRefname};
use gitbutler_repo::credentials::Helper;
use gitbutler_repo::{LogUntil, RepoActionsExt, RepositoryExt};
use std::borrow::{Borrow, Cow};
#[cfg(target_family = "unix")]
use std::os::unix::prelude::PermissionsExt;
use std::time::SystemTime;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time, vec,
};

use anyhow::{anyhow, bail, Context, Result};
use bstr::{BString, ByteSlice, ByteVec};
use diffy::{apply_bytes as diffy_apply, Line, Patch};
use git2_hooks::HookResult;
use hex::ToHex;
use serde::{Deserialize, Serialize};

use crate::author::Author;
use crate::branch_manager::BranchManagerExt;
use crate::conflicts::{self, RepoConflictsExt};
use crate::integration::get_workspace_head;
use crate::remote::{branch_to_remote_branch, RemoteBranch};
use crate::status::{get_applied_status, virtual_hunks_by_git_hunks};
use crate::VirtualBranchesExt;
use gitbutler_error::error::Code;
use gitbutler_error::error::Marker;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::rebase::{cherry_rebase, cherry_rebase_group};
use gitbutler_time::time::now_since_unix_epoch_ms;

// this struct is a mapping to the view `Branch` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
// it holds a materialized view for presentation purposes of the Branch struct in Rust
// which is our persisted data structure for virtual branches
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct VirtualBranch {
    pub id: BranchId,
    pub name: String,
    pub notes: String,
    pub active: bool,
    pub files: Vec<VirtualBranchFile>,
    pub commits: Vec<VirtualBranchCommit>,
    pub requires_force: bool, // does this branch require a force push to the upstream?
    pub conflicted: bool, // is this branch currently in a conflicted state (only for the workspace)
    pub order: usize,     // the order in which this branch should be displayed in the UI
    pub upstream: Option<RemoteBranch>, // the upstream branch where this branch pushes to, if any
    pub upstream_name: Option<String>, // the upstream branch where this branch will push to on next push
    pub base_current: bool, // is this vbranch based on the current base branch? if false, this needs to be manually merged with conflicts
    pub ownership: BranchOwnershipClaims,
    pub updated_at: u128,
    pub selected_for_changes: bool,
    pub allow_rebasing: bool,
    #[serde(with = "gitbutler_serde::serde::oid")]
    pub head: git2::Oid,
    /// The merge base between the target branch and the virtual branch
    #[serde(with = "gitbutler_serde::serde::oid")]
    pub merge_base: git2::Oid,
    /// The fork point between the target branch and the virtual branch
    #[serde(with = "gitbutler_serde::serde::oid_opt", default)]
    pub fork_point: Option<git2::Oid>,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranches {
    pub branches: Vec<VirtualBranch>,
    pub skipped_files: Vec<gitbutler_diff::FileDiff>,
}

// this is the struct that maps to the view `Commit` type in Typescript
// it is derived from walking the git commits between the `Branch.head` commit
// and the `Target.sha` commit, or, everything that is uniquely committed to
// the virtual branch we assign it to. an array of them are returned as part of
// the `VirtualBranch` struct
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchCommit {
    #[serde(with = "gitbutler_serde::serde::oid")]
    pub id: git2::Oid,
    #[serde(serialize_with = "gitbutler_serde::serde::as_string_lossy")]
    pub description: BString,
    pub created_at: u128,
    pub author: Author,
    pub is_remote: bool,
    pub files: Vec<VirtualBranchFile>,
    pub is_integrated: bool,
    #[serde(with = "gitbutler_serde::serde::oid_vec")]
    pub parent_ids: Vec<git2::Oid>,
    pub branch_id: BranchId,
    pub change_id: Option<String>,
    pub is_signed: bool,
}

// A hunk is locked when it depends on changes in commits that are in your
// workspace. A hunk can be locked to more than one branch if it overlaps
// with more than one committed hunk.
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Copy)]
#[serde(rename_all = "camelCase")]
pub struct HunkLock {
    pub branch_id: BranchId,
    #[serde(with = "gitbutler_serde::serde::oid")]
    pub commit_id: git2::Oid,
}

// this struct is a mapping to the view `File` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
// it holds a materialized view for presentation purposes of one entry of the
// `Branch.ownership` vector in Rust. an array of them are returned as part of
// the `VirtualBranch` struct, which map to each entry of the `Branch.ownership` vector
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchFile {
    // TODO(ST): `id` is just `path` as string - UI could adapt and avoid this copy.
    pub id: String,
    pub path: PathBuf,
    pub hunks: Vec<VirtualBranchHunk>,
    pub modified_at: u128,
    pub conflicted: bool,
    pub binary: bool,
    pub large: bool,
}

// this struct is a mapping to the view `Hunk` type in Typescript
// found in src-tauri/src/routes/repo/[project_id]/types.ts
// it holds a materialized view for presentation purposes of one entry of
// each hunk in one `Branch.ownership` vector entry in Rust.
// an array of them are returned as part of the `VirtualBranchFile` struct
//
// it is not persisted, it is only used for presentation purposes through the ipc
//
#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranchHunk {
    pub id: String,
    #[serde(serialize_with = "gitbutler_serde::serde::as_string_lossy")]
    pub diff: BString,
    pub modified_at: u128,
    pub file_path: PathBuf,
    #[serde(serialize_with = "gitbutler_branch::serde::hash_to_hex")]
    pub hash: HunkHash,
    pub old_start: u32,
    pub start: u32,
    pub end: u32,
    #[serde(skip)]
    pub old_lines: u32,
    pub binary: bool,
    pub locked: bool,
    pub locked_to: Option<Box<[HunkLock]>>,
    pub change_type: gitbutler_diff::ChangeType,
    /// Indicates that the hunk depends on multiple branches. In this case the hunk cant be moved or comitted.
    pub poisoned: bool,
}

impl From<VirtualBranchHunk> for GitHunk {
    fn from(val: VirtualBranchHunk) -> Self {
        GitHunk {
            old_start: val.old_start,
            old_lines: val.old_lines,
            new_start: val.start,
            new_lines: val.end - val.start,
            diff_lines: val.diff,
            binary: val.binary,
            change_type: val.change_type,
        }
    }
}

/// Lifecycle
impl VirtualBranchHunk {
    pub(crate) fn gen_id(new_start: u32, new_lines: u32) -> String {
        format!("{}-{}", new_start, new_start + new_lines)
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "value")]
pub enum NameConflictResolution {
    #[default]
    Suffix,
    Rename(String),
    Overwrite,
}

pub fn unapply_ownership(
    project_repository: &ProjectRepository,
    ownership: &BranchOwnershipClaims,
    _perm: &mut WorktreeWritePermission,
) -> Result<()> {
    project_repository.assure_resolved()?;

    let vb_state = project_repository.project().virtual_branches();

    let integration_commit_id = get_workspace_head(project_repository)?;

    let applied_statuses = get_applied_status(project_repository, None)
        .context("failed to get status by branch")?
        .branches;

    let hunks_to_unapply = applied_statuses
        .iter()
        .map(
            |(_branch, branch_files)| -> Result<Vec<(PathBuf, gitbutler_diff::GitHunk)>> {
                let mut hunks_to_unapply: Vec<(PathBuf, GitHunk)> = Vec::new();
                for (path, hunks) in branch_files {
                    let ownership_hunks: Vec<&Hunk> = ownership
                        .claims
                        .iter()
                        .filter(|o| o.file_path == *path)
                        .flat_map(|f| &f.hunks)
                        .collect();
                    for hunk in hunks {
                        let hunk: GitHunk = hunk.clone().into();
                        if ownership_hunks.contains(&&Hunk::from(&hunk)) {
                            hunks_to_unapply.push((path.clone(), hunk));
                        }
                    }
                }

                hunks_to_unapply.sort_by(|a, b| a.1.old_start.cmp(&b.1.old_start));

                Ok(hunks_to_unapply)
            },
        )
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    let mut diff = HashMap::new();
    for h in hunks_to_unapply {
        if let Some(reversed_hunk) = gitbutler_diff::reverse_hunk(&h.1) {
            diff.entry(h.0).or_insert_with(Vec::new).push(reversed_hunk);
        } else {
            bail!("failed to reverse hunk")
        }
    }

    let repo = project_repository.repo();

    let target_commit = repo
        .find_commit(integration_commit_id)
        .context("failed to find target commit")?;

    let base_tree = target_commit.tree().context("failed to get target tree")?;
    let final_tree = applied_statuses.into_iter().fold(
        target_commit.tree().context("failed to get target tree"),
        |final_tree, status| {
            let final_tree = final_tree?;
            let tree_oid = write_tree(project_repository, &integration_commit_id, status.1)?;
            let branch_tree = repo.find_tree(tree_oid)?;
            let mut result = repo.merge_trees(&base_tree, &final_tree, &branch_tree, None)?;
            let final_tree_oid = result.write_tree_to(project_repository.repo())?;
            repo.find_tree(final_tree_oid)
                .context("failed to find tree")
        },
    )?;

    let final_tree_oid = write_tree_onto_tree(project_repository, &final_tree, diff)?;
    let final_tree = repo
        .find_tree(final_tree_oid)
        .context("failed to find tree")?;

    repo.checkout_tree_builder(&final_tree)
        .force()
        .remove_untracked()
        .checkout()
        .context("failed to checkout tree")?;

    crate::integration::update_gitbutler_integration(&vb_state, project_repository)?;

    Ok(())
}

// reset a file in the project to the index state
pub(crate) fn reset_files(
    project_repository: &ProjectRepository,
    files: &Vec<String>,
) -> Result<()> {
    project_repository.assure_resolved()?;

    // for each tree, we need to checkout the entry from the index at that path
    // or if it doesn't exist, remove the file from the working directory
    let repo = project_repository.repo();
    let index = repo.index().context("failed to get index")?;
    for file in files {
        let entry = index.get_path(Path::new(file), 0);
        if entry.is_some() {
            repo.checkout_index_path_builder(Path::new(file))
                .context("failed to checkout index")?;
        } else {
            // find the project root
            let project_root = &project_repository.project().path;
            let path = Path::new(file);
            //combine the project root with the file path
            let path = &project_root.join(path);
            std::fs::remove_file(path).context("failed to remove file")?;
        }
    }

    Ok(())
}
fn find_base_tree<'a>(
    repo: &'a git2::Repository,
    branch_commit: &'a git2::Commit<'a>,
    target_commit: &'a git2::Commit<'a>,
) -> Result<git2::Tree<'a>> {
    // find merge base between target_commit and branch_commit
    let merge_base = repo
        .merge_base(target_commit.id(), branch_commit.id())
        .context("failed to find merge base")?;
    // turn oid into a commit
    let merge_base_commit = repo
        .find_commit(merge_base)
        .context("failed to find merge base commit")?;
    let base_tree = merge_base_commit
        .tree()
        .context("failed to get base tree object")?;
    Ok(base_tree)
}

/// Resolves the "old_applied" state of branches
///
/// This should only ever be called by `list_virtual_branches
///
/// This checks for the case where !branch.old_applied && branch.in_workspace
/// If this is the case, we ought to unapply the branch as its been carried
/// over from the old style of unapplying
fn resolve_old_applied_state(
    project_repository: &ProjectRepository,
    vb_state: &VirtualBranchesHandle,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let branches = vb_state.list_all_branches()?;

    let branch_manager = project_repository.branch_manager();

    for mut branch in branches {
        if branch.is_old_unapplied() {
            branch_manager.convert_to_real_branch(branch.id, Default::default(), perm)?;
        } else {
            branch.applied = branch.in_workspace;
            vb_state.set_branch(branch)?;
        }
    }

    Ok(())
}

pub fn list_virtual_branches(
    ctx: &ProjectRepository,
    // TODO(ST): this should really only shared access, but there is some internals
    //           that conditionally write things.
    perm: &mut WorktreeWritePermission,
) -> Result<(Vec<VirtualBranch>, Vec<gitbutler_diff::FileDiff>)> {
    let mut branches: Vec<VirtualBranch> = Vec::new();

    let vb_state = ctx.project().virtual_branches();

    resolve_old_applied_state(ctx, &vb_state, perm)?;

    let default_target = vb_state
        .get_default_target()
        .context("failed to get default target")?;

    // let (statuses, skipped_files, locks) = get_applied_status(ctx, Some(perm))?;
    let status = get_applied_status(ctx, Some(perm))?;
    let max_selected_for_changes = status
        .branches
        .iter()
        .filter_map(|(branch, _)| branch.selected_for_changes)
        .max()
        .unwrap_or(-1);

    for (branch, files) in status.branches {
        let repo = ctx.repo();
        update_conflict_markers(ctx, &files)?;

        let upstream_branch = match branch.clone().upstream {
            Some(upstream) => repo.find_branch_by_refname(&Refname::from(upstream))?,
            None => None,
        };

        let upstram_branch_commit = upstream_branch
            .as_ref()
            .map(|branch| branch.get().peel_to_commit())
            .transpose()
            .context(format!(
                "failed to find upstream branch commit for {}",
                branch.name
            ))?;

        // find upstream commits if we found an upstream reference
        let mut pushed_commits = HashMap::new();
        if let Some(upstream) = &upstram_branch_commit {
            let merge_base =
                repo.merge_base(upstream.id(), default_target.sha)
                    .context(format!(
                        "failed to find merge base between {} and {}",
                        upstream.id(),
                        default_target.sha
                    ))?;
            for oid in ctx.l(upstream.id(), LogUntil::Commit(merge_base))? {
                pushed_commits.insert(oid, true);
            }
        }

        let mut is_integrated = false;
        let mut is_remote = false;

        // find all commits on head that are not on target.sha
        let commits = ctx.log(branch.head, LogUntil::Commit(default_target.sha))?;
        let check_commit = IsCommitIntegrated::new(ctx, &default_target)?;
        let vbranch_commits = commits
            .iter()
            .map(|commit| {
                is_remote = if is_remote {
                    is_remote
                } else {
                    pushed_commits.contains_key(&commit.id())
                };

                // only check for integration if we haven't already found an integration
                if !is_integrated {
                    is_integrated = check_commit.is_integrated(commit)?
                };

                commit_to_vbranch_commit(ctx, &branch, commit, is_integrated, is_remote)
            })
            .collect::<Result<Vec<_>>>()?;

        let merge_base = repo
            .merge_base(default_target.sha, branch.head)
            .context("failed to find merge base")?;
        let base_current = true;

        let upstream = upstream_branch
            .and_then(|upstream_branch| branch_to_remote_branch(ctx, &upstream_branch));

        let mut files = diffs_into_virtual_files(ctx, files);

        let path_claim_positions: HashMap<&PathBuf, usize> = branch
            .ownership
            .claims
            .iter()
            .enumerate()
            .map(|(index, ownership_claim)| (&ownership_claim.file_path, index))
            .collect();

        files.sort_by(|a, b| {
            path_claim_positions
                .get(&a.path)
                .unwrap_or(&usize::MAX)
                .cmp(path_claim_positions.get(&b.path).unwrap_or(&usize::MAX))
        });

        let requires_force = is_requires_force(ctx, &branch)?;

        let fork_point = commits
            .last()
            .and_then(|c| c.parent(0).ok())
            .map(|c| c.id());

        let branch = VirtualBranch {
            id: branch.id,
            name: branch.name,
            notes: branch.notes,
            active: true,
            files,
            order: branch.order,
            commits: vbranch_commits,
            requires_force,
            upstream,
            upstream_name: branch
                .upstream
                .and_then(|r| Refname::from(r).branch().map(Into::into)),
            conflicted: conflicts::is_resolving(ctx),
            base_current,
            ownership: branch.ownership,
            updated_at: branch.updated_timestamp_ms,
            selected_for_changes: branch.selected_for_changes == Some(max_selected_for_changes),
            allow_rebasing: branch.allow_rebasing,
            head: branch.head,
            merge_base,
            fork_point,
        };
        branches.push(branch);
    }

    let mut branches = branches_with_large_files_abridged(branches);
    branches.sort_by(|a, b| a.order.cmp(&b.order));

    Ok((branches, status.skipped_files))
}

fn branches_with_large_files_abridged(mut branches: Vec<VirtualBranch>) -> Vec<VirtualBranch> {
    for branch in &mut branches {
        for file in &mut branch.files {
            // Diffs larger than 500kb are considered large
            if file.hunks.iter().any(|hunk| hunk.diff.len() > 500_000) {
                file.large = true;
                file.hunks.iter_mut().for_each(|hunk| {
                    hunk.diff.drain(..);
                });
            }
        }
    }
    branches
}

fn joined(start_a: u32, end_a: u32, start_b: u32, end_b: u32) -> bool {
    ((start_a >= start_b && start_a <= end_b) || (end_a >= start_b && end_a <= end_b))
        || ((start_b >= start_a && start_b <= end_a) || (end_b >= start_a && end_b <= end_a))
}

fn is_requires_force(project_repository: &ProjectRepository, branch: &Branch) -> Result<bool> {
    let upstream = if let Some(upstream) = &branch.upstream {
        upstream
    } else {
        return Ok(false);
    };

    let reference = match project_repository
        .repo()
        .refname_to_id(&upstream.to_string())
    {
        Ok(reference) => reference,
        Err(err) if err.code() == git2::ErrorCode::NotFound => return Ok(false),
        Err(other) => return Err(other).context("failed to find upstream reference"),
    };

    let upstream_commit = project_repository
        .repo()
        .find_commit(reference)
        .context("failed to find upstream commit")?;

    let merge_base = project_repository
        .repo()
        .merge_base(upstream_commit.id(), branch.head)?;

    Ok(merge_base != upstream_commit.id())
}

fn list_virtual_commit_files(
    project_repository: &ProjectRepository,
    commit: &git2::Commit,
) -> Result<Vec<VirtualBranchFile>> {
    if commit.parent_count() == 0 {
        return Ok(vec![]);
    }
    let parent = commit.parent(0).context("failed to get parent commit")?;
    let commit_tree = commit.tree().context("failed to get commit tree")?;
    let parent_tree = parent.tree().context("failed to get parent tree")?;
    let diff = gitbutler_diff::trees(project_repository.repo(), &parent_tree, &commit_tree)?;
    let hunks_by_filepath = virtual_hunks_by_file_diffs(&project_repository.project().path, diff);
    Ok(virtual_hunks_into_virtual_files(
        project_repository,
        hunks_by_filepath,
    ))
}

fn commit_to_vbranch_commit(
    repository: &ProjectRepository,
    branch: &Branch,
    commit: &git2::Commit,
    is_integrated: bool,
    is_remote: bool,
) -> Result<VirtualBranchCommit> {
    let timestamp = u128::try_from(commit.time().seconds())?;
    let message = commit.message_bstr().to_owned();

    let files =
        list_virtual_commit_files(repository, commit).context("failed to list commit files")?;

    let parent_ids: Vec<git2::Oid> = commit
        .parents()
        .map(|c| {
            let c: git2::Oid = c.id();
            c
        })
        .collect::<Vec<_>>();

    let commit = VirtualBranchCommit {
        id: commit.id(),
        created_at: timestamp * 1000,
        author: commit.author().into(),
        description: message,
        is_remote,
        files,
        is_integrated,
        parent_ids,
        branch_id: branch.id,
        change_id: commit.change_id(),
        is_signed: commit.is_signed(),
    };

    Ok(commit)
}

/// Integrates upstream work from a remote branch.
///
/// First we determine strategy based on preferences and branch state. If you
/// have allowed force push then it is likely branch commits frequently get
/// rebased, meaning we want to cherry pick new upstream work onto our rebased
/// commits.
///
/// If your local branch has been rebased, but you have new local only commits,
/// we _must_ rebase the upstream commits on top of the last rebased commit. We
/// do this to avoid duplicate commits, but we then need to let the user decide
/// if the local only commits get rebased on top of new upstream work or merged
/// with the new commits. The latter is sometimes preferable because you have
/// at most one merge conflict to resolve, while rebasing requires a multi-step
/// interactive process (currently not supported, so we abort).
///
/// If you do not allow force push then first validate the remote branch and
/// your local branch have the same merge base. A different merge base means
/// means either you or the remote branch has been rebased, and merging the
/// two would introduce duplicate commits (same changes, different hash).
///
/// Additionally, if we succeed in integrating the upstream commit, we still
/// need to merge the new branch tree with the working directory tree. This
/// might introduce more conflicts, but there is no need to commit at the
/// end since there will only be one parent commit.
///
pub fn integrate_upstream_commits(
    project_repository: &ProjectRepository,
    branch_id: BranchId,
) -> Result<()> {
    conflicts::is_conflicting(project_repository, None)?;

    let repo = project_repository.repo();
    let project = project_repository.project();
    let vb_state = project.virtual_branches();

    let mut branch = vb_state.get_branch_in_workspace(branch_id)?;
    let default_target = vb_state.get_default_target()?;

    let upstream_branch = branch.upstream.as_ref().context("upstream not found")?;
    let upstream_oid = repo.refname_to_id(&upstream_branch.to_string())?;
    let upstream_commit = repo.find_commit(upstream_oid)?;

    if upstream_commit.id() == branch.head {
        return Ok(());
    }

    let upstream_commits =
        project_repository.list_commits(upstream_commit.id(), default_target.sha)?;
    let branch_commits = project_repository.list_commits(branch.head, default_target.sha)?;

    let branch_commit_ids = branch_commits.iter().map(|c| c.id()).collect::<Vec<_>>();

    let branch_change_ids = branch_commits
        .iter()
        .filter_map(|c| c.change_id())
        .collect::<Vec<_>>();

    let mut unknown_commits: Vec<git2::Oid> = upstream_commits
        .iter()
        .filter(|c| {
            (!c.change_id()
                .is_some_and(|cid| branch_change_ids.contains(&cid)))
                && !branch_commit_ids.contains(&c.id())
        })
        .map(|c| c.id())
        .collect::<Vec<_>>();

    let rebased_commits = upstream_commits
        .iter()
        .filter(|c| {
            c.change_id()
                .is_some_and(|cid| branch_change_ids.contains(&cid))
                && !branch_commit_ids.contains(&c.id())
        })
        .map(|c| c.id())
        .collect::<Vec<_>>();

    // If there are no new commits then there is nothing to do.
    if unknown_commits.is_empty() {
        return Ok(());
    };

    let merge_base = repo.merge_base(default_target.sha, upstream_oid)?;

    // Booleans needed for a decision on how integrate upstream commits.
    // let is_same_base = default_target.sha == merge_base;
    let can_use_force = branch.allow_rebasing;
    let has_rebased_commits = !rebased_commits.is_empty();

    // We can't proceed if we rebased local commits but no permission to force push. In this
    // scenario we would need to "cherry rebase" new upstream commits onto the last rebased
    // local commit.
    if has_rebased_commits && !can_use_force {
        return Err(anyhow!("Cannot merge rebased commits without force push")
            .context("Aborted because force push is disallowed and commits have been rebased")
            .context(Marker::ProjectConflict));
    }

    let integration_result = match can_use_force {
        true => integrate_with_rebase(project_repository, &mut branch, &mut unknown_commits),
        false => {
            if has_rebased_commits {
                return Err(anyhow!("Cannot merge rebased commits without force push")
                    .context(
                        "Aborted because force push is disallowed and commits have been rebased",
                    )
                    .context(Marker::ProjectConflict));
            }
            integrate_with_merge(
                project_repository,
                &mut branch,
                &upstream_commit,
                merge_base,
            )
            .map(Into::into)
        }
    };

    if integration_result.as_ref().err().map_or(false, |err| {
        err.downcast_ref()
            .is_some_and(|marker: &Marker| *marker == Marker::ProjectConflict)
    }) {
        return Ok(());
    };

    let new_head = integration_result?;
    let new_head_tree = repo.find_commit(new_head)?.tree()?;
    let head_commit = repo.find_commit(new_head)?;

    let wd_tree = project_repository.repo().get_wd_tree()?;
    let integration_tree = repo
        .find_commit(get_workspace_head(project_repository)?)?
        .tree()?;

    let mut merge_index = repo.merge_trees(&integration_tree, &new_head_tree, &wd_tree, None)?;

    if merge_index.has_conflicts() {
        repo.checkout_index_builder(&mut merge_index)
            .allow_conflicts()
            .conflict_style_merge()
            .force()
            .checkout()?;
    } else {
        branch.head = new_head;
        branch.tree = head_commit.tree()?.id();
        vb_state.set_branch(branch.clone())?;
        repo.checkout_index_builder(&mut merge_index)
            .force()
            .checkout()?;
    };

    crate::integration::update_gitbutler_integration(&vb_state, project_repository)?;
    Ok(())
}

pub(crate) fn integrate_with_rebase(
    project_repository: &ProjectRepository,
    branch: &mut Branch,
    unknown_commits: &mut Vec<git2::Oid>,
) -> Result<git2::Oid> {
    cherry_rebase_group(
        project_repository,
        branch.head,
        unknown_commits.as_mut_slice(),
    )
}

pub(crate) fn integrate_with_merge(
    project_repository: &ProjectRepository,
    branch: &mut Branch,
    upstream_commit: &git2::Commit,
    merge_base: git2::Oid,
) -> Result<git2::Oid> {
    let wd_tree = project_repository.repo().get_wd_tree()?;
    let repo = project_repository.repo();
    let remote_tree = upstream_commit.tree().context("failed to get tree")?;
    let upstream_branch = branch.upstream.as_ref().context("upstream not found")?;
    // let merge_tree = repo.find_commit(merge_base).and_then(|c| c.tree())?;
    let merge_tree = repo.find_commit(merge_base)?;
    let merge_tree = merge_tree.tree()?;

    let mut merge_index = repo.merge_trees(&merge_tree, &wd_tree, &remote_tree, None)?;

    if merge_index.has_conflicts() {
        let conflicts = merge_index.conflicts()?;
        let merge_conflicts = conflicts
            .flatten()
            .filter_map(|c| c.our)
            .map(|our| gix::path::try_from_bstr(Cow::Owned(our.path.into())))
            .collect::<Result<Vec<_>, _>>()?;
        conflicts::mark(
            project_repository,
            merge_conflicts,
            Some(upstream_commit.id()),
        )?;
        repo.checkout_index_builder(&mut merge_index)
            .allow_conflicts()
            .conflict_style_merge()
            .force()
            .checkout()?;
        return Err(anyhow!("merge problem")).context(Marker::ProjectConflict);
    }

    let merge_tree_oid = merge_index.write_tree_to(project_repository.repo())?;
    let merge_tree = repo.find_tree(merge_tree_oid)?;
    let head_commit = repo.find_commit(branch.head)?;

    project_repository.commit(
        format!(
            "Merged {}/{} into {}",
            upstream_branch.remote(),
            upstream_branch.branch(),
            branch.name
        )
        .as_str(),
        &merge_tree,
        &[&head_commit, upstream_commit],
        None,
    )
}

pub fn update_branch(
    project_repository: &ProjectRepository,
    branch_update: &BranchUpdateRequest,
) -> Result<Branch> {
    let vb_state = project_repository.project().virtual_branches();
    let mut branch = vb_state.get_branch_in_workspace(branch_update.id)?;

    if let Some(ownership) = &branch_update.ownership {
        set_ownership(&vb_state, &mut branch, ownership).context("failed to set ownership")?;
    }

    if let Some(name) = &branch_update.name {
        let all_virtual_branches = vb_state
            .list_branches_in_workspace()
            .context("failed to read virtual branches")?;

        project_repository.delete_branch_reference(&branch)?;

        branch.name = dedup(
            &all_virtual_branches
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            name,
        );

        project_repository.add_branch_reference(&branch)?;
    };

    if let Some(updated_upstream) = &branch_update.upstream {
        let default_target = vb_state.get_default_target()?;
        let upstream_remote = match default_target.push_remote_name {
            Some(remote) => remote.clone(),
            None => default_target.branch.remote().to_owned(),
        };

        let remote_branch = format!(
            "refs/remotes/{}/{}",
            upstream_remote,
            normalize_branch_name(updated_upstream)
        )
        .parse::<RemoteRefname>()
        .unwrap();
        branch.upstream = Some(remote_branch);
    };

    if let Some(notes) = branch_update.notes.clone() {
        branch.notes = notes;
    };

    if let Some(order) = branch_update.order {
        branch.order = order;
    };

    if let Some(selected_for_changes) = branch_update.selected_for_changes {
        branch.selected_for_changes = if selected_for_changes {
            for mut other_branch in vb_state
                .list_branches_in_workspace()
                .context("failed to read virtual branches")?
                .into_iter()
                .filter(|b| b.id != branch.id)
            {
                other_branch.selected_for_changes = None;
                vb_state.set_branch(other_branch.clone())?;
            }
            Some(now_since_unix_epoch_ms())
        } else {
            None
        };
    };

    if let Some(allow_rebasing) = branch_update.allow_rebasing {
        branch.allow_rebasing = allow_rebasing;
    };

    vb_state.set_branch(branch.clone())?;
    Ok(branch)
}

pub(crate) fn ensure_selected_for_changes(vb_state: &VirtualBranchesHandle) -> Result<()> {
    let mut virtual_branches = vb_state
        .list_branches_in_workspace()
        .context("failed to list branches")?;

    if virtual_branches.is_empty() {
        println!("no applied branches");
        return Ok(());
    }

    if virtual_branches
        .iter()
        .any(|b| b.selected_for_changes.is_some())
    {
        println!("some branches already selected for changes");
        return Ok(());
    }

    virtual_branches.sort_by_key(|branch| branch.order);

    virtual_branches[0].selected_for_changes = Some(now_since_unix_epoch_ms());
    vb_state.set_branch(virtual_branches[0].clone())?;
    Ok(())
}

pub(crate) fn set_ownership(
    vb_state: &VirtualBranchesHandle,
    target_branch: &mut Branch,
    ownership: &BranchOwnershipClaims,
) -> Result<()> {
    if target_branch.ownership.eq(ownership) {
        // nothing to update
        return Ok(());
    }

    let virtual_branches = vb_state
        .list_branches_in_workspace()
        .context("failed to read virtual branches")?;

    let mut claim_outcomes = reconcile_claims(virtual_branches, target_branch, &ownership.claims)?;
    for claim_outcome in &mut claim_outcomes {
        if !claim_outcome.removed_claims.is_empty() {
            vb_state
                .set_branch(claim_outcome.updated_branch.clone())
                .context("failed to write ownership for branch".to_string())?;
        }
    }

    // Updates the claiming branch that was passed as mutable state with the new ownership claims
    // TODO: remove mutable reference to target_branch
    target_branch.ownership = ownership.clone();

    Ok(())
}

#[derive(Default)]
pub struct MTimeCache(HashMap<PathBuf, u128>);

impl MTimeCache {
    pub fn mtime_by_path<P: AsRef<Path>>(&mut self, path: P) -> u128 {
        let path = path.as_ref();

        if let Some(mtime) = self.0.get(path) {
            return *mtime;
        }

        let mtime = path
            .metadata()
            .map_or_else(
                |_| SystemTime::now(),
                |metadata| {
                    metadata
                        .modified()
                        .or(metadata.created())
                        .unwrap_or_else(|_| SystemTime::now())
                },
            )
            .duration_since(time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        self.0.insert(path.into(), mtime);
        mtime
    }
}

pub(crate) fn virtual_hunks_by_file_diffs<'a>(
    project_path: &'a Path,
    diff: impl IntoIterator<Item = (PathBuf, FileDiff)> + 'a,
) -> HashMap<PathBuf, Vec<VirtualBranchHunk>> {
    virtual_hunks_by_git_hunks(
        project_path,
        diff.into_iter()
            .map(move |(file_path, file)| (file_path, file.hunks)),
        None,
    )
}

pub type BranchStatus = HashMap<PathBuf, Vec<gitbutler_diff::GitHunk>>;
pub type VirtualBranchHunksByPathMap = HashMap<PathBuf, Vec<VirtualBranchHunk>>;

/// NOTE: There is no use returning an iterator here as this acts like the final product.
fn virtual_hunks_into_virtual_files(
    project_repository: &ProjectRepository,
    hunks: impl IntoIterator<Item = (PathBuf, Vec<VirtualBranchHunk>)>,
) -> Vec<VirtualBranchFile> {
    hunks
        .into_iter()
        .map(|(path, hunks)| {
            let id = path.display().to_string();
            let conflicted =
                conflicts::is_conflicting(project_repository, Some(&path)).unwrap_or(false);
            let binary = hunks.iter().any(|h| h.binary);
            let modified_at = hunks.iter().map(|h| h.modified_at).max().unwrap_or(0);
            debug_assert!(hunks.iter().all(|hunk| hunk.file_path == path));
            VirtualBranchFile {
                id,
                path,
                hunks,
                binary,
                large: false,
                modified_at,
                conflicted,
            }
        })
        .collect::<Vec<_>>()
}

// reset virtual branch to a specific commit
pub(crate) fn reset_branch(
    project_repository: &ProjectRepository,
    branch_id: BranchId,
    target_commit_id: git2::Oid,
) -> Result<()> {
    let vb_state = project_repository.project().virtual_branches();

    let default_target = vb_state.get_default_target()?;

    let mut branch = vb_state.get_branch_in_workspace(branch_id)?;
    if branch.head == target_commit_id {
        // nothing to do
        return Ok(());
    }

    if default_target.sha != target_commit_id
        && !project_repository
            .l(branch.head, LogUntil::Commit(default_target.sha))?
            .contains(&target_commit_id)
    {
        bail!("commit {target_commit_id} not in the branch");
    }

    // Compute the old workspace before resetting, so we can figure out
    // what hunks were released by this reset, and assign them to this branch.
    let old_head = get_workspace_head(project_repository)?;

    branch.head = target_commit_id;
    branch.updated_timestamp_ms = gitbutler_time::time::now_ms();
    vb_state.set_branch(branch.clone())?;

    let updated_head = get_workspace_head(project_repository)?;
    let repo = project_repository.repo();
    let diff = trees(
        repo,
        &repo
            .find_commit(updated_head)?
            .tree()
            .map_err(anyhow::Error::from)?,
        &repo
            .find_commit(old_head)?
            .tree()
            .map_err(anyhow::Error::from)?,
    )?;

    // Assign the new hunks to the branch we're working on.
    for (path, filediff) in diff {
        for hunk in filediff.hunks {
            let hash = Hunk::hash_diff(&hunk.diff_lines);
            branch.ownership.put(
                format!(
                    "{}:{}-{}-{:?}",
                    path.display(),
                    hunk.new_start,
                    hunk.new_start + hunk.new_lines,
                    &hash
                )
                .parse()?,
            );
        }
    }
    vb_state
        .set_branch(branch)
        .context("failed to write branch")?;

    crate::integration::update_gitbutler_integration(&vb_state, project_repository)
        .context("failed to update gitbutler integration")?;

    Ok(())
}

fn diffs_into_virtual_files(
    project_repository: &ProjectRepository,
    hunks_by_filepath: HashMap<PathBuf, Vec<VirtualBranchHunk>>,
) -> Vec<VirtualBranchFile> {
    virtual_hunks_into_virtual_files(project_repository, hunks_by_filepath)
}

// this function takes a list of file ownership,
// constructs a tree from those changes on top of the target
// and writes it as a new tree for storage
pub(crate) fn write_tree<T>(
    project_repository: &ProjectRepository,
    target: &git2::Oid,
    files: impl IntoIterator<Item = (impl Borrow<PathBuf>, impl Borrow<Vec<T>>)>,
) -> Result<git2::Oid>
where
    T: Into<gitbutler_diff::GitHunk> + Clone,
{
    write_tree_onto_commit(project_repository, *target, files)
}

pub(crate) fn write_tree_onto_commit<T>(
    project_repository: &ProjectRepository,
    commit_oid: git2::Oid,
    files: impl IntoIterator<Item = (impl Borrow<PathBuf>, impl Borrow<Vec<T>>)>,
) -> Result<git2::Oid>
where
    T: Into<gitbutler_diff::GitHunk> + Clone,
{
    // read the base sha into an index
    let git_repository: &git2::Repository = project_repository.repo();

    let head_commit = git_repository.find_commit(commit_oid)?;
    let base_tree = head_commit.tree()?;

    write_tree_onto_tree(project_repository, &base_tree, files)
}

pub(crate) fn write_tree_onto_tree<T>(
    project_repository: &ProjectRepository,
    base_tree: &git2::Tree,
    files: impl IntoIterator<Item = (impl Borrow<PathBuf>, impl Borrow<Vec<T>>)>,
) -> Result<git2::Oid>
where
    T: Into<gitbutler_diff::GitHunk> + Clone,
{
    let git_repository = project_repository.repo();
    let mut builder = git2::build::TreeUpdateBuilder::new();
    // now update the index with content in the working directory for each file
    for (rel_path, hunks) in files {
        let rel_path = rel_path.borrow();
        let hunks: Vec<GitHunk> = hunks.borrow().iter().map(|h| h.clone().into()).collect();
        let full_path = project_repository.project().worktree_path().join(rel_path);

        let is_submodule = full_path.is_dir()
            && hunks.len() == 1
            && hunks[0].diff_lines.contains_str(b"Subproject commit");

        // if file exists
        if full_path.exists() {
            // if file is executable, use 755, otherwise 644
            let mut filemode = git2::FileMode::Blob;
            // check if full_path file is executable
            if let Ok(metadata) = std::fs::symlink_metadata(&full_path) {
                #[cfg(target_family = "unix")]
                {
                    if metadata.permissions().mode() & 0o111 != 0 {
                        filemode = git2::FileMode::BlobExecutable;
                    }
                }

                #[cfg(target_os = "windows")]
                {
                    // NOTE: *Keep* the existing executable bit if it was present
                    //       in the tree already, don't try to derive something from
                    //       the FS that doesn't exist.
                    filemode = base_tree
                        .get_path(rel_path)
                        .ok()
                        .and_then(|entry| {
                            (entry.filemode() & 0o100000 == 0o100000
                                && entry.filemode() & 0o111 != 0)
                                .then_some(git2::FileMode::BlobExecutable)
                        })
                        .unwrap_or(filemode);
                }

                if metadata.file_type().is_symlink() {
                    filemode = git2::FileMode::Link;
                }
            }

            // get the blob
            if filemode == git2::FileMode::Link {
                // it's a symlink, make the content the path of the link
                let link_target = std::fs::read_link(&full_path)?;

                // if the link target is inside the project repository, make it relative
                let link_target = link_target
                    .strip_prefix(project_repository.project().worktree_path())
                    .unwrap_or(&link_target);

                let blob_oid = git_repository.blob(
                    link_target
                        .to_str()
                        .ok_or_else(|| {
                            anyhow!("path contains invalid utf-8 characters: {link_target:?}")
                        })?
                        .as_bytes(),
                )?;
                builder.upsert(rel_path, blob_oid, filemode);
            } else if let Ok(tree_entry) = base_tree.get_path(rel_path) {
                if hunks.len() == 1 && hunks[0].binary {
                    let new_blob_oid = &hunks[0].diff_lines;
                    // convert string to Oid
                    let new_blob_oid = new_blob_oid
                        .to_str()
                        .expect("hex-string")
                        .parse()
                        .context("failed to diff as oid")?;
                    builder.upsert(rel_path, new_blob_oid, filemode);
                } else {
                    // blob from tree_entry
                    let blob = tree_entry
                        .to_object(git_repository)
                        .unwrap()
                        .peel_to_blob()
                        .context("failed to get blob")?;

                    let blob_contents = blob.content();

                    let mut hunks = hunks.iter().collect::<Vec<_>>();
                    hunks.sort_by_key(|hunk| hunk.new_start);
                    let mut all_diffs = BString::default();
                    for hunk in hunks {
                        all_diffs.push_str(&hunk.diff_lines);
                    }

                    let patch = Patch::from_bytes(&all_diffs)?;
                    let blob_contents = apply(blob_contents, &patch).context(format!(
                        "failed to apply\n{}\nonto:\n{}",
                        all_diffs.as_bstr(),
                        blob_contents.as_bstr()
                    ));

                    match blob_contents {
                        Ok(blob_contents) => {
                            // create a blob
                            let new_blob_oid = git_repository.blob(blob_contents.as_bytes())?;
                            // upsert into the builder
                            builder.upsert(rel_path, new_blob_oid, filemode);
                        }
                        Err(_) => {
                            // If the patch failed to apply, do nothing, this is handled elsewhere
                            continue;
                        }
                    }
                }
            } else if is_submodule {
                let mut blob_contents = BString::default();

                let mut hunks = hunks.iter().collect::<Vec<_>>();
                hunks.sort_by_key(|hunk| hunk.new_start);
                let mut all_diffs = BString::default();
                for hunk in hunks {
                    all_diffs.push_str(&hunk.diff_lines);
                }
                let patch = Patch::from_bytes(&all_diffs)?;
                blob_contents = apply(&blob_contents, &patch)
                    .context(format!("failed to apply {}", all_diffs))?;

                // create a blob
                let new_blob_oid = git_repository.blob(blob_contents.as_bytes())?;
                // upsert into the builder
                builder.upsert(rel_path, new_blob_oid, filemode);
            } else {
                // create a git blob from a file on disk
                let blob_oid = git_repository
                    .blob_path(&full_path)
                    .context(format!("failed to create blob from path {:?}", &full_path))?;
                builder.upsert(rel_path, blob_oid, filemode);
            }
        } else if base_tree.get_path(rel_path).is_ok() {
            // remove file from index if it exists in the base tree
            builder.remove(rel_path);
        }
    }

    // now write out the tree
    let tree_oid = builder
        .create_updated(project_repository.repo(), base_tree)
        .context("failed to write updated tree")?;

    Ok(tree_oid)
}

#[allow(clippy::too_many_arguments)]
pub fn commit(
    project_repository: &ProjectRepository,
    branch_id: BranchId,
    message: &str,
    ownership: Option<&BranchOwnershipClaims>,
    run_hooks: bool,
) -> Result<git2::Oid> {
    let mut message_buffer = message.to_owned();

    if run_hooks {
        let hook_result = git2_hooks::hooks_commit_msg(
            project_repository.repo(),
            Some(&["../.husky"]),
            &mut message_buffer,
        )
        .context("failed to run hook")
        .context(Code::CommitHookFailed)?;

        if let HookResult::RunNotSuccessful { stdout, .. } = hook_result {
            return Err(anyhow!("commit-msg hook rejected: {}", stdout.trim())
                .context(Code::CommitHookFailed));
        }

        let hook_result =
            git2_hooks::hooks_pre_commit(project_repository.repo(), Some(&["../.husky"]))
                .context("failed to run hook")
                .context(Code::CommitHookFailed)?;

        if let HookResult::RunNotSuccessful { stdout, .. } = hook_result {
            return Err(
                anyhow!("commit hook rejected: {}", stdout.trim()).context(Code::CommitHookFailed)
            );
        }
    }

    let message = &message_buffer;

    // get the files to commit
    let statuses = get_applied_status(project_repository, None)
        .context("failed to get status by branch")?
        .branches;

    let (ref mut branch, files) = statuses
        .into_iter()
        .find(|(branch, _)| branch.id == branch_id)
        .with_context(|| format!("branch {branch_id} not found"))?;

    update_conflict_markers(project_repository, &files)
        .context(Code::CommitMergeConflictFailure)?;

    project_repository
        .assure_unconflicted()
        .context(Code::CommitMergeConflictFailure)?;

    let tree_oid = if let Some(ownership) = ownership {
        let files = files.into_iter().filter_map(|(filepath, hunks)| {
            let hunks = hunks
                .into_iter()
                .filter(|hunk| {
                    let hunk: GitHunk = hunk.clone().into();
                    ownership
                        .claims
                        .iter()
                        .find(|f| f.file_path.eq(&filepath))
                        .map_or(false, |f| {
                            f.hunks.iter().any(|h| {
                                h.start == hunk.new_start
                                    && h.end == hunk.new_start + hunk.new_lines
                            })
                        })
                })
                .collect::<Vec<_>>();
            if hunks.is_empty() {
                None
            } else {
                Some((filepath, hunks))
            }
        });
        write_tree_onto_commit(project_repository, branch.head, files)?
    } else {
        write_tree_onto_commit(project_repository, branch.head, files)?
    };

    let git_repository = project_repository.repo();
    let parent_commit = git_repository
        .find_commit(branch.head)
        .context(format!("failed to find commit {:?}", branch.head))?;
    let tree = git_repository
        .find_tree(tree_oid)
        .context(format!("failed to find tree {:?}", tree_oid))?;

    // now write a commit, using a merge parent if it exists
    let extra_merge_parent = conflicts::merge_parent(project_repository)
        .context("failed to get merge parent")
        .context(Code::CommitMergeConflictFailure)?;

    let commit_oid = match extra_merge_parent {
        Some(merge_parent) => {
            let merge_parent = git_repository
                .find_commit(merge_parent)
                .context(format!("failed to find merge parent {:?}", merge_parent))?;
            let commit_oid = project_repository.commit(
                message,
                &tree,
                &[&parent_commit, &merge_parent],
                None,
            )?;
            conflicts::clear(project_repository)
                .context("failed to clear conflicts")
                .context(Code::CommitMergeConflictFailure)?;
            commit_oid
        }
        None => project_repository.commit(message, &tree, &[&parent_commit], None)?,
    };

    if run_hooks {
        git2_hooks::hooks_post_commit(project_repository.repo(), Some(&["../.husky"]))
            .context("failed to run hook")
            .context(Code::CommitHookFailed)?;
    }

    let vb_state = project_repository.project().virtual_branches();
    branch.tree = tree_oid;
    branch.head = commit_oid;
    branch.updated_timestamp_ms = gitbutler_time::time::now_ms();
    vb_state.set_branch(branch.clone())?;

    crate::integration::update_gitbutler_integration(&vb_state, project_repository)
        .context("failed to update gitbutler integration")?;

    Ok(commit_oid)
}

pub(crate) fn push(
    project_repository: &ProjectRepository,
    branch_id: BranchId,
    with_force: bool,
    credentials: &Helper,
    askpass: Option<Option<BranchId>>,
) -> Result<()> {
    let vb_state = project_repository.project().virtual_branches();

    let mut vbranch = vb_state.get_branch_in_workspace(branch_id)?;
    let remote_branch = if let Some(upstream_branch) = &vbranch.upstream {
        upstream_branch.clone()
    } else {
        let default_target = vb_state.get_default_target()?;
        let upstream_remote = match default_target.push_remote_name {
            Some(remote) => remote.clone(),
            None => default_target.branch.remote().to_owned(),
        };

        let remote_branch = format!(
            "refs/remotes/{}/{}",
            upstream_remote,
            normalize_branch_name(&vbranch.name)
        )
        .parse::<RemoteRefname>()
        .context("failed to parse remote branch name")?;

        let remote_branches = project_repository.repo().remote_branches()?;
        let existing_branches = remote_branches
            .iter()
            .map(RemoteRefname::branch)
            .map(str::to_lowercase) // git is weird about case sensitivity here, assume not case sensitive
            .collect::<Vec<_>>();

        remote_branch.with_branch(&dedup_fmt(
            &existing_branches
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            remote_branch.branch(),
            "-",
        ))
    };

    project_repository.push(
        &vbranch.head,
        &remote_branch,
        with_force,
        credentials,
        None,
        askpass,
    )?;

    vbranch.upstream = Some(remote_branch.clone());
    vbranch.upstream_head = Some(vbranch.head);
    vb_state
        .set_branch(vbranch.clone())
        .context("failed to write target branch after push")?;
    project_repository.fetch(
        remote_branch.remote(),
        credentials,
        askpass.map(|_| "modal".to_string()),
    )?;

    Ok(())
}

struct IsCommitIntegrated<'repo> {
    repo: &'repo git2::Repository,
    target_commit_id: git2::Oid,
    remote_head_id: git2::Oid,
    upstream_commits: Vec<git2::Oid>,
    /// A repository opened at the same path as `repo`, but with an in-memory ODB attached
    /// to avoid writing intermediate objects.
    inmemory_repo: git2::Repository,
}

impl<'repo> IsCommitIntegrated<'repo> {
    fn new(ctx: &'repo ProjectRepository, target: &Target) -> anyhow::Result<Self> {
        let remote_branch = ctx
            .repo()
            .find_branch_by_refname(&target.branch.clone().into())?
            .ok_or(anyhow!("failed to get branch"))?;
        let remote_head = remote_branch.get().peel_to_commit()?;
        let upstream_commits = ctx.l(remote_head.id(), LogUntil::Commit(target.sha))?;
        let inmemory_repo = ctx.repo().in_memory_repo()?;
        Ok(Self {
            repo: ctx.repo(),
            target_commit_id: target.sha,
            remote_head_id: remote_head.id(),
            upstream_commits,
            inmemory_repo,
        })
    }
}

impl IsCommitIntegrated<'_> {
    fn is_integrated(&self, commit: &git2::Commit) -> Result<bool> {
        if self.target_commit_id == commit.id() {
            // could not be integrated if heads are the same.
            return Ok(false);
        }

        if self.upstream_commits.is_empty() {
            // could not be integrated - there is nothing new upstream.
            return Ok(false);
        }

        if self.upstream_commits.contains(&commit.id()) {
            return Ok(true);
        }

        let merge_base_id = self.repo.merge_base(self.target_commit_id, commit.id())?;
        if merge_base_id.eq(&commit.id()) {
            // if merge branch is the same as branch head and there are upstream commits
            // then it's integrated
            return Ok(true);
        }

        let merge_base = self.repo.find_commit(merge_base_id)?;
        let merge_base_tree = merge_base.tree()?;
        let upstream = self.repo.find_commit(self.remote_head_id)?;
        let upstream_tree = upstream.tree()?;

        if merge_base_tree.id() == upstream_tree.id() {
            // if merge base is the same as upstream tree, then it's integrated
            return Ok(true);
        }

        // try to merge our tree into the upstream tree
        let mut merge_index = self
            .repo
            .merge_trees(&merge_base_tree, &commit.tree()?, &upstream_tree, None)
            .context("failed to merge trees")?;

        if merge_index.has_conflicts() {
            return Ok(false);
        }

        let merge_tree_oid = merge_index
            .write_tree_to(&self.inmemory_repo)
            .context("failed to write tree")?;

        // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
        // then the vbranch is fully merged
        Ok(merge_tree_oid == upstream_tree.id())
    }
}

pub fn is_remote_branch_mergeable(
    project_repository: &ProjectRepository,
    branch_name: &RemoteRefname,
) -> Result<bool> {
    let vb_state = project_repository.project().virtual_branches();

    let default_target = vb_state.get_default_target()?;
    let target_commit = project_repository
        .repo()
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;

    let branch = project_repository
        .repo()
        .find_branch_by_refname(&branch_name.into())?
        .ok_or(anyhow!("branch not found"))?;
    let branch_oid = branch.get().target().context("detatched head")?;
    let branch_commit = project_repository
        .repo()
        .find_commit(branch_oid)
        .context("failed to find branch commit")?;

    let base_tree = find_base_tree(project_repository.repo(), &branch_commit, &target_commit)?;

    let wd_tree = project_repository.repo().get_wd_tree()?;

    let branch_tree = branch_commit.tree().context("failed to find branch tree")?;
    let mergeable = !project_repository
        .repo()
        .merge_trees(&base_tree, &branch_tree, &wd_tree, None)
        .context("failed to merge trees")?
        .has_conflicts();

    Ok(mergeable)
}

// this function takes a list of file ownership from a "from" commit and "moves"
// those changes to a "to" commit in a branch. This allows users to drag changes
// from one commit to another.
// if the "to" commit is below the "from" commit, the changes are simply added to the "to" commit
// and the rebase should be simple. if the "to" commit is above the "from" commit,
// the changes need to be removed from the "from" commit, everything rebased,
// then added to the "to" commit and everything above that rebased again.
pub(crate) fn move_commit_file(
    project_repository: &ProjectRepository,
    branch_id: BranchId,
    from_commit_id: git2::Oid,
    to_commit_id: git2::Oid,
    target_ownership: &BranchOwnershipClaims,
) -> Result<git2::Oid> {
    let vb_state = project_repository.project().virtual_branches();

    let Some(mut target_branch) = vb_state.try_branch_in_workspace(branch_id)? else {
        return Ok(to_commit_id); // this is wrong
    };

    let default_target = vb_state.get_default_target()?;

    let mut to_amend_oid = to_commit_id;
    let mut amend_commit = project_repository
        .repo()
        .find_commit(to_amend_oid)
        .context("failed to find commit")?;

    // find all the commits upstream from the target "to" commit
    let mut upstream_commits =
        project_repository.l(target_branch.head, LogUntil::Commit(amend_commit.id()))?;

    // get a list of all the diffs across all the virtual branches
    let base_file_diffs = gitbutler_diff::workdir(project_repository.repo(), &default_target.sha)
        .context("failed to diff workdir")?;

    // filter base_file_diffs to HashMap<filepath, Vec<GitHunk>> only for hunks in target_ownership
    // this is essentially the group of patches that we're "moving"
    let diffs_to_amend = target_ownership
        .claims
        .iter()
        .filter_map(|file_ownership| {
            let hunks = base_file_diffs
                .get(&file_ownership.file_path)
                .map(|hunks| {
                    hunks
                        .hunks
                        .iter()
                        .filter(|hunk| {
                            file_ownership.hunks.iter().any(|owned_hunk| {
                                owned_hunk.start == hunk.new_start
                                    && owned_hunk.end == hunk.new_start + hunk.new_lines
                            })
                        })
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if hunks.is_empty() {
                None
            } else {
                Some((file_ownership.file_path.clone(), hunks))
            }
        })
        .collect::<HashMap<_, _>>();

    // if we're not moving anything, return an error
    if diffs_to_amend.is_empty() {
        bail!("target ownership not found");
    }

    // is from_commit_oid in upstream_commits?
    if !upstream_commits.contains(&from_commit_id) {
        // this means that the "from" commit is _below_ the "to" commit in the history
        // which makes things a little more complicated because in this case we need to
        // remove the changes from the lower "from" commit, rebase everything, then add the changes
        // to the _rebased_ version of the "to" commit that is above it.

        // first, let's get the from commit data and it's parent data
        let from_commit = project_repository
            .repo()
            .find_commit(from_commit_id)
            .context("failed to find commit")?;
        let from_tree = from_commit.tree().context("failed to find tree")?;
        let from_parent = from_commit.parent(0).context("failed to find parent")?;
        let from_parent_tree = from_parent.tree().context("failed to find parent tree")?;

        // ok, what is the entire patch introduced in the "from" commit?
        // we need to remove the parts of this patch that are in target_ownership (the parts we're moving)
        // and then apply the rest to the parent tree of the "from" commit to
        // create the new "from" commit without the changes we're moving
        let from_commit_diffs =
            gitbutler_diff::trees(project_repository.repo(), &from_parent_tree, &from_tree)
                .context("failed to diff trees")?;

        // filter from_commit_diffs to HashMap<filepath, Vec<GitHunk>> only for hunks NOT in target_ownership
        // this is the patch parts we're keeping
        let diffs_to_keep = from_commit_diffs
            .iter()
            .filter_map(|(filepath, file_diff)| {
                let hunks = file_diff
                    .hunks
                    .iter()
                    .filter(|hunk| {
                        !target_ownership.claims.iter().any(|file_ownership| {
                            file_ownership.file_path.eq(filepath)
                                && file_ownership.hunks.iter().any(|owned_hunk| {
                                    owned_hunk.start == hunk.new_start
                                        && owned_hunk.end == hunk.new_start + hunk.new_lines
                                })
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if hunks.is_empty() {
                    None
                } else {
                    Some((filepath.clone(), hunks))
                }
            })
            .collect::<HashMap<_, _>>();

        let repo = project_repository.repo();

        // write our new tree and commit for the new "from" commit without the moved changes
        let new_from_tree_id =
            write_tree_onto_commit(project_repository, from_parent.id(), &diffs_to_keep)?;
        let new_from_tree = &repo
            .find_tree(new_from_tree_id)
            .with_context(|| "tree {new_from_tree_oid} not found")?;
        let new_from_commit_oid = project_repository
            .repo()
            .commit_with_signature(
                None,
                &from_commit.author(),
                &from_commit.committer(),
                &from_commit.message_bstr().to_str_lossy(),
                new_from_tree,
                &[&from_parent],
                from_commit.gitbutler_headers(),
            )
            .context("commit failed")?;

        // rebase everything above the new "from" commit that has the moved changes removed
        let new_head = match cherry_rebase(
            project_repository,
            new_from_commit_oid,
            from_commit_id,
            target_branch.head,
        ) {
            Ok(Some(new_head)) => new_head,
            Ok(None) => bail!("no rebase was performed"),
            Err(err) => return Err(err).context("rebase failed"),
        };

        // ok, now we need to identify which the new "to" commit is in the rebased history
        // so we'll take a list of the upstream oids and find it simply based on location
        // (since the order should not have changed in our simple rebase)
        let old_upstream_commit_oids =
            project_repository.l(target_branch.head, LogUntil::Commit(default_target.sha))?;

        let new_upstream_commit_oids =
            project_repository.l(new_head, LogUntil::Commit(default_target.sha))?;

        // find to_commit_oid offset in upstream_commits vector
        let to_commit_offset = old_upstream_commit_oids
            .iter()
            .position(|c| *c == to_amend_oid)
            .context("failed to find commit in old commits")?;

        // find the new "to" commit in our new rebased upstream commits
        to_amend_oid = *new_upstream_commit_oids
            .get(to_commit_offset)
            .context("failed to find commit in new commits")?;

        // reset the "to" commit variable for writing the changes back to
        amend_commit = project_repository
            .repo()
            .find_commit(to_amend_oid)
            .context("failed to find commit")?;

        // reset the concept of what the upstream commits are to be the rebased ones
        upstream_commits = project_repository.l(new_head, LogUntil::Commit(amend_commit.id()))?;
    }

    // ok, now we will apply the moved changes to the "to" commit.
    // if we were moving the changes down, we didn't need to rewrite the "from" commit
    // because it will be rewritten with the upcoming rebase.
    // if we were moving the changes "up" we've already rewritten the "from" commit

    // apply diffs_to_amend to the commit tree
    // and write a new commit with the changes we're moving
    let new_tree_oid = write_tree_onto_commit(project_repository, to_amend_oid, &diffs_to_amend)?;
    let new_tree = project_repository
        .repo()
        .find_tree(new_tree_oid)
        .context("failed to find new tree")?;
    let parents: Vec<_> = amend_commit.parents().collect();
    let commit_oid = project_repository
        .repo()
        .commit_with_signature(
            None,
            &amend_commit.author(),
            &amend_commit.committer(),
            &amend_commit.message_bstr().to_str_lossy(),
            &new_tree,
            &parents.iter().collect::<Vec<_>>(),
            amend_commit.gitbutler_headers(),
        )
        .context("failed to create commit")?;

    // now rebase upstream commits, if needed

    // if there are no upstream commits (the "to" commit was the branch head), then we're done
    if upstream_commits.is_empty() {
        target_branch.head = commit_oid;
        vb_state.set_branch(target_branch.clone())?;
        crate::integration::update_gitbutler_integration(&vb_state, project_repository)?;
        return Ok(commit_oid);
    }

    // otherwise, rebase the upstream commits onto the new commit
    let last_commit = upstream_commits.first().cloned().unwrap();
    let new_head = cherry_rebase(
        project_repository,
        commit_oid,
        amend_commit.id(),
        last_commit,
    )?;

    // if that rebase worked, update the branch head and the gitbutler integration
    if let Some(new_head) = new_head {
        target_branch.head = new_head;
        vb_state.set_branch(target_branch.clone())?;
        crate::integration::update_gitbutler_integration(&vb_state, project_repository)?;
        Ok(commit_oid)
    } else {
        Err(anyhow!("rebase failed"))
    }
}

// takes a list of file ownership and a commit oid and rewrites that commit to
// add the file changes. The branch is then rebased onto the new commit
// and the respective branch head is updated
pub(crate) fn amend(
    project_repository: &ProjectRepository,
    branch_id: BranchId,
    commit_oid: git2::Oid,
    target_ownership: &BranchOwnershipClaims,
) -> Result<git2::Oid> {
    project_repository.assure_resolved()?;
    let vb_state = project_repository.project().virtual_branches();

    let virtual_branches = vb_state
        .list_branches_in_workspace()
        .context("failed to read virtual branches")?;

    if !virtual_branches.iter().any(|b| b.id == branch_id) {
        bail!("could not find applied branch with id {branch_id} to amend to");
    }

    let default_target = vb_state.get_default_target()?;

    let mut applied_statuses = get_applied_status(project_repository, None)?.branches;

    let (ref mut target_branch, target_status) = applied_statuses
        .iter_mut()
        .find(|(b, _)| b.id == branch_id)
        .ok_or_else(|| anyhow!("could not find branch {branch_id} in status list"))?;

    if target_branch.upstream.is_some() && !target_branch.allow_rebasing {
        // amending to a pushed head commit will cause a force push that is not allowed
        bail!("force-push is not allowed");
    }

    if project_repository
        .l(target_branch.head, LogUntil::Commit(default_target.sha))?
        .is_empty()
    {
        bail!("branch has no commits - there is nothing to amend to");
    }

    // find commit oid
    let amend_commit = project_repository
        .repo()
        .find_commit(commit_oid)
        .context("failed to find commit")?;

    let diffs_to_amend = target_ownership
        .claims
        .iter()
        .filter_map(|file_ownership| {
            let hunks = target_status
                .get(&file_ownership.file_path)
                .map(|hunks| {
                    hunks
                        .iter()
                        .filter(|hunk| {
                            let hunk: GitHunk = (*hunk).clone().into();
                            file_ownership.hunks.iter().any(|owned_hunk| {
                                owned_hunk.start == hunk.new_start
                                    && owned_hunk.end == hunk.new_start + hunk.new_lines
                            })
                        })
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if hunks.is_empty() {
                None
            } else {
                Some((file_ownership.file_path.clone(), hunks))
            }
        })
        .collect::<HashMap<_, _>>();

    if diffs_to_amend.is_empty() {
        bail!("target ownership not found");
    }

    // apply diffs_to_amend to the commit tree
    let new_tree_oid = write_tree_onto_commit(project_repository, commit_oid, &diffs_to_amend)?;
    let new_tree = project_repository
        .repo()
        .find_tree(new_tree_oid)
        .context("failed to find new tree")?;

    let parents: Vec<_> = amend_commit.parents().collect();
    let commit_oid = project_repository
        .repo()
        .commit_with_signature(
            None,
            &amend_commit.author(),
            &amend_commit.committer(),
            &amend_commit.message_bstr().to_str_lossy(),
            &new_tree,
            &parents.iter().collect::<Vec<_>>(),
            amend_commit.gitbutler_headers(),
        )
        .context("failed to create commit")?;

    // now rebase upstream commits, if needed
    let upstream_commits =
        project_repository.l(target_branch.head, LogUntil::Commit(amend_commit.id()))?;
    // if there are no upstream commits, we're done
    if upstream_commits.is_empty() {
        target_branch.head = commit_oid;
        vb_state.set_branch(target_branch.clone())?;
        crate::integration::update_gitbutler_integration(&vb_state, project_repository)?;
        return Ok(commit_oid);
    }

    let last_commit = upstream_commits.first().cloned().unwrap();

    let new_head = cherry_rebase(
        project_repository,
        commit_oid,
        amend_commit.id(),
        last_commit,
    )?;

    if let Some(new_head) = new_head {
        target_branch.head = new_head;
        vb_state.set_branch(target_branch.clone())?;
        crate::integration::update_gitbutler_integration(&vb_state, project_repository)?;
        Ok(commit_oid)
    } else {
        Err(anyhow!("rebase failed"))
    }
}

// move a given commit in a branch up one or down one
// if the offset is positive, move the commit down one
// if the offset is negative, move the commit up one
// rewrites the branch head to the new head commit
pub(crate) fn reorder_commit(
    project_repository: &ProjectRepository,
    branch_id: BranchId,
    commit_oid: git2::Oid,
    offset: i32,
) -> Result<()> {
    let vb_state = project_repository.project().virtual_branches();

    let default_target = vb_state.get_default_target()?;

    let mut branch = vb_state.get_branch_in_workspace(branch_id)?;
    // find the commit to offset from
    let commit = project_repository
        .repo()
        .find_commit(commit_oid)
        .context("failed to find commit")?;

    let parent = commit.parent(0).context("failed to find parent")?;
    let parent_oid = parent.id();

    if offset < 0 {
        // move commit up
        if branch.head == commit_oid {
            // can't move the head commit up
            return Ok(());
        }

        // get a list of the commits to rebase
        let mut ids_to_rebase = project_repository.l(branch.head, LogUntil::Commit(commit.id()))?;

        ids_to_rebase.insert(
            ids_to_rebase.len() - offset.unsigned_abs() as usize,
            commit_oid,
        );

        let new_head = cherry_rebase_group(project_repository, parent_oid, &mut ids_to_rebase)
            .context("rebase failed")?;
        branch.head = new_head;
        branch.updated_timestamp_ms = gitbutler_time::time::now_ms();
        vb_state.set_branch(branch.clone())?;

        crate::integration::update_gitbutler_integration(&vb_state, project_repository)
            .context("failed to update gitbutler integration")?;
    } else {
        //  move commit down
        if default_target.sha == parent_oid {
            // can't move the commit down past the target
            return Ok(());
        }

        let mut target = parent.clone();

        for _ in 0..offset {
            target = target.parent(0).context("failed to find target")?;
        }

        let target_oid = target.id();

        // get a list of the commits to rebase
        let mut ids_to_rebase: Vec<git2::Oid> = project_repository
            .l(branch.head, LogUntil::Commit(target_oid))?
            .iter()
            .filter(|id| **id != commit_oid)
            .cloned()
            .collect();

        ids_to_rebase.push(commit_oid);

        let new_head = cherry_rebase_group(project_repository, target_oid, &mut ids_to_rebase)
            .context("rebase failed")?;

        branch.head = new_head;
        branch.updated_timestamp_ms = gitbutler_time::time::now_ms();
        vb_state.set_branch(branch.clone())?;

        crate::integration::update_gitbutler_integration(&vb_state, project_repository)
            .context("failed to update gitbutler integration")?;
    }

    Ok(())
}

// create and insert a blank commit (no tree change) either above or below a commit
// if offset is positive, insert below, if negative, insert above
// return the oid of the new head commit of the branch with the inserted blank commit
pub(crate) fn insert_blank_commit(
    project_repository: &ProjectRepository,
    branch_id: BranchId,
    commit_oid: git2::Oid,
    offset: i32,
) -> Result<()> {
    let vb_state = project_repository.project().virtual_branches();

    let mut branch = vb_state.get_branch_in_workspace(branch_id)?;
    // find the commit to offset from
    let mut commit = project_repository
        .repo()
        .find_commit(commit_oid)
        .context("failed to find commit")?;

    if offset > 0 {
        commit = commit.parent(0).context("failed to find parent")?;
    }

    let commit_tree = commit.tree().unwrap();
    let blank_commit_oid = project_repository.commit("", &commit_tree, &[&commit], None)?;

    if commit.id() == branch.head && offset < 0 {
        // inserting before the first commit
        branch.head = blank_commit_oid;
        crate::integration::update_gitbutler_integration(&vb_state, project_repository)
            .context("failed to update gitbutler integration")?;
    } else {
        // rebase all commits above it onto the new commit
        match cherry_rebase(
            project_repository,
            blank_commit_oid,
            commit.id(),
            branch.head,
        ) {
            Ok(Some(new_head)) => {
                branch.head = new_head;
                crate::integration::update_gitbutler_integration(&vb_state, project_repository)
                    .context("failed to update gitbutler integration")?;
            }
            Ok(None) => bail!("no rebase happened"),
            Err(err) => {
                return Err(err).context("rebase failed");
            }
        }
    }
    branch.updated_timestamp_ms = gitbutler_time::time::now_ms();
    vb_state.set_branch(branch.clone())?;

    Ok(())
}

// remove a commit in a branch by rebasing all commits _except_ for it onto it's parent
// if successful, it will update the branch head to the new head commit
pub(crate) fn undo_commit(
    project_repository: &ProjectRepository,
    branch_id: BranchId,
    commit_oid: git2::Oid,
) -> Result<()> {
    let vb_state = project_repository.project().virtual_branches();

    let mut branch = vb_state.get_branch_in_workspace(branch_id)?;
    let commit = project_repository
        .repo()
        .find_commit(commit_oid)
        .context("failed to find commit")?;

    let new_commit_oid;

    if branch.head == commit_oid {
        // if commit is the head, just set head to the parent
        new_commit_oid = commit.parent(0).context("failed to find parent")?.id();
    } else {
        // if commit is not the head, rebase all commits above it onto it's parent
        let parent_commit_oid = commit.parent(0).context("failed to find parent")?.id();

        match cherry_rebase(
            project_repository,
            parent_commit_oid,
            commit_oid,
            branch.head,
        ) {
            Ok(Some(new_head)) => {
                new_commit_oid = new_head;
            }
            Ok(None) => bail!("no rebase happened"),
            Err(err) => {
                return Err(err).context("rebase failed");
            }
        }
    }

    if new_commit_oid != commit_oid {
        branch.head = new_commit_oid;
        branch.updated_timestamp_ms = gitbutler_time::time::now_ms();
        vb_state.set_branch(branch.clone())?;

        crate::integration::update_gitbutler_integration(&vb_state, project_repository)
            .context("failed to update gitbutler integration")?;
    }

    Ok(())
}

/// squashes a commit from a virtual branch into its parent.
pub(crate) fn squash(
    project_repository: &ProjectRepository,
    branch_id: BranchId,
    commit_id: git2::Oid,
) -> Result<()> {
    project_repository.assure_resolved()?;

    let vb_state = project_repository.project().virtual_branches();
    let mut branch = vb_state.get_branch_in_workspace(branch_id)?;
    let default_target = vb_state.get_default_target()?;
    let branch_commit_oids =
        project_repository.l(branch.head, LogUntil::Commit(default_target.sha))?;

    if !branch_commit_oids.contains(&commit_id) {
        bail!("commit {commit_id} not in the branch")
    }

    let commit_to_squash = project_repository
        .repo()
        .find_commit(commit_id)
        .context("failed to find commit")?;

    let parent_commit = commit_to_squash
        .parent(0)
        .context("failed to find parent commit")?;

    let pushed_commit_oids = branch.upstream_head.map_or_else(
        || Ok(vec![]),
        |upstream_head| project_repository.l(upstream_head, LogUntil::Commit(default_target.sha)),
    )?;

    if pushed_commit_oids.contains(&parent_commit.id()) && !branch.allow_rebasing {
        // squashing into a pushed commit will cause a force push that is not allowed
        bail!("force push not allowed");
    }

    if !branch_commit_oids.contains(&parent_commit.id()) {
        bail!("can not squash root commit");
    }

    // create a commit that:
    //  * has the tree of the target commit
    //  * has the message combined of the target commit and parent commit
    //  * has parents of the parents commit.
    let parents: Vec<_> = parent_commit.parents().collect();

    let new_commit_oid = project_repository
        .repo()
        .commit_with_signature(
            None,
            &commit_to_squash.author(),
            &commit_to_squash.committer(),
            &format!(
                "{}\n{}",
                parent_commit.message_bstr(),
                commit_to_squash.message_bstr(),
            ),
            &commit_to_squash.tree().context("failed to find tree")?,
            &parents.iter().collect::<Vec<_>>(),
            // use the squash commit's headers
            commit_to_squash.gitbutler_headers(),
        )
        .context("failed to commit")?;

    let ids_to_rebase = {
        let ids = branch_commit_oids
            .split(|oid| oid.eq(&commit_id))
            .collect::<Vec<_>>();
        ids.first().copied()
    }
    .with_context(|| format!("commit {commit_id} not in the branch"))?;
    let mut ids_to_rebase = ids_to_rebase.to_vec();

    match cherry_rebase_group(project_repository, new_commit_oid, &mut ids_to_rebase) {
        Ok(new_head_id) => {
            // save new branch head
            branch.head = new_head_id;
            branch.updated_timestamp_ms = gitbutler_time::time::now_ms();
            vb_state.set_branch(branch.clone())?;

            crate::integration::update_gitbutler_integration(&vb_state, project_repository)
                .context("failed to update gitbutler integration")?;
            Ok(())
        }
        Err(err) => Err(err.context("rebase error").context(Code::Unknown)),
    }
}

// changes a commit message for commit_oid, rebases everything above it, updates branch head if successful
pub(crate) fn update_commit_message(
    project_repository: &ProjectRepository,
    branch_id: BranchId,
    commit_id: git2::Oid,
    message: &str,
) -> Result<()> {
    if message.is_empty() {
        bail!("commit message can not be empty");
    }
    project_repository.assure_unconflicted()?;

    let vb_state = project_repository.project().virtual_branches();
    let default_target = vb_state.get_default_target()?;

    let mut branch = vb_state.get_branch_in_workspace(branch_id)?;
    let branch_commit_oids =
        project_repository.l(branch.head, LogUntil::Commit(default_target.sha))?;

    if !branch_commit_oids.contains(&commit_id) {
        bail!("commit {commit_id} not in the branch");
    }

    let pushed_commit_oids = branch.upstream_head.map_or_else(
        || Ok(vec![]),
        |upstream_head| project_repository.l(upstream_head, LogUntil::Commit(default_target.sha)),
    )?;

    if pushed_commit_oids.contains(&commit_id) && !branch.allow_rebasing {
        // updating the message of a pushed commit will cause a force push that is not allowed
        bail!("force push not allowed");
    }

    let target_commit = project_repository
        .repo()
        .find_commit(commit_id)
        .context("failed to find commit")?;

    let parents: Vec<_> = target_commit.parents().collect();

    let new_commit_oid = project_repository
        .repo()
        .commit_with_signature(
            None,
            &target_commit.author(),
            &target_commit.committer(),
            message,
            &target_commit.tree().context("failed to find tree")?,
            &parents.iter().collect::<Vec<_>>(),
            target_commit.gitbutler_headers(),
        )
        .context("failed to commit")?;

    let ids_to_rebase = {
        let ids = branch_commit_oids
            .split(|oid| oid.eq(&commit_id))
            .collect::<Vec<_>>();
        ids.first().copied()
    }
    .with_context(|| format!("commit {commit_id} not in the branch"))?;
    let mut ids_to_rebase = ids_to_rebase.to_vec();

    let new_head_id = cherry_rebase_group(project_repository, new_commit_oid, &mut ids_to_rebase)
        .map_err(|err| err.context("rebase error"))?;
    // save new branch head
    branch.head = new_head_id;
    branch.updated_timestamp_ms = gitbutler_time::time::now_ms();
    vb_state.set_branch(branch.clone())?;

    crate::integration::update_gitbutler_integration(&vb_state, project_repository)
        .context("failed to update gitbutler integration")?;
    Ok(())
}

/// moves commit from the branch it's in to the top of the target branch
pub(crate) fn move_commit(
    project_repository: &ProjectRepository,
    target_branch_id: BranchId,
    commit_id: git2::Oid,
) -> Result<()> {
    project_repository.assure_resolved()?;
    let vb_state = project_repository.project().virtual_branches();

    let applied_branches = vb_state
        .list_branches_in_workspace()
        .context("failed to read virtual branches")?;

    if !applied_branches.iter().any(|b| b.id == target_branch_id) {
        bail!("branch {target_branch_id} is not among applied branches")
    }

    let mut applied_statuses = get_applied_status(project_repository, None)?.branches;

    let (ref mut source_branch, source_status) = applied_statuses
        .iter_mut()
        .find(|(b, _)| b.head == commit_id)
        .ok_or_else(|| anyhow!("commit {commit_id} to be moved could not be found"))?;

    let source_branch_non_comitted_files = source_status;

    let source_branch_head = project_repository
        .repo()
        .find_commit(commit_id)
        .context("failed to find commit")?;
    let source_branch_head_parent = source_branch_head
        .parent(0)
        .context("failed to get parent commit")?;
    let source_branch_head_tree = source_branch_head
        .tree()
        .context("failed to get commit tree")?;
    let source_branch_head_parent_tree = source_branch_head_parent
        .tree()
        .context("failed to get parent tree")?;
    let branch_head_diff = gitbutler_diff::trees(
        project_repository.repo(),
        &source_branch_head_parent_tree,
        &source_branch_head_tree,
    )?;

    let branch_head_diff: HashMap<_, _> =
        gitbutler_diff::diff_files_into_hunks(branch_head_diff).collect();
    let is_source_locked = source_branch_non_comitted_files
        .iter()
        .any(|(path, hunks)| {
            branch_head_diff.get(path).map_or(false, |head_diff_hunks| {
                hunks.iter().any(|hunk| {
                    let hunk: GitHunk = hunk.clone().into();
                    head_diff_hunks.iter().any(|head_hunk| {
                        joined(
                            head_hunk.new_start,
                            head_hunk.new_start + head_hunk.new_lines,
                            hunk.new_start,
                            hunk.new_start + hunk.new_lines,
                        )
                    })
                })
            })
        });

    if is_source_locked {
        bail!("the source branch contains hunks locked to the target commit")
    }

    // move files ownerships from source branch to the destination branch

    let ownerships_to_transfer = branch_head_diff
        .iter()
        .map(|(file_path, hunks)| {
            (
                file_path.clone(),
                hunks.iter().map(Into::into).collect::<Vec<_>>(),
            )
        })
        .map(|(file_path, hunks)| OwnershipClaim { file_path, hunks })
        .flat_map(|file_ownership| source_branch.ownership.take(&file_ownership))
        .collect::<Vec<_>>();

    // reset the source branch to the parent commit
    {
        source_branch.head = source_branch_head_parent.id();
        vb_state.set_branch(source_branch.clone())?;
    }

    // move the commit to destination branch target branch
    {
        let mut destination_branch = vb_state.get_branch_in_workspace(target_branch_id)?;

        for ownership in ownerships_to_transfer {
            destination_branch.ownership.put(ownership);
        }

        let new_destination_tree_oid = write_tree_onto_commit(
            project_repository,
            destination_branch.head,
            branch_head_diff,
        )
        .context("failed to write tree onto commit")?;
        let new_destination_tree = project_repository
            .repo()
            .find_tree(new_destination_tree_oid)
            .context("failed to find tree")?;

        let new_destination_head_oid = project_repository
            .commit(
                &source_branch_head.message_bstr().to_str_lossy(),
                &new_destination_tree,
                &[&project_repository
                    .repo()
                    .find_commit(destination_branch.head)
                    .context("failed to get dst branch head commit")?],
                source_branch_head.gitbutler_headers(),
            )
            .context("failed to commit")?;

        destination_branch.head = new_destination_head_oid;
        vb_state.set_branch(destination_branch.clone())?;
    }

    crate::integration::update_gitbutler_integration(&vb_state, project_repository)
        .context("failed to update gitbutler integration")?;

    Ok(())
}

/// Just like [`diffy::apply()`], but on error it will attach hashes of the input `base_image` and `patch`.
pub(crate) fn apply<S: AsRef<[u8]>>(base_image: S, patch: &Patch<'_, [u8]>) -> Result<BString> {
    fn md5_hash_hex(b: impl AsRef<[u8]>) -> String {
        md5::compute(b).encode_hex()
    }

    #[derive(Debug)]
    #[allow(dead_code)] // Read by Debug auto-impl, which doesn't count
    pub enum DebugLine {
        // Note that each of these strings is a hash only
        Context(String),
        Delete(String),
        Insert(String),
    }

    impl<'a> From<&diffy::Line<'a, [u8]>> for DebugLine {
        fn from(line: &Line<'a, [u8]>) -> Self {
            match line {
                Line::Context(s) => DebugLine::Context(md5_hash_hex(s)),
                Line::Delete(s) => DebugLine::Delete(md5_hash_hex(s)),
                Line::Insert(s) => DebugLine::Insert(md5_hash_hex(s)),
            }
        }
    }

    #[derive(Debug)]
    #[allow(dead_code)] // Read by Debug auto-impl, which doesn't count
    struct DebugHunk {
        old_range: diffy::HunkRange,
        new_range: diffy::HunkRange,
        lines: Vec<DebugLine>,
    }

    impl<'a> From<&diffy::Hunk<'a, [u8]>> for DebugHunk {
        fn from(hunk: &diffy::Hunk<'a, [u8]>) -> Self {
            Self {
                old_range: hunk.old_range(),
                new_range: hunk.new_range(),
                lines: hunk.lines().iter().map(Into::into).collect(),
            }
        }
    }

    #[derive(Debug)]
    #[allow(dead_code)] // Read by Debug auto-impl, which doesn't count
    struct DebugContext {
        base_image_hash: String,
        hunks: Vec<DebugHunk>,
    }

    impl std::fmt::Display for DebugContext {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            std::fmt::Debug::fmt(self, f)
        }
    }

    diffy_apply(base_image.as_ref(), patch)
        .with_context(|| DebugContext {
            base_image_hash: md5_hash_hex(base_image),
            hunks: patch.hunks().iter().map(Into::into).collect(),
        })
        .map(Into::into)
}

// Goes through a set of changes and checks if conflicts are present. If no conflicts
// are present in a file it will be resolved, meaning it will be removed from the
// conflicts file.
fn update_conflict_markers(
    project_repository: &ProjectRepository,
    files: &HashMap<PathBuf, Vec<impl Into<GitHunk> + Clone>>,
) -> Result<()> {
    let conflicting_files = conflicts::conflicting_files(project_repository)?;
    for (file_path, non_commited_hunks) in files {
        let mut conflicted = false;
        if conflicting_files.contains(file_path) {
            // check file for conflict markers, resolve the file if there are none in any hunk
            for hunk in non_commited_hunks {
                let hunk: GitHunk = hunk.clone().into();
                if hunk.diff_lines.contains_str(b"<<<<<<< ours") {
                    conflicted = true;
                }
                if hunk.diff_lines.contains_str(b">>>>>>> theirs") {
                    conflicted = true;
                }
            }
            if !conflicted {
                conflicts::resolve(project_repository, file_path).unwrap();
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn joined_test() {
        assert!(!joined(1, 2, 3, 4));
        assert!(joined(1, 4, 2, 3));
        assert!(joined(2, 3, 1, 4));
        assert!(!joined(3, 4, 1, 2));

        assert!(joined(1, 2, 2, 3));
        assert!(joined(1, 3, 2, 3));
        assert!(joined(2, 3, 1, 2));

        assert!(!joined(1, 1, 2, 2));
        assert!(joined(1, 1, 1, 1));
        assert!(joined(1, 1, 1, 2));
        assert!(joined(1, 2, 2, 2));
    }

    #[test]
    fn normalize_branch_name_test() {
        assert_eq!(normalize_branch_name("feature/branch"), "feature/branch");
        assert_eq!(normalize_branch_name("foo#branch"), "foo#branch");
        assert_eq!(normalize_branch_name("foo!branch"), "foo!branch");
    }
}
