use std::borrow::Borrow;
#[cfg(target_family = "unix")]
use std::os::unix::prelude::PermissionsExt;
use std::time::SystemTime;
use std::{
    collections::HashMap,
    hash::Hash,
    path::{Path, PathBuf},
    time, vec,
};

use anyhow::{anyhow, bail, Context, Result};
use bstr::{BString, ByteSlice, ByteVec};
use diffy::{apply_bytes as diffy_apply, Line, Patch};
use git2_hooks::HookResult;
use hex::ToHex;
use regex::Regex;
use serde::Serialize;

use super::integration::get_workspace_head;
use super::{
    branch::{
        self, Branch, BranchCreateRequest, BranchId, BranchOwnershipClaims, Hunk, OwnershipClaim,
    },
    branch_to_remote_branch, errors, target, RemoteBranch, VirtualBranchesHandle,
};
use crate::git::diff::{diff_files_into_hunks, trees, FileDiff};
use crate::snapshots::snapshoter::Snapshoter;
use crate::virtual_branches::branch::HunkHash;
use crate::{
    dedup::{dedup, dedup_fmt},
    git::{
        self,
        diff::{self},
        Commit, Refname, RemoteRefname,
    },
    keys,
    project_repository::{self, conflicts, LogUntil},
    reader, users,
};
use crate::{error::Error, git::diff::GitHunk};

type AppliedStatuses = Vec<(branch::Branch, BranchStatus)>;

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
    pub head: git::Oid,
}

#[derive(Debug, PartialEq, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualBranches {
    pub branches: Vec<VirtualBranch>,
    pub skipped_files: Vec<git::diff::FileDiff>,
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
    pub id: git::Oid,
    #[serde(serialize_with = "crate::serde::as_string_lossy")]
    pub description: BString,
    pub created_at: u128,
    pub author: Author,
    pub is_remote: bool,
    pub files: Vec<VirtualBranchFile>,
    pub is_integrated: bool,
    pub parent_ids: Vec<git::Oid>,
    pub branch_id: BranchId,
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
    #[serde(serialize_with = "crate::serde::as_string_lossy")]
    pub diff: BString,
    pub modified_at: u128,
    pub file_path: PathBuf,
    #[serde(serialize_with = "crate::serde::hash_to_hex")]
    pub hash: HunkHash,
    pub old_start: u32,
    pub start: u32,
    pub end: u32,
    pub binary: bool,
    pub locked: bool,
    pub locked_to: Option<Box<[diff::HunkLock]>>,
    pub change_type: diff::ChangeType,
}

/// Lifecycle
impl VirtualBranchHunk {
    pub(crate) fn gen_id(new_start: u32, new_lines: u32) -> String {
        format!("{}-{}", new_start, new_start + new_lines)
    }
    fn from_git_hunk(
        project_path: &Path,
        file_path: PathBuf,
        hunk: GitHunk,
        mtimes: &mut MTimeCache,
    ) -> Self {
        let hash = Hunk::hash_diff(&hunk.diff_lines);
        Self {
            id: Self::gen_id(hunk.new_start, hunk.new_lines),
            modified_at: mtimes.mtime_by_path(project_path.join(&file_path)),
            file_path,
            diff: hunk.diff_lines,
            old_start: hunk.old_start,
            start: hunk.new_start,
            end: hunk.new_start + hunk.new_lines,
            binary: hunk.binary,
            hash,
            locked: hunk.locked_to.len() > 0,
            locked_to: Some(hunk.locked_to),
            change_type: hunk.change_type,
        }
    }
}

#[derive(Debug, Serialize, Hash, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Author {
    pub name: String,
    pub email: String,
    pub gravatar_url: url::Url,
}

impl From<git::Signature<'_>> for Author {
    fn from(value: git::Signature) -> Self {
        let name = value.name().unwrap_or_default().to_string();
        let email = value.email().unwrap_or_default().to_string();

        let gravatar_url = url::Url::parse(&format!(
            "https://www.gravatar.com/avatar/{:x}?s=100&r=g&d=retro",
            md5::compute(email.to_lowercase())
        ))
        .unwrap();

        Author {
            name,
            email,
            gravatar_url,
        }
    }
}

pub fn normalize_branch_name(name: &str) -> String {
    let pattern = Regex::new("[^A-Za-z0-9_/.#]+").unwrap();
    pattern.replace_all(name, "-").to_string()
}

pub fn apply_branch(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    signing_key: Option<&keys::PrivateKey>,
    user: Option<&users::User>,
) -> Result<(), errors::ApplyBranchError> {
    if project_repository.is_resolving() {
        return Err(errors::ApplyBranchError::Conflict(
            errors::ProjectConflict {
                project_id: project_repository.project().id,
            },
        ));
    }
    let repo = &project_repository.git_repository;

    let vb_state = project_repository.project().virtual_branches();
    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to get default target")?
    else {
        return Err(errors::ApplyBranchError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let mut branch = match vb_state.get_branch(branch_id) {
        Ok(branch) => Ok(branch),
        Err(reader::Error::NotFound) => Err(errors::ApplyBranchError::BranchNotFound(
            errors::BranchNotFound {
                project_id: project_repository.project().id,
                branch_id: *branch_id,
            },
        )),
        Err(error) => Err(errors::ApplyBranchError::Other(error.into())),
    }?;

    if branch.applied {
        return Ok(());
    }

    let target_commit = repo
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;
    let target_tree = target_commit.tree().context("failed to get target tree")?;

    // calculate the merge base and make sure it's the same as the target commit
    // if not, we need to merge or rebase the branch to get it up to date

    let merge_base = repo
        .merge_base(default_target.sha, branch.head)
        .context(format!(
            "failed to find merge base between {} and {}",
            default_target.sha, branch.head
        ))?;
    if merge_base != default_target.sha {
        // Branch is out of date, merge or rebase it
        let merge_base_tree = repo
            .find_commit(merge_base)
            .context(format!("failed to find merge base commit {}", merge_base))?
            .tree()
            .context("failed to find merge base tree")?;

        let branch_tree = repo
            .find_tree(branch.tree)
            .context("failed to find branch tree")?;

        let mut merge_index = repo
            .merge_trees(&merge_base_tree, &branch_tree, &target_tree)
            .context("failed to merge trees")?;

        if merge_index.has_conflicts() {
            // currently we can only deal with the merge problem branch
            for mut branch in
                super::get_status_by_branch(project_repository, Some(&target_commit.id()))?
                    .0
                    .into_iter()
                    .map(|(branch, _)| branch)
                    .filter(|branch| branch.applied)
            {
                branch.applied = false;
                vb_state.set_branch(branch)?;
            }

            // apply the branch
            branch.applied = true;
            vb_state.set_branch(branch)?;

            // checkout the conflicts
            repo.checkout_index(&mut merge_index)
                .allow_conflicts()
                .conflict_style_merge()
                .force()
                .checkout()
                .context("failed to checkout index")?;

            // mark conflicts
            let conflicts = merge_index
                .conflicts()
                .context("failed to get merge index conflicts")?;
            let mut merge_conflicts = Vec::new();
            for path in conflicts.flatten() {
                if let Some(ours) = path.our {
                    let path = std::str::from_utf8(&ours.path)
                        .context("failed to convert path to utf8")?
                        .to_string();
                    merge_conflicts.push(path);
                }
            }
            conflicts::mark(
                project_repository,
                &merge_conflicts,
                Some(default_target.sha),
            )?;

            return Ok(());
        }

        let head_commit = repo
            .find_commit(branch.head)
            .context("failed to find head commit")?;

        let merged_branch_tree_oid = merge_index
            .write_tree_to(repo)
            .context("failed to write tree")?;

        let merged_branch_tree = repo
            .find_tree(merged_branch_tree_oid)
            .context("failed to find tree")?;

        let ok_with_force_push = project_repository.project().ok_with_force_push;
        if branch.upstream.is_some() && !ok_with_force_push {
            // branch was pushed to upstream, and user doesn't like force pushing.
            // create a merge commit to avoid the need of force pushing then.

            let new_branch_head = project_repository.commit(
                user,
                format!(
                    "Merged {}/{} into {}",
                    default_target.branch.remote(),
                    default_target.branch.branch(),
                    branch.name
                )
                .as_str(),
                &merged_branch_tree,
                &[&head_commit, &target_commit],
                signing_key,
            )?;

            // ok, update the virtual branch
            branch.head = new_branch_head;
            branch.tree = merged_branch_tree_oid;
            vb_state.set_branch(branch.clone())?;
        } else {
            // branch was not pushed to upstream yet. attempt a rebase,
            let (_, committer) = project_repository.git_signatures(user)?;
            let mut rebase_options = git2::RebaseOptions::new();
            rebase_options.quiet(true);
            rebase_options.inmemory(true);
            let mut rebase = repo
                .rebase(
                    Some(branch.head),
                    Some(target_commit.id()),
                    None,
                    Some(&mut rebase_options),
                )
                .context("failed to rebase")?;

            let mut rebase_success = true;
            // check to see if these commits have already been pushed
            let mut last_rebase_head = branch.head;
            while rebase.next().is_some() {
                let index = rebase
                    .inmemory_index()
                    .context("failed to get inmemory index")?;
                if index.has_conflicts() {
                    rebase_success = false;
                    break;
                }

                if let Ok(commit_id) = rebase.commit(None, &committer.clone().into(), None) {
                    last_rebase_head = commit_id.into();
                } else {
                    rebase_success = false;
                    break;
                }
            }

            if rebase_success {
                // rebase worked out, rewrite the branch head
                rebase.finish(None).context("failed to finish rebase")?;
                branch.head = last_rebase_head;
                branch.tree = merged_branch_tree_oid;
            } else {
                // rebase failed, do a merge commit
                rebase.abort().context("failed to abort rebase")?;

                // get tree from merge_tree_oid
                let merge_tree = repo
                    .find_tree(merged_branch_tree_oid)
                    .context("failed to find tree")?;

                // commit the merge tree oid
                let new_branch_head = project_repository
                    .commit(
                        user,
                        format!(
                            "Merged {}/{} into {}",
                            default_target.branch.remote(),
                            default_target.branch.branch(),
                            branch.name
                        )
                        .as_str(),
                        &merge_tree,
                        &[&head_commit, &target_commit],
                        signing_key,
                    )
                    .context("failed to commit merge")?;

                branch.head = new_branch_head;
                branch.tree = merged_branch_tree_oid;
            }
        }
    }

    let wd_tree = project_repository.get_wd_tree()?;

    let branch_tree = repo
        .find_tree(branch.tree)
        .context("failed to find branch tree")?;

    // check index for conflicts
    let mut merge_index = repo
        .merge_trees(&target_tree, &wd_tree, &branch_tree)
        .context("failed to merge trees")?;

    if merge_index.has_conflicts() {
        return Err(errors::ApplyBranchError::BranchConflicts(*branch_id));
    }

    // apply the branch
    branch.applied = true;
    vb_state.set_branch(branch)?;

    ensure_selected_for_changes(&vb_state).context("failed to ensure selected for changes")?;

    // checkout the merge index
    repo.checkout_index(&mut merge_index)
        .force()
        .checkout()
        .context("failed to checkout index")?;

    super::integration::update_gitbutler_integration(&vb_state, project_repository)?;

    Ok(())
}

pub fn unapply_ownership(
    project_repository: &project_repository::Repository,
    ownership: &BranchOwnershipClaims,
) -> Result<(), errors::UnapplyOwnershipError> {
    if conflicts::is_resolving(project_repository) {
        return Err(errors::UnapplyOwnershipError::Conflict(
            errors::ProjectConflict {
                project_id: project_repository.project().id,
            },
        ));
    }

    let vb_state = project_repository.project().virtual_branches();

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to get default target")?
    else {
        return Err(errors::UnapplyOwnershipError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let applied_branches = vb_state
        .list_branches()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|b| b.applied)
        .collect::<Vec<_>>();

    let integration_commit_id =
        super::integration::get_workspace_head(&vb_state, project_repository)?;

    let (applied_statuses, _) = get_applied_status(
        project_repository,
        &integration_commit_id,
        &default_target.sha,
        applied_branches,
    )
    .context("failed to get status by branch")?;

    let hunks_to_unapply = applied_statuses
        .iter()
        .map(
            |(_branch, branch_files)| -> Result<Vec<(PathBuf, &diff::GitHunk)>> {
                let mut hunks_to_unapply = Vec::new();
                for (path, hunks) in branch_files {
                    let ownership_hunks: Vec<&Hunk> = ownership
                        .claims
                        .iter()
                        .filter(|o| o.file_path == *path)
                        .flat_map(|f| &f.hunks)
                        .collect();
                    for hunk in hunks {
                        if ownership_hunks.contains(&&Hunk::from(hunk)) {
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
        if let Some(reversed_hunk) = diff::reverse_hunk(h.1) {
            diff.entry(h.0).or_insert_with(Vec::new).push(reversed_hunk);
        } else {
            return Err(errors::UnapplyOwnershipError::Other(anyhow::anyhow!(
                "failed to reverse hunk"
            )));
        }
    }

    let repo = &project_repository.git_repository;

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
            let mut result = repo.merge_trees(&base_tree, &final_tree, &branch_tree)?;
            let final_tree_oid = result.write_tree_to(repo)?;
            repo.find_tree(final_tree_oid)
                .context("failed to find tree")
        },
    )?;

    let final_tree_oid = write_tree_onto_tree(project_repository, &final_tree, diff)?;
    let final_tree = repo
        .find_tree(final_tree_oid)
        .context("failed to find tree")?;

    repo.checkout_tree(&final_tree)
        .force()
        .remove_untracked()
        .checkout()
        .context("failed to checkout tree")?;

    super::integration::update_gitbutler_integration(&vb_state, project_repository)?;

    Ok(())
}

// reset a file in the project to the index state
pub fn reset_files(
    project_repository: &project_repository::Repository,
    files: &Vec<String>,
) -> Result<(), errors::UnapplyOwnershipError> {
    if conflicts::is_resolving(project_repository) {
        return Err(errors::UnapplyOwnershipError::Conflict(
            errors::ProjectConflict {
                project_id: project_repository.project().id,
            },
        ));
    }

    // for each tree, we need to checkout the entry from the index at that path
    // or if it doesn't exist, remove the file from the working directory
    let repo = &project_repository.git_repository;
    let index = repo.index().context("failed to get index")?;
    for file in files {
        let entry = index.get_path(Path::new(file), 0);
        if entry.is_some() {
            repo.checkout_index_path(Path::new(file))
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

// to unapply a branch, we need to write the current tree out, then remove those file changes from the wd
pub fn unapply_branch(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
) -> Result<Option<branch::Branch>, errors::UnapplyBranchError> {
    let vb_state = project_repository.project().virtual_branches();

    let mut target_branch = vb_state
        .get_branch(branch_id)
        .map_err(|error| match error {
            reader::Error::NotFound => {
                errors::UnapplyBranchError::BranchNotFound(errors::BranchNotFound {
                    project_id: project_repository.project().id,
                    branch_id: *branch_id,
                })
            }
            error => errors::UnapplyBranchError::Other(error.into()),
        })?;

    if !target_branch.applied {
        return Ok(Some(target_branch));
    }

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to get default target")?
    else {
        return Err(errors::UnapplyBranchError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let repo = &project_repository.git_repository;
    let target_commit = repo
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;

    let final_tree = if conflicts::is_resolving(project_repository) {
        {
            target_branch.applied = false;
            target_branch.selected_for_changes = None;
            vb_state.set_branch(target_branch.clone())?;
        }
        conflicts::clear(project_repository).context("failed to clear conflicts")?;
        target_commit.tree().context("failed to get target tree")?
    } else {
        // if we are not resolving, we need to merge the rest of the applied branches
        let applied_branches = vb_state
            .list_branches()
            .context("failed to read virtual branches")?
            .into_iter()
            .filter(|b| b.applied)
            .collect::<Vec<_>>();

        let integration_commit =
            super::integration::update_gitbutler_integration(&vb_state, project_repository)?;

        let (applied_statuses, _) = get_applied_status(
            project_repository,
            &integration_commit,
            &default_target.sha,
            applied_branches,
        )
        .context("failed to get status by branch")?;

        let status = applied_statuses
            .iter()
            .find(|(s, _)| s.id == target_branch.id)
            .context("failed to find status for branch");

        if let Ok((branch, files)) = status {
            update_conflict_markers(project_repository, files)?;
            if files.is_empty() && branch.head == default_target.sha {
                // if there is nothing to unapply, remove the branch straight away
                vb_state
                    .remove_branch(target_branch.id)
                    .context("Failed to remove branch")?;

                ensure_selected_for_changes(&vb_state)
                    .context("failed to ensure selected for changes")?;

                project_repository.delete_branch_reference(&target_branch)?;
                return Ok(None);
            }

            target_branch.tree = write_tree(project_repository, &target_branch.head, files)?;
            target_branch.applied = false;
            target_branch.selected_for_changes = None;
            vb_state.set_branch(target_branch.clone())?;
        }

        let target_commit = repo
            .find_commit(default_target.sha)
            .context("failed to find target commit")?;

        // ok, update the wd with the union of the rest of the branches
        let base_tree = target_commit.tree().context("failed to get target tree")?;

        // go through the other applied branches and merge them into the final tree
        // then check that out into the working directory
        let final_tree = applied_statuses
            .into_iter()
            .filter(|(branch, _)| &branch.id != branch_id)
            .fold(
                target_commit.tree().context("failed to get target tree"),
                |final_tree, status| {
                    let final_tree = final_tree?;
                    let branch = status.0;
                    let tree_oid = write_tree(project_repository, &branch.head, status.1)?;
                    let branch_tree = repo.find_tree(tree_oid)?;
                    let mut result = repo.merge_trees(&base_tree, &final_tree, &branch_tree)?;
                    let final_tree_oid = result.write_tree_to(repo)?;
                    repo.find_tree(final_tree_oid)
                        .context("failed to find tree")
                },
            )?;

        ensure_selected_for_changes(&vb_state).context("failed to ensure selected for changes")?;

        final_tree
    };

    // checkout final_tree into the working directory
    repo.checkout_tree(&final_tree)
        .force()
        .remove_untracked()
        .checkout()
        .context("failed to checkout tree")?;

    super::integration::update_gitbutler_integration(&vb_state, project_repository)?;

    Ok(Some(target_branch))
}

fn find_base_tree<'a>(
    repo: &'a git::Repository,
    branch_commit: &'a git::Commit<'a>,
    target_commit: &'a git::Commit<'a>,
) -> Result<git::Tree<'a>> {
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

pub fn list_virtual_branches(
    project_repository: &project_repository::Repository,
) -> Result<(Vec<VirtualBranch>, Vec<diff::FileDiff>), errors::ListVirtualBranchesError> {
    let mut branches: Vec<VirtualBranch> = Vec::new();

    let vb_state = project_repository.project().virtual_branches();
    let default_target = vb_state
        .get_default_target()
        .context("failed to get default target")?;

    let integration_commit_id =
        super::integration::get_workspace_head(&vb_state, project_repository)?;
    let integration_commit = project_repository
        .git_repository
        .find_commit(integration_commit_id)
        .unwrap();

    let (statuses, skipped_files) =
        get_status_by_branch(project_repository, Some(&integration_commit.id()))?;
    let max_selected_for_changes = statuses
        .iter()
        .filter_map(|(branch, _)| branch.selected_for_changes)
        .max()
        .unwrap_or(-1);

    for (branch, files) in statuses {
        let repo = &project_repository.git_repository;
        update_conflict_markers(project_repository, &files)?;

        let upstream_branch = match branch
            .upstream
            .as_ref()
            .map(|name| repo.find_branch(&git::Refname::from(name)))
            .transpose()
        {
            Err(git::Error::NotFound(_)) => Ok(None),
            Err(error) => Err(error),
            Ok(branch) => Ok(branch),
        }
        .context(format!(
            "failed to find upstream branch for {}",
            branch.name
        ))?;

        let upstram_branch_commit = upstream_branch
            .as_ref()
            .map(git::Branch::peel_to_commit)
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
            for oid in project_repository.l(upstream.id(), LogUntil::Commit(merge_base))? {
                pushed_commits.insert(oid, true);
            }
        }

        let mut is_integrated = false;
        let mut is_remote = false;

        // find all commits on head that are not on target.sha
        let commits = project_repository
            .log(branch.head, LogUntil::Commit(default_target.sha))
            .context(format!("failed to get log for branch {}", branch.name))?
            .iter()
            .map(|commit| {
                is_remote = if is_remote {
                    is_remote
                } else {
                    pushed_commits.contains_key(&commit.id())
                };

                // only check for integration if we haven't already found an integration
                is_integrated = if is_integrated {
                    is_integrated
                } else {
                    is_commit_integrated(project_repository, &default_target, commit)?
                };

                commit_to_vbranch_commit(
                    project_repository,
                    &branch,
                    commit,
                    is_integrated,
                    is_remote,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        // if the branch is not applied, check to see if it's mergeable and up to date
        let mut base_current = true;
        if !branch.applied {
            // determine if this branch is up to date with the target/base
            let merge_base = repo
                .merge_base(default_target.sha, branch.head)
                .context("failed to find merge base")?;
            if merge_base != default_target.sha {
                base_current = false;
            }
        }

        let upstream = upstream_branch
            .map(|upstream_branch| branch_to_remote_branch(&upstream_branch))
            .transpose()?
            .flatten();

        let mut files = diffs_into_virtual_files(project_repository, files);

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

        let requires_force = is_requires_force(project_repository, &branch)?;

        let branch = VirtualBranch {
            id: branch.id,
            name: branch.name,
            notes: branch.notes,
            active: branch.applied,
            files,
            order: branch.order,
            commits,
            requires_force,
            upstream,
            upstream_name: branch
                .upstream
                .and_then(|r| Refname::from(r).branch().map(Into::into)),
            conflicted: conflicts::is_resolving(project_repository),
            base_current,
            ownership: branch.ownership,
            updated_at: branch.updated_timestamp_ms,
            selected_for_changes: branch.selected_for_changes == Some(max_selected_for_changes),
            head: branch.head,
        };
        branches.push(branch);
    }

    let mut branches = branches_with_large_files_abridged(branches);
    branches.sort_by(|a, b| a.order.cmp(&b.order));

    Ok((branches, skipped_files))
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

fn is_requires_force(
    project_repository: &project_repository::Repository,
    branch: &branch::Branch,
) -> Result<bool> {
    let upstream = if let Some(upstream) = &branch.upstream {
        upstream
    } else {
        return Ok(false);
    };

    let reference = match project_repository
        .git_repository
        .refname_to_id(&upstream.to_string())
    {
        Ok(reference) => reference,
        Err(git::Error::NotFound(_)) => return Ok(false),
        Err(other) => return Err(other).context("failed to find upstream reference"),
    };

    let upstream_commit = project_repository
        .git_repository
        .find_commit(reference)
        .context("failed to find upstream commit")?;

    let merge_base = project_repository
        .git_repository
        .merge_base(upstream_commit.id(), branch.head)?;

    Ok(merge_base != upstream_commit.id())
}

fn list_virtual_commit_files(
    project_repository: &project_repository::Repository,
    commit: &git::Commit,
) -> Result<Vec<VirtualBranchFile>> {
    if commit.parent_count() == 0 {
        return Ok(vec![]);
    }
    let parent = commit.parent(0).context("failed to get parent commit")?;
    let commit_tree = commit.tree().context("failed to get commit tree")?;
    let parent_tree = parent.tree().context("failed to get parent tree")?;
    let diff = diff::trees(
        &project_repository.git_repository,
        &parent_tree,
        &commit_tree,
    )?;
    let hunks_by_filepath = virtual_hunks_by_file_diffs(&project_repository.project().path, diff);
    Ok(virtual_hunks_into_virtual_files(
        project_repository,
        hunks_by_filepath,
    ))
}

fn commit_to_vbranch_commit(
    repository: &project_repository::Repository,
    branch: &branch::Branch,
    commit: &git::Commit,
    is_integrated: bool,
    is_remote: bool,
) -> Result<VirtualBranchCommit> {
    let timestamp = u128::try_from(commit.time().seconds())?;
    let signature = commit.author();
    let message = commit.message().to_owned();

    let files =
        list_virtual_commit_files(repository, commit).context("failed to list commit files")?;

    let parent_ids = commit.parents()?.iter().map(Commit::id).collect::<Vec<_>>();

    let commit = VirtualBranchCommit {
        id: commit.id(),
        created_at: timestamp * 1000,
        author: Author::from(signature),
        description: message,
        is_remote,
        files,
        is_integrated,
        parent_ids,
        branch_id: branch.id,
    };

    Ok(commit)
}

pub fn create_virtual_branch(
    project_repository: &project_repository::Repository,
    create: &BranchCreateRequest,
) -> Result<branch::Branch, errors::CreateVirtualBranchError> {
    let vb_state = project_repository.project().virtual_branches();

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to get default target")?
    else {
        return Err(errors::CreateVirtualBranchError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let commit = project_repository
        .git_repository
        .find_commit(default_target.sha)
        .context("failed to find default target commit")?;

    let tree = commit
        .tree()
        .context("failed to find defaut target commit tree")?;

    let mut all_virtual_branches = vb_state
        .list_branches()
        .context("failed to read virtual branches")?;
    all_virtual_branches.sort_by_key(|branch| branch.order);

    let order = create
        .order
        .unwrap_or(all_virtual_branches.len())
        .clamp(0, all_virtual_branches.len());

    let selected_for_changes = if let Some(selected_for_changes) = create.selected_for_changes {
        if selected_for_changes {
            for mut other_branch in vb_state
                .list_branches()
                .context("failed to read virtual branches")?
            {
                other_branch.selected_for_changes = None;
                vb_state.set_branch(other_branch.clone())?;
            }
            Some(chrono::Utc::now().timestamp_millis())
        } else {
            None
        }
    } else {
        (!all_virtual_branches
            .iter()
            .any(|b| b.selected_for_changes.is_some()))
        .then_some(chrono::Utc::now().timestamp_millis())
    };

    // make space for the new branch
    for (i, branch) in all_virtual_branches.iter().enumerate() {
        let mut branch = branch.clone();
        let new_order = if i < order { i } else { i + 1 };
        if branch.order != new_order {
            branch.order = new_order;
            vb_state
                .set_branch(branch.clone())
                .context("failed to write branch")?;
        }
    }

    let name = dedup(
        &all_virtual_branches
            .iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>(),
        create
            .name
            .as_ref()
            .unwrap_or(&"Virtual branch".to_string()),
    );

    let now = crate::time::now_ms();

    let mut branch = Branch {
        id: BranchId::generate(),
        name,
        notes: String::new(),
        applied: true,
        upstream: None,
        upstream_head: None,
        tree: tree.id(),
        head: default_target.sha,
        created_timestamp_ms: now,
        updated_timestamp_ms: now,
        ownership: BranchOwnershipClaims::default(),
        order,
        selected_for_changes,
    };

    if let Some(ownership) = &create.ownership {
        set_ownership(&vb_state, &mut branch, ownership).context("failed to set ownership")?;
    }

    vb_state
        .set_branch(branch.clone())
        .context("failed to write branch")?;

    project_repository.add_branch_reference(&branch)?;

    Ok(branch)
}

pub fn merge_virtual_branch_upstream(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    signing_key: Option<&keys::PrivateKey>,
    user: Option<&users::User>,
) -> Result<(), errors::MergeVirtualBranchUpstreamError> {
    if conflicts::is_conflicting::<&Path>(project_repository, None)? {
        return Err(errors::MergeVirtualBranchUpstreamError::Conflict(
            errors::ProjectConflict {
                project_id: project_repository.project().id,
            },
        ));
    }

    let vb_state = project_repository.project().virtual_branches();

    let mut branch = match vb_state.get_branch(branch_id) {
        Ok(branch) => Ok(branch),
        Err(reader::Error::NotFound) => Err(
            errors::MergeVirtualBranchUpstreamError::BranchNotFound(errors::BranchNotFound {
                project_id: project_repository.project().id,
                branch_id: *branch_id,
            }),
        ),
        Err(error) => Err(errors::MergeVirtualBranchUpstreamError::Other(error.into())),
    }?;

    // check if the branch upstream can be merged into the wd cleanly
    let repo = &project_repository.git_repository;

    // get upstream from the branch and find the remote branch
    let mut upstream_commit = None;
    let upstream_branch = branch
        .upstream
        .as_ref()
        .context("no upstream branch found")?;
    if let Ok(upstream_oid) = repo.refname_to_id(&upstream_branch.to_string()) {
        if let Ok(upstream_commit_obj) = repo.find_commit(upstream_oid) {
            upstream_commit = Some(upstream_commit_obj);
        }
    }

    // if there is no upstream commit, then there is nothing to do
    if upstream_commit.is_none() {
        // no upstream commit, no merge to be done
        return Ok(());
    }

    // there is an upstream commit, so lets check it out
    let upstream_commit = upstream_commit.unwrap();
    let remote_tree = upstream_commit.tree().context("failed to get tree")?;

    if upstream_commit.id() == branch.head {
        // upstream is already merged, nothing to do
        return Ok(());
    }

    // if any other branches are applied, unapply them
    let applied_branches = vb_state
        .list_branches()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|b| b.applied)
        .filter(|b| b.id != *branch_id)
        .collect::<Vec<_>>();

    // unapply all other branches
    for other_branch in applied_branches {
        unapply_branch(project_repository, &other_branch.id).context("failed to unapply branch")?;
    }

    // get merge base from remote branch commit and target commit
    let merge_base = repo
        .merge_base(upstream_commit.id(), branch.head)
        .context("failed to find merge base")?;
    let merge_tree = repo
        .find_commit(merge_base)
        .and_then(|c| c.tree())
        .context(format!(
            "failed to find merge base commit {} tree",
            merge_base
        ))?;

    // get wd tree
    let wd_tree = project_repository.get_wd_tree()?;

    // try to merge our wd tree with the upstream tree
    let mut merge_index = repo
        .merge_trees(&merge_tree, &wd_tree, &remote_tree)
        .context("failed to merge trees")?;

    if merge_index.has_conflicts() {
        // checkout the conflicts
        repo.checkout_index(&mut merge_index)
            .allow_conflicts()
            .conflict_style_merge()
            .force()
            .checkout()
            .context("failed to checkout index")?;

        // mark conflicts
        let conflicts = merge_index.conflicts().context("failed to get conflicts")?;
        let mut merge_conflicts = Vec::new();
        for path in conflicts.flatten() {
            if let Some(ours) = path.our {
                let path = std::str::from_utf8(&ours.path)
                    .context("failed to convert path to utf8")?
                    .to_string();
                merge_conflicts.push(path);
            }
        }
        conflicts::mark(
            project_repository,
            &merge_conflicts,
            Some(upstream_commit.id()),
        )?;
    } else {
        let merge_tree_oid = merge_index
            .write_tree_to(repo)
            .context("failed to write tree")?;
        let merge_tree = repo
            .find_tree(merge_tree_oid)
            .context("failed to find merge tree")?;

        if *project_repository.project().ok_with_force_push {
            // attempt a rebase
            let (_, committer) = project_repository.git_signatures(user)?;
            let mut rebase_options = git2::RebaseOptions::new();
            rebase_options.quiet(true);
            rebase_options.inmemory(true);
            let mut rebase = repo
                .rebase(
                    Some(branch.head),
                    Some(upstream_commit.id()),
                    None,
                    Some(&mut rebase_options),
                )
                .context("failed to rebase")?;

            let mut rebase_success = true;
            // check to see if these commits have already been pushed
            let mut last_rebase_head = upstream_commit.id();
            while rebase.next().is_some() {
                let index = rebase
                    .inmemory_index()
                    .context("failed to get inmemory index")?;
                if index.has_conflicts() {
                    rebase_success = false;
                    break;
                }

                if let Ok(commit_id) = rebase.commit(None, &committer.clone().into(), None) {
                    last_rebase_head = commit_id.into();
                } else {
                    rebase_success = false;
                    break;
                }
            }

            if rebase_success {
                // rebase worked out, rewrite the branch head
                rebase.finish(None).context("failed to finish rebase")?;

                project_repository
                    .git_repository
                    .checkout_tree(&merge_tree)
                    .force()
                    .checkout()
                    .context("failed to checkout tree")?;

                branch.head = last_rebase_head;
                branch.tree = merge_tree_oid;
                vb_state.set_branch(branch.clone())?;
                super::integration::update_gitbutler_integration(&vb_state, project_repository)?;

                return Ok(());
            }

            rebase.abort().context("failed to abort rebase")?;
        }

        let head_commit = repo
            .find_commit(branch.head)
            .context("failed to find head commit")?;

        let new_branch_head = project_repository.commit(
            user,
            format!(
                "Merged {}/{} into {}",
                upstream_branch.remote(),
                upstream_branch.branch(),
                branch.name
            )
            .as_str(),
            &merge_tree,
            &[&head_commit, &upstream_commit],
            signing_key,
        )?;

        // checkout the merge tree
        repo.checkout_tree(&merge_tree)
            .force()
            .checkout()
            .context("failed to checkout tree")?;

        // write the branch data
        branch.head = new_branch_head;
        branch.tree = merge_tree_oid;
        vb_state.set_branch(branch.clone())?;
    }

    super::integration::update_gitbutler_integration(&vb_state, project_repository)?;

    Ok(())
}

pub fn update_branch(
    project_repository: &project_repository::Repository,
    branch_update: branch::BranchUpdateRequest,
) -> Result<branch::Branch, errors::UpdateBranchError> {
    let vb_state = project_repository.project().virtual_branches();
    let mut branch = vb_state
        .get_branch(&branch_update.id)
        .map_err(|error| match error {
            reader::Error::NotFound => {
                errors::UpdateBranchError::BranchNotFound(errors::BranchNotFound {
                    project_id: project_repository.project().id,
                    branch_id: branch_update.id,
                })
            }
            _ => errors::UpdateBranchError::Other(error.into()),
        })?;

    if let Some(ownership) = branch_update.ownership {
        set_ownership(&vb_state, &mut branch, &ownership).context("failed to set ownership")?;
    }

    if let Some(name) = branch_update.name {
        let all_virtual_branches = vb_state
            .list_branches()
            .context("failed to read virtual branches")?;

        project_repository.delete_branch_reference(&branch)?;

        branch.name = dedup(
            &all_virtual_branches
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            &name,
        );

        project_repository.add_branch_reference(&branch)?;
    };

    if let Some(updated_upstream) = branch_update.upstream {
        let Some(default_target) = vb_state
            .try_get_default_target()
            .context("failed to get default target")?
        else {
            return Err(errors::UpdateBranchError::DefaultTargetNotSet(
                errors::DefaultTargetNotSet {
                    project_id: project_repository.project().id,
                },
            ));
        };

        let upstream_remote = match default_target.push_remote_name {
            Some(remote) => remote.clone(),
            None => default_target.branch.remote().to_owned(),
        };

        let remote_branch = format!(
            "refs/remotes/{}/{}",
            upstream_remote,
            normalize_branch_name(&updated_upstream)
        )
        .parse::<git::RemoteRefname>()
        .unwrap();
        branch.upstream = Some(remote_branch);
    };

    if let Some(notes) = branch_update.notes {
        branch.notes = notes;
    };

    if let Some(order) = branch_update.order {
        branch.order = order;
    };

    if let Some(selected_for_changes) = branch_update.selected_for_changes {
        branch.selected_for_changes = if selected_for_changes {
            for mut other_branch in vb_state
                .list_branches()
                .context("failed to read virtual branches")?
                .into_iter()
                .filter(|b| b.id != branch.id)
            {
                other_branch.selected_for_changes = None;
                vb_state.set_branch(other_branch.clone())?;
            }
            Some(chrono::Utc::now().timestamp_millis())
        } else {
            None
        };
    };

    vb_state
        .set_branch(branch.clone())
        .context("failed to write target branch")?;

    Ok(branch)
}

pub fn delete_branch(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
) -> Result<(), Error> {
    let vb_state = project_repository.project().virtual_branches();
    let branch = match vb_state.get_branch(branch_id) {
        Ok(branch) => Ok(branch),
        Err(reader::Error::NotFound) => return Ok(()),
        Err(error) => Err(error),
    }
    .context("failed to read branch")?;

    if branch.applied && unapply_branch(project_repository, branch_id)?.is_none() {
        return Ok(());
    }

    vb_state
        .remove_branch(branch.id)
        .context("Failed to remove branch")?;

    project_repository.delete_branch_reference(&branch)?;

    ensure_selected_for_changes(&vb_state).context("failed to ensure selected for changes")?;

    _ = branch.snapshot_deletion(project_repository.project());
    Ok(())
}

fn ensure_selected_for_changes(vb_state: &VirtualBranchesHandle) -> Result<()> {
    let mut applied_branches = vb_state
        .list_branches()
        .context("failed to list branches")?
        .into_iter()
        .filter(|b| b.applied)
        .collect::<Vec<_>>();

    if applied_branches.is_empty() {
        println!("no applied branches");
        return Ok(());
    }

    if applied_branches
        .iter()
        .any(|b| b.selected_for_changes.is_some())
    {
        println!("some branches already selected for changes");
        return Ok(());
    }

    applied_branches.sort_by_key(|branch| branch.order);

    applied_branches[0].selected_for_changes = Some(chrono::Utc::now().timestamp_millis());
    vb_state.set_branch(applied_branches[0].clone())?;
    Ok(())
}

fn set_ownership(
    vb_state: &VirtualBranchesHandle,
    target_branch: &mut branch::Branch,
    ownership: &branch::BranchOwnershipClaims,
) -> Result<()> {
    if target_branch.ownership.eq(ownership) {
        // nothing to update
        return Ok(());
    }

    let virtual_branches = vb_state
        .list_branches()
        .context("failed to read virtual branches")?;

    let mut claim_outcomes =
        branch::reconcile_claims(virtual_branches, target_branch, &ownership.claims)?;
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
struct MTimeCache(HashMap<PathBuf, u128>);

impl MTimeCache {
    fn mtime_by_path<P: AsRef<Path>>(&mut self, path: P) -> u128 {
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

pub(super) fn virtual_hunks_by_git_hunks<'a>(
    project_path: &'a Path,
    diff: impl IntoIterator<Item = (PathBuf, Vec<diff::GitHunk>)> + 'a,
) -> impl Iterator<Item = (PathBuf, Vec<VirtualBranchHunk>)> + 'a {
    let mut mtimes = MTimeCache::default();
    diff.into_iter().map(move |(file_path, hunks)| {
        let hunks = hunks
            .into_iter()
            .map(|hunk| {
                VirtualBranchHunk::from_git_hunk(project_path, file_path.clone(), hunk, &mut mtimes)
            })
            .collect::<Vec<_>>();
        (file_path, hunks)
    })
}

pub fn virtual_hunks_by_file_diffs<'a>(
    project_path: &'a Path,
    diff: impl IntoIterator<Item = (PathBuf, FileDiff)> + 'a,
) -> impl Iterator<Item = (PathBuf, Vec<VirtualBranchHunk>)> + 'a {
    virtual_hunks_by_git_hunks(
        project_path,
        diff.into_iter()
            .map(move |(file_path, file)| (file_path, file.hunks)),
    )
}

pub type BranchStatus = HashMap<PathBuf, Vec<diff::GitHunk>>;
pub type VirtualBranchHunksByPathMap = HashMap<PathBuf, Vec<VirtualBranchHunk>>;

// list the virtual branches and their file statuses (statusi?)
#[allow(clippy::type_complexity)]
pub fn get_status_by_branch(
    project_repository: &project_repository::Repository,
    integration_commit: Option<&git::Oid>,
) -> Result<(AppliedStatuses, Vec<diff::FileDiff>)> {
    let vb_state = project_repository.project().virtual_branches();

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to read default target")?
    else {
        return Ok((vec![], vec![]));
    };

    let virtual_branches = vb_state
        .list_branches()
        .context("failed to read virtual branches")?;

    let applied_virtual_branches = virtual_branches
        .iter()
        .filter(|branch| branch.applied)
        .cloned()
        .collect::<Vec<_>>();

    let (applied_status, skipped_files) = get_applied_status(
        project_repository,
        // TODO: Keep this optional or update lots of tests?
        integration_commit.unwrap_or(&default_target.sha),
        &default_target.sha,
        applied_virtual_branches,
    )?;

    let non_applied_virtual_branches = virtual_branches
        .into_iter()
        .filter(|branch| !branch.applied)
        .collect::<Vec<_>>();

    let non_applied_status =
        get_non_applied_status(project_repository, non_applied_virtual_branches)?;

    Ok((
        applied_status
            .into_iter()
            .chain(non_applied_status)
            .collect(),
        skipped_files,
    ))
}

// given a list of non applied virtual branches, return the status of each file, comparing the default target with
// virtual branch latest tree
//
// ownerships are not taken into account here, as they are not relevant for non applied branches
fn get_non_applied_status(
    project_repository: &project_repository::Repository,
    virtual_branches: Vec<branch::Branch>,
) -> Result<Vec<(branch::Branch, BranchStatus)>> {
    virtual_branches
        .into_iter()
        .map(|branch| -> Result<(branch::Branch, BranchStatus)> {
            if branch.applied {
                bail!("branch {} is applied", branch.name);
            }
            let branch_tree = project_repository
                .git_repository
                .find_tree(branch.tree)
                .context(format!("failed to find tree {}", branch.tree))?;

            let head_tree = project_repository
                .git_repository
                .find_commit(branch.head)
                .context("failed to find target commit")?
                .tree()
                .context("failed to find target tree")?;

            let diff = diff::trees(&project_repository.git_repository, &head_tree, &branch_tree)?;

            Ok((branch, diff::diff_files_into_hunks(diff).collect()))
        })
        .collect::<Result<Vec<_>>>()
}

// Returns branches and their associated file changes, in addition to a list
// of skipped files.
fn get_applied_status(
    project_repository: &project_repository::Repository,
    integration_commit: &git::Oid,
    target_sha: &git::Oid,
    mut virtual_branches: Vec<branch::Branch>,
) -> Result<(AppliedStatuses, Vec<diff::FileDiff>)> {
    let base_file_diffs = diff::workdir(&project_repository.git_repository, integration_commit)
        .context("failed to diff workdir")?;

    let mut skipped_files: Vec<diff::FileDiff> = Vec::new();
    for file_diff in base_file_diffs.values() {
        if file_diff.skipped {
            skipped_files.push(file_diff.clone());
        }
    }
    let mut base_diffs: HashMap<_, _> = diff_files_into_hunks(base_file_diffs).collect();

    // sort by order, so that the default branch is first (left in the ui)
    virtual_branches.sort_by(|a, b| a.order.cmp(&b.order));

    if virtual_branches.is_empty() && !base_diffs.is_empty() {
        virtual_branches =
            vec![
                create_virtual_branch(project_repository, &BranchCreateRequest::default())
                    .context("failed to create default branch")?,
            ];
    }

    let mut commit_to_branch = HashMap::new();
    for branch in &mut virtual_branches {
        for commit in project_repository.log(branch.head, LogUntil::Commit(*target_sha))? {
            commit_to_branch.insert(commit.id(), branch.id);
        }
    }

    let mut diffs_by_branch: HashMap<BranchId, BranchStatus> = virtual_branches
        .iter()
        .map(|branch| (branch.id, HashMap::new()))
        .collect();

    let mut mtimes = MTimeCache::default();
    let mut locked_hunk_map = HashMap::<HunkHash, Vec<diff::HunkLock>>::new();

    let merge_base = project_repository
        .git_repository
        .merge_base(*target_sha, *integration_commit)?;

    // The merge base between the integration commit and target _is_ the
    // target, unless you are resolving merge conflicts. If that's the case
    // we ignore locks until we are back to normal mode.
    //
    // If we keep the test repo and panic when `berge_base != target_sha`
    // when running the test below, then we have the following commit graph.
    //
    // Test: virtual_branches::update_base_branch::applied_branch::integrated_with_locked_conflicting_hunks
    // Command: `git log --oneline --all --graph``
    // * 244b526 GitButler WIP Commit
    // | * ea2956e (HEAD -> gitbutler/integration) GitButler Integration Commit
    // | *   3f2ccce (origin/master) Merge pull request from refs/heads/Virtual-branch
    // | |\
    // | |/
    // |/|
    // | * a6a0ed8 second
    // | | * f833dbe (int) Workspace Head
    // | |/
    // |/|
    // * | dee400c (origin/Virtual-branch, merge_base) third
    // |/
    // * 56c139c (master) first
    // * 6276165 Initial commit
    // (END)
    if merge_base == *target_sha {
        for (path, hunks) in base_diffs.clone().into_iter() {
            for hunk in hunks {
                let blame = project_repository.git_repository.blame(
                    &path,
                    hunk.old_start,
                    (hunk.old_start + hunk.old_lines).saturating_sub(1),
                    target_sha,
                    integration_commit,
                );

                if let Ok(blame) = blame {
                    for blame_hunk in blame.iter() {
                        let commit_id = git::Oid::from(blame_hunk.orig_commit_id());
                        if commit_id != *target_sha && commit_id != *integration_commit {
                            let hash = Hunk::hash_diff(&hunk.diff_lines);
                            let Some(branch_id) = commit_to_branch.get(&commit_id) else {
                                // This is a truly absurd situation
                                // TODO: Test if this still happens
                                tracing::error!(
                                    "commit {:?} not found in commit_to_branch map {:?}",
                                    commit_id,
                                    commit_to_branch
                                );
                                continue;
                            };

                            let hunk_lock = diff::HunkLock {
                                branch_id: *branch_id,
                                commit_id,
                            };
                            locked_hunk_map
                                .entry(hash)
                                .and_modify(|locks| {
                                    locks.push(hunk_lock);
                                })
                                .or_insert(vec![hunk_lock]);
                        }
                    }
                }
            }
        }
    }

    for branch in &mut virtual_branches {
        if !branch.applied {
            bail!("branch {} is not applied", branch.name);
        }

        let old_claims = branch.ownership.claims.clone();
        let new_claims = old_claims
            .iter()
            .filter_map(|claim| {
                let git_diff_hunks = match base_diffs.get_mut(&claim.file_path) {
                    None => return None,
                    Some(hunks) => hunks,
                };

                let mtime = mtimes.mtime_by_path(claim.file_path.as_path());

                let claimed_hunks: Vec<Hunk> = claim
                    .hunks
                    .iter()
                    .filter_map(|claimed_hunk| {
                        // if any of the current hunks intersects with the owned hunk, we want to keep it
                        for (i, git_diff_hunk) in git_diff_hunks.iter().enumerate() {
                            let hash = Hunk::hash_diff(&git_diff_hunk.diff_lines);
                            if locked_hunk_map.contains_key(&hash) {
                                return None; // Defer allocation to unclaimed hunks processing
                            }
                            if claimed_hunk.eq(&Hunk::from(git_diff_hunk)) {
                                let timestamp = claimed_hunk.timestamp_ms().unwrap_or(mtime);
                                diffs_by_branch
                                    .entry(branch.id)
                                    .or_default()
                                    .entry(claim.file_path.clone())
                                    .or_default()
                                    .push(git_diff_hunk.clone());

                                git_diff_hunks.remove(i);
                                return Some(
                                    claimed_hunk
                                        .clone()
                                        .with_timestamp(timestamp)
                                        .with_hash(hash),
                                );
                            } else if claimed_hunk.intersects(git_diff_hunk) {
                                diffs_by_branch
                                    .entry(branch.id)
                                    .or_default()
                                    .entry(claim.file_path.clone())
                                    .or_default()
                                    .push(git_diff_hunk.clone());
                                let updated_hunk = Hunk {
                                    start: git_diff_hunk.new_start,
                                    end: git_diff_hunk.new_start + git_diff_hunk.new_lines,
                                    timestamp_ms: Some(mtime),
                                    hash: Some(hash),
                                    locked_to: git_diff_hunk.locked_to.to_vec(),
                                };
                                git_diff_hunks.remove(i);
                                return Some(updated_hunk);
                            }
                        }
                        None
                    })
                    .collect();

                if claimed_hunks.is_empty() {
                    // No need for an empty claim
                    None
                } else {
                    Some(OwnershipClaim {
                        file_path: claim.file_path.clone(),
                        hunks: claimed_hunks,
                    })
                }
            })
            .collect();

        branch.ownership = BranchOwnershipClaims { claims: new_claims };
    }

    let max_selected_for_changes = virtual_branches
        .iter()
        .filter_map(|b| b.selected_for_changes)
        .max()
        .unwrap_or(-1);
    let default_vbranch_pos = virtual_branches
        .iter()
        .position(|b| b.selected_for_changes == Some(max_selected_for_changes))
        .unwrap_or(0);

    // Everything claimed has been removed from `base_diffs`, here we just
    // process the remaining ones.
    for (filepath, hunks) in base_diffs {
        for hunk in hunks {
            let hash = Hunk::hash_diff(&hunk.diff_lines);
            let locked_to = locked_hunk_map.get(&hash);

            let vbranch_pos = if let Some(locks) = locked_to {
                let first_lock = &locks[0];
                let p = virtual_branches
                    .iter()
                    .position(|vb| vb.id == first_lock.branch_id);
                match p {
                    Some(p) => p,
                    _ => default_vbranch_pos,
                }
            } else {
                default_vbranch_pos
            };

            let hash = Hunk::hash_diff(&hunk.diff_lines);
            let mut new_hunk = Hunk::from(&hunk)
                .with_timestamp(mtimes.mtime_by_path(filepath.as_path()))
                .with_hash(hash);
            new_hunk.locked_to = match locked_to {
                Some(locked_to) => locked_to.clone(),
                _ => vec![],
            };

            virtual_branches[vbranch_pos].ownership.put(OwnershipClaim {
                file_path: filepath.clone(),
                hunks: vec![Hunk::from(&hunk)
                    .with_timestamp(mtimes.mtime_by_path(filepath.as_path()))
                    .with_hash(Hunk::hash_diff(&hunk.diff_lines))],
            });

            let hunk = match locked_to {
                Some(locks) => hunk.with_locks(locks),
                _ => hunk,
            };
            diffs_by_branch
                .entry(virtual_branches[vbranch_pos].id)
                .or_default()
                .entry(filepath.clone())
                .or_default()
                .push(hunk);
        }
    }

    let mut hunks_by_branch = diffs_by_branch
        .into_iter()
        .map(|(branch_id, hunks)| {
            (
                virtual_branches
                    .iter()
                    .find(|b| b.id.eq(&branch_id))
                    .unwrap()
                    .clone(),
                hunks,
            )
        })
        .collect::<Vec<_>>();

    // write updated state if not resolving
    if !project_repository.is_resolving() {
        let vb_state = project_repository.project().virtual_branches();
        for (vbranch, files) in &mut hunks_by_branch {
            vbranch.tree = write_tree(project_repository, &vbranch.head, files)?;
            vb_state
                .set_branch(vbranch.clone())
                .context(format!("failed to write virtual branch {}", vbranch.name))?;
        }
    }

    Ok((hunks_by_branch, skipped_files))
}

/// NOTE: There is no use returning an iterator here as this acts like the final product.
fn virtual_hunks_into_virtual_files(
    project_repository: &project_repository::Repository,
    hunks: impl IntoIterator<Item = (PathBuf, Vec<VirtualBranchHunk>)>,
) -> Vec<VirtualBranchFile> {
    hunks
        .into_iter()
        .map(|(path, hunks)| {
            let id = path.display().to_string();
            let conflicted =
                conflicts::is_conflicting(project_repository, Some(&id)).unwrap_or(false);
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
pub fn reset_branch(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    target_commit_oid: git::Oid,
) -> Result<(), errors::ResetBranchError> {
    let vb_state = project_repository.project().virtual_branches();

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to read default target")?
    else {
        return Err(errors::ResetBranchError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let mut branch = match vb_state.get_branch(branch_id) {
        Ok(branch) => Ok(branch),
        Err(reader::Error::NotFound) => Err(errors::ResetBranchError::BranchNotFound(
            errors::BranchNotFound {
                branch_id: *branch_id,
                project_id: project_repository.project().id,
            },
        )),
        Err(error) => Err(errors::ResetBranchError::Other(error.into())),
    }?;

    if branch.head == target_commit_oid {
        // nothing to do
        return Ok(());
    }

    if default_target.sha != target_commit_oid
        && !project_repository
            .l(branch.head, LogUntil::Commit(default_target.sha))?
            .contains(&target_commit_oid)
    {
        return Err(errors::ResetBranchError::CommitNotFoundInBranch(
            target_commit_oid,
        ));
    }

    // Compute the old workspace before resetting so we can can figure out
    // what hunks were released by this reset, and assign them to this branch.
    let old_head = get_workspace_head(&vb_state, project_repository)?;

    branch.head = target_commit_oid;
    vb_state
        .set_branch(branch.clone())
        .context("failed to write branch")?;

    let updated_head = get_workspace_head(&vb_state, project_repository)?;
    let repo = &project_repository.git_repository;
    let diff = trees(
        repo,
        &repo.find_commit(updated_head)?.tree()?,
        &repo.find_commit(old_head)?.tree()?,
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

    super::integration::update_gitbutler_integration(&vb_state, project_repository)
        .context("failed to update gitbutler integration")?;

    Ok(())
}

fn diffs_into_virtual_files(
    project_repository: &project_repository::Repository,
    diffs: BranchStatus,
) -> Vec<VirtualBranchFile> {
    let hunks_by_filepath = virtual_hunks_by_git_hunks(&project_repository.project().path, diffs);
    virtual_hunks_into_virtual_files(project_repository, hunks_by_filepath)
}

// this function takes a list of file ownership,
// constructs a tree from those changes on top of the target
// and writes it as a new tree for storage
pub fn write_tree(
    project_repository: &project_repository::Repository,
    target: &git::Oid,
    files: impl IntoIterator<Item = (impl Borrow<PathBuf>, impl Borrow<Vec<diff::GitHunk>>)>,
) -> Result<git::Oid> {
    write_tree_onto_commit(project_repository, *target, files)
}

pub fn write_tree_onto_commit(
    project_repository: &project_repository::Repository,
    commit_oid: git::Oid,
    files: impl IntoIterator<Item = (impl Borrow<PathBuf>, impl Borrow<Vec<diff::GitHunk>>)>,
) -> Result<git::Oid> {
    // read the base sha into an index
    let git_repository = &project_repository.git_repository;

    let head_commit = git_repository.find_commit(commit_oid)?;
    let base_tree = head_commit.tree()?;

    write_tree_onto_tree(project_repository, &base_tree, files)
}

pub fn write_tree_onto_tree(
    project_repository: &project_repository::Repository,
    base_tree: &git::Tree,
    files: impl IntoIterator<Item = (impl Borrow<PathBuf>, impl Borrow<Vec<diff::GitHunk>>)>,
) -> Result<git::Oid> {
    let git_repository = &project_repository.git_repository;
    let mut builder = git_repository.treebuilder(Some(base_tree));
    // now update the index with content in the working directory for each file
    for (rel_path, hunks) in files {
        let rel_path = rel_path.borrow();
        let hunks = hunks.borrow();
        let full_path = project_repository.path().join(rel_path);

        let is_submodule = full_path.is_dir()
            && hunks.len() == 1
            && hunks[0].diff_lines.contains_str(b"Subproject commit");

        // if file exists
        if full_path.exists() {
            // if file is executable, use 755, otherwise 644
            let mut filemode = git::FileMode::Blob;
            // check if full_path file is executable
            if let Ok(metadata) = std::fs::symlink_metadata(&full_path) {
                #[cfg(target_family = "unix")]
                {
                    if metadata.permissions().mode() & 0o111 != 0 {
                        filemode = git::FileMode::BlobExecutable;
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
                                .then_some(git::FileMode::BlobExecutable)
                        })
                        .unwrap_or(filemode);
                }

                if metadata.file_type().is_symlink() {
                    filemode = git::FileMode::Link;
                }
            }

            // get the blob
            if filemode == git::FileMode::Link {
                // it's a symlink, make the content the path of the link
                let link_target = std::fs::read_link(&full_path)?;

                // if the link target is inside the project repository, make it relative
                let link_target = link_target
                    .strip_prefix(project_repository.path())
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
                    ))?;

                    // create a blob
                    let new_blob_oid = git_repository.blob(&blob_contents)?;
                    // upsert into the builder
                    builder.upsert(rel_path, new_blob_oid, filemode);
                }
            } else if is_submodule {
                let mut blob_contents = BString::default();

                let mut hunks = hunks.iter().collect::<Vec<_>>();
                hunks.sort_by_key(|hunk| hunk.new_start);
                for hunk in hunks {
                    let patch = Patch::from_bytes(&hunk.diff_lines)?;
                    blob_contents = apply(&blob_contents, &patch)
                        .context(format!("failed to apply {}", &hunk.diff_lines))?;
                }

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
    let tree_oid = builder.write().context("failed to write updated tree")?;

    Ok(tree_oid)
}

fn _print_tree(repo: &git2::Repository, tree: &git2::Tree) -> Result<()> {
    println!("tree id: {}", tree.id());
    for entry in tree {
        println!(
            "  entry: {} {}",
            entry.name().unwrap_or_default(),
            entry.id()
        );
        // get entry contents
        let object = entry.to_object(repo).context("failed to get object")?;
        let blob = object.as_blob().context("failed to get blob")?;
        // convert content to string
        if let Ok(content) = std::str::from_utf8(blob.content()) {
            println!("    blob: {}", content);
        } else {
            println!("    blob: BINARY");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn commit(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    message: &str,
    ownership: Option<&branch::BranchOwnershipClaims>,
    signing_key: Option<&keys::PrivateKey>,
    user: Option<&users::User>,
    run_hooks: bool,
) -> Result<git::Oid, errors::CommitError> {
    let mut message_buffer = message.to_owned();
    let vb_state = project_repository.project().virtual_branches();

    if run_hooks {
        let hook_result = project_repository
            .git_repository
            .run_hook_commit_msg(&mut message_buffer)
            .context("failed to run hook")?;

        if let HookResult::RunNotSuccessful { stdout, .. } = hook_result {
            return Err(errors::CommitError::CommitMsgHookRejected(stdout));
        }

        let hook_result = project_repository
            .git_repository
            .run_hook_pre_commit()
            .context("failed to run hook")?;

        if let HookResult::RunNotSuccessful { stdout, .. } = hook_result {
            return Err(errors::CommitError::CommitHookRejected(stdout));
        }
    }

    let message = &message_buffer;

    let integration_commit_id =
        super::integration::get_workspace_head(&vb_state, project_repository)?;
    // get the files to commit
    let (statuses, _) = get_status_by_branch(project_repository, Some(&integration_commit_id))
        .context("failed to get status by branch")?;

    let (ref mut branch, files) = statuses
        .into_iter()
        .find(|(branch, _)| branch.id == *branch_id)
        .ok_or_else(|| {
            errors::CommitError::BranchNotFound(errors::BranchNotFound {
                project_id: project_repository.project().id,
                branch_id: *branch_id,
            })
        })?;

    update_conflict_markers(project_repository, &files)?;

    if conflicts::is_conflicting::<&Path>(project_repository, None)? {
        return Err(errors::CommitError::Conflicted(errors::ProjectConflict {
            project_id: project_repository.project().id,
        }));
    }

    let tree_oid = if let Some(ownership) = ownership {
        let files = files.into_iter().filter_map(|(filepath, hunks)| {
            let hunks = hunks
                .into_iter()
                .filter(|hunk| {
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

    let git_repository = &project_repository.git_repository;
    let parent_commit = git_repository
        .find_commit(branch.head)
        .context(format!("failed to find commit {:?}", branch.head))?;
    let tree = git_repository
        .find_tree(tree_oid)
        .context(format!("failed to find tree {:?}", tree_oid))?;

    // now write a commit, using a merge parent if it exists
    let extra_merge_parent =
        conflicts::merge_parent(project_repository).context("failed to get merge parent")?;

    let commit_oid = match extra_merge_parent {
        Some(merge_parent) => {
            let merge_parent = git_repository
                .find_commit(merge_parent)
                .context(format!("failed to find merge parent {:?}", merge_parent))?;
            let commit_oid = project_repository.commit(
                user,
                message,
                &tree,
                &[&parent_commit, &merge_parent],
                signing_key,
            )?;
            conflicts::clear(project_repository).context("failed to clear conflicts")?;
            commit_oid
        }
        None => project_repository.commit(user, message, &tree, &[&parent_commit], signing_key)?,
    };

    if run_hooks {
        project_repository
            .git_repository
            .run_hook_post_commit()
            .context("failed to run hook")?;
    }

    let vb_state = project_repository.project().virtual_branches();
    branch.tree = tree_oid;
    branch.head = commit_oid;
    vb_state
        .set_branch(branch.clone())
        .context("failed to write branch")?;

    super::integration::update_gitbutler_integration(&vb_state, project_repository)
        .context("failed to update gitbutler integration")?;

    Ok(commit_oid)
}

pub fn push(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    with_force: bool,
    credentials: &git::credentials::Helper,
    askpass: Option<Option<BranchId>>,
) -> Result<(), errors::PushError> {
    let vb_state = project_repository.project().virtual_branches();

    let mut vbranch = vb_state
        .get_branch(branch_id)
        .map_err(|error| match error {
            reader::Error::NotFound => errors::PushError::BranchNotFound(errors::BranchNotFound {
                project_id: project_repository.project().id,
                branch_id: *branch_id,
            }),
            error => errors::PushError::Other(error.into()),
        })?;

    let remote_branch = if let Some(upstream_branch) = &vbranch.upstream {
        upstream_branch.clone()
    } else {
        let Some(default_target) = vb_state
            .try_get_default_target()
            .context("failed to get default target")?
        else {
            return Err(errors::PushError::DefaultTargetNotSet(
                errors::DefaultTargetNotSet {
                    project_id: project_repository.project().id,
                },
            ));
        };

        let upstream_remote = match default_target.push_remote_name {
            Some(remote) => remote.clone(),
            None => default_target.branch.remote().to_owned(),
        };

        let remote_branch = format!(
            "refs/remotes/{}/{}",
            upstream_remote,
            normalize_branch_name(&vbranch.name)
        )
        .parse::<git::RemoteRefname>()
        .context("failed to parse remote branch name")?;

        let remote_branches = project_repository.git_remote_branches()?;
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

fn is_commit_integrated(
    project_repository: &project_repository::Repository,
    target: &target::Target,
    commit: &git::Commit,
) -> Result<bool> {
    let remote_branch = project_repository
        .git_repository
        .find_branch(&target.branch.clone().into())?;
    let remote_head = remote_branch.peel_to_commit()?;
    let upstream_commits = project_repository.l(
        remote_head.id(),
        project_repository::LogUntil::Commit(target.sha),
    )?;

    if target.sha.eq(&commit.id()) {
        // could not be integrated if heads are the same.
        return Ok(false);
    }

    if upstream_commits.is_empty() {
        // could not be integrated - there is nothing new upstream.
        return Ok(false);
    }

    if upstream_commits.contains(&commit.id()) {
        return Ok(true);
    }

    let merge_base_id = project_repository
        .git_repository
        .merge_base(target.sha, commit.id())?;
    if merge_base_id.eq(&commit.id()) {
        // if merge branch is the same as branch head and there are upstream commits
        // then it's integrated
        return Ok(true);
    }

    let merge_base = project_repository
        .git_repository
        .find_commit(merge_base_id)?;
    let merge_base_tree = merge_base.tree()?;
    let upstream = project_repository
        .git_repository
        .find_commit(remote_head.id())?;
    let upstream_tree = upstream.tree()?;

    if merge_base_tree.id() == upstream_tree.id() {
        // if merge base is the same as upstream tree, then it's integrated
        return Ok(true);
    }

    // try to merge our tree into the upstream tree
    let mut merge_index = project_repository
        .git_repository
        .merge_trees(&merge_base_tree, &commit.tree()?, &upstream_tree)
        .context("failed to merge trees")?;

    if merge_index.has_conflicts() {
        return Ok(false);
    }

    let merge_tree_oid = merge_index
        .write_tree_to(&project_repository.git_repository)
        .context("failed to write tree")?;

    // if the merge_tree is the same as the new_target_tree and there are no files (uncommitted changes)
    // then the vbranch is fully merged
    Ok(merge_tree_oid == upstream_tree.id())
}

pub fn is_remote_branch_mergeable(
    project_repository: &project_repository::Repository,
    branch_name: &git::RemoteRefname,
) -> Result<bool, errors::IsRemoteBranchMergableError> {
    let vb_state = project_repository.project().virtual_branches();

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to get default target")?
    else {
        return Err(errors::IsRemoteBranchMergableError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let target_commit = project_repository
        .git_repository
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;

    let branch = match project_repository
        .git_repository
        .find_branch(&branch_name.into())
    {
        Ok(branch) => Ok(branch),
        Err(git::Error::NotFound(_)) => Err(errors::IsRemoteBranchMergableError::BranchNotFound(
            branch_name.clone(),
        )),
        Err(error) => Err(errors::IsRemoteBranchMergableError::Other(error.into())),
    }?;
    let branch_oid = branch.target().context("detatched head")?;
    let branch_commit = project_repository
        .git_repository
        .find_commit(branch_oid)
        .context("failed to find branch commit")?;

    let base_tree = find_base_tree(
        &project_repository.git_repository,
        &branch_commit,
        &target_commit,
    )?;

    let wd_tree = project_repository.get_wd_tree()?;

    let branch_tree = branch_commit.tree().context("failed to find branch tree")?;
    let mergeable = !project_repository
        .git_repository
        .merge_trees(&base_tree, &branch_tree, &wd_tree)
        .context("failed to merge trees")?
        .has_conflicts();

    Ok(mergeable)
}

pub fn is_virtual_branch_mergeable(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
) -> Result<bool, errors::IsVirtualBranchMergeable> {
    let vb_state = project_repository.project().virtual_branches();
    let branch = match vb_state.get_branch(branch_id) {
        Ok(branch) => Ok(branch),
        Err(reader::Error::NotFound) => Err(errors::IsVirtualBranchMergeable::BranchNotFound(
            errors::BranchNotFound {
                project_id: project_repository.project().id,
                branch_id: *branch_id,
            },
        )),
        Err(error) => Err(errors::IsVirtualBranchMergeable::Other(error.into())),
    }?;

    if branch.applied {
        return Ok(true);
    }

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to read default target")?
    else {
        return Err(errors::IsVirtualBranchMergeable::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    // determine if this branch is up to date with the target/base
    let merge_base = project_repository
        .git_repository
        .merge_base(default_target.sha, branch.head)
        .context("failed to find merge base")?;

    if merge_base != default_target.sha {
        return Ok(false);
    }

    let branch_commit = project_repository
        .git_repository
        .find_commit(branch.head)
        .context("failed to find branch commit")?;

    let target_commit = project_repository
        .git_repository
        .find_commit(default_target.sha)
        .context("failed to find target commit")?;

    let base_tree = find_base_tree(
        &project_repository.git_repository,
        &branch_commit,
        &target_commit,
    )?;

    let wd_tree = project_repository.get_wd_tree()?;

    // determine if this tree is mergeable
    let branch_tree = project_repository
        .git_repository
        .find_tree(branch.tree)
        .context("failed to find branch tree")?;

    let is_mergeable = !project_repository
        .git_repository
        .merge_trees(&base_tree, &branch_tree, &wd_tree)
        .context("failed to merge trees")?
        .has_conflicts();

    Ok(is_mergeable)
}

// this function takes a list of file ownership from a "from" commit and "moves"
// those changes to a "to" commit in a branch. This allows users to drag changes
// from one commit to another.
// if the "to" commit is below the "from" commit, the changes are simply added to the "to" commit
// and the rebase should be simple. if the "to" commit is above the "from" commit,
// the changes need to be removed from the "from" commit, everything rebased,
// then added to the "to" commit and everything above that rebased again.
pub fn move_commit_file(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    from_commit_oid: git::Oid,
    to_commit_oid: git::Oid,
    target_ownership: &BranchOwnershipClaims,
) -> Result<git::Oid, errors::VirtualBranchError> {
    let vb_state = project_repository.project().virtual_branches();

    let mut target_branch = match vb_state.get_branch(branch_id) {
        Ok(branch) => Ok(branch),
        Err(reader::Error::NotFound) => return Ok(to_commit_oid), // this is wrong
        Err(error) => Err(error),
    }
    .context("failed to read branch")?;

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to read default target")?
    else {
        return Err(errors::VirtualBranchError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let mut to_amend_oid = to_commit_oid;
    let mut amend_commit = project_repository
        .git_repository
        .find_commit(to_amend_oid)
        .context("failed to find commit")?;

    // find all the commits upstream from the target "to" commit
    let mut upstream_commits = project_repository.l(
        target_branch.head,
        project_repository::LogUntil::Commit(amend_commit.id()),
    )?;

    // get a list of all the diffs across all the virtual branches
    let base_file_diffs = diff::workdir(&project_repository.git_repository, &default_target.sha)
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
        return Err(errors::VirtualBranchError::TargetOwnerhshipNotFound(
            target_ownership.clone(),
        ));
    }

    // is from_commit_oid in upstream_commits?
    if !upstream_commits.contains(&from_commit_oid) {
        // this means that the "from" commit is _below_ the "to" commit in the history
        // which makes things a little more complicated because in this case we need to
        // remove the changes from the lower "from" commit, rebase everything, then add the changes
        // to the _rebased_ version of the "to" commit that is above it.

        // first, let's get the from commit data and it's parent data
        let from_commit = project_repository
            .git_repository
            .find_commit(from_commit_oid)
            .context("failed to find commit")?;
        let from_tree = from_commit.tree().context("failed to find tree")?;
        let from_parent = from_commit.parent(0).context("failed to find parent")?;
        let from_parent_tree = from_parent.tree().context("failed to find parent tree")?;

        // ok, what is the entire patch introduced in the "from" commit?
        // we need to remove the parts of this patch that are in target_ownership (the parts we're moving)
        // and then apply the rest to the parent tree of the "from" commit to
        // create the new "from" commit without the changes we're moving
        let from_commit_diffs = diff::trees(
            &project_repository.git_repository,
            &from_parent_tree,
            &from_tree,
        )
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

        let repo = &project_repository.git_repository;

        // write our new tree and commit for the new "from" commit without the moved changes
        let new_from_tree_oid =
            write_tree_onto_commit(project_repository, from_parent.id(), &diffs_to_keep)?;
        let new_from_tree = &repo
            .find_tree(new_from_tree_oid)
            .map_err(|_error| errors::VirtualBranchError::GitObjectNotFound(new_from_tree_oid))?;
        let new_from_commit_oid = repo
            .commit(
                None,
                &from_commit.author(),
                &from_commit.committer(),
                &from_commit.message().to_str_lossy(),
                new_from_tree,
                &[&from_parent],
            )
            .map_err(|_error| errors::VirtualBranchError::CommitFailed)?;

        // rebase everything above the new "from" commit that has the moved changes removed
        let new_head = match cherry_rebase(
            project_repository,
            new_from_commit_oid,
            from_commit_oid,
            target_branch.head,
        ) {
            Ok(Some(new_head)) => new_head,
            _ => {
                return Err(errors::VirtualBranchError::RebaseFailed);
            }
        };

        // ok, now we need to identify which the new "to" commit is in the rebased history
        // so we'll take a list of the upstream oids and find it simply based on location
        // (since the order should not have changed in our simple rebase)
        let old_upstream_commit_oids = project_repository.l(
            target_branch.head,
            project_repository::LogUntil::Commit(default_target.sha),
        )?;

        let new_upstream_commit_oids = project_repository.l(
            new_head,
            project_repository::LogUntil::Commit(default_target.sha),
        )?;

        // find to_commit_oid offset in upstream_commits vector
        let to_commit_offset = old_upstream_commit_oids
            .iter()
            .position(|c| *c == to_amend_oid)
            .ok_or(errors::VirtualBranchError::Other(anyhow!(
                "failed to find commit in old commits"
            )))?;

        // find the new "to" commit in our new rebased upstream commits
        to_amend_oid = *new_upstream_commit_oids.get(to_commit_offset).ok_or(
            errors::VirtualBranchError::Other(anyhow!("failed to find commit in new commits")),
        )?;

        // reset the "to" commit variable for writing the changes back to
        amend_commit = project_repository
            .git_repository
            .find_commit(to_amend_oid)
            .context("failed to find commit")?;

        // reset the concept of what the upstream commits are to be the rebased ones
        upstream_commits = project_repository.l(
            new_head,
            project_repository::LogUntil::Commit(amend_commit.id()),
        )?;
    }

    // ok, now we will apply the moved changes to the "to" commit.
    // if we were moving the changes down, we didn't need to rewrite the "from" commit
    // because it will be rewritten with the upcoming rebase.
    // if we were moving the changes "up" we've already rewritten the "from" commit

    // apply diffs_to_amend to the commit tree
    // and write a new commit with the changes we're moving
    let new_tree_oid = write_tree_onto_commit(project_repository, to_amend_oid, &diffs_to_amend)?;
    let new_tree = project_repository
        .git_repository
        .find_tree(new_tree_oid)
        .context("failed to find new tree")?;
    let parents = amend_commit
        .parents()
        .context("failed to find head commit parents")?;
    let commit_oid = project_repository
        .git_repository
        .commit(
            None,
            &amend_commit.author(),
            &amend_commit.committer(),
            &amend_commit.message().to_str_lossy(),
            &new_tree,
            &parents.iter().collect::<Vec<_>>(),
        )
        .context("failed to create commit")?;

    // now rebase upstream commits, if needed

    // if there are no upstream commits (the "to" commit was the branch head), then we're done
    if upstream_commits.is_empty() {
        target_branch.head = commit_oid;
        vb_state.set_branch(target_branch.clone())?;
        super::integration::update_gitbutler_integration(&vb_state, project_repository)?;
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
        super::integration::update_gitbutler_integration(&vb_state, project_repository)?;
        Ok(commit_oid)
    } else {
        Err(errors::VirtualBranchError::RebaseFailed)
    }
}

// takes a list of file ownership and a commit oid and rewrites that commit to
// add the file changes. The branch is then rebased onto the new commit
// and the respective branch head is updated
pub fn amend(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    commit_oid: git::Oid,
    target_ownership: &BranchOwnershipClaims,
) -> Result<git::Oid, errors::VirtualBranchError> {
    if conflicts::is_conflicting::<&Path>(project_repository, None)? {
        return Err(errors::VirtualBranchError::Conflict(
            errors::ProjectConflict {
                project_id: project_repository.project().id,
            },
        ));
    }

    let vb_state = project_repository.project().virtual_branches();

    let all_branches = vb_state
        .list_branches()
        .context("failed to read virtual branches")?;

    if !all_branches.iter().any(|b| b.id == *branch_id) {
        return Err(errors::VirtualBranchError::BranchNotFound(
            errors::BranchNotFound {
                project_id: project_repository.project().id,
                branch_id: *branch_id,
            },
        ));
    }

    let applied_branches = all_branches
        .into_iter()
        .filter(|b| b.applied)
        .collect::<Vec<_>>();

    if !applied_branches.iter().any(|b| b.id == *branch_id) {
        return Err(errors::VirtualBranchError::BranchNotFound(
            errors::BranchNotFound {
                project_id: project_repository.project().id,
                branch_id: *branch_id,
            },
        ));
    }

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to read default target")?
    else {
        return Err(errors::VirtualBranchError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let integration_commit_id =
        super::integration::get_workspace_head(&vb_state, project_repository)?;

    let (mut applied_statuses, _) = get_applied_status(
        project_repository,
        &integration_commit_id,
        &default_target.sha,
        applied_branches,
    )?;

    let (ref mut target_branch, target_status) = applied_statuses
        .iter_mut()
        .find(|(b, _)| b.id == *branch_id)
        .ok_or_else(|| {
            errors::VirtualBranchError::BranchNotFound(errors::BranchNotFound {
                project_id: project_repository.project().id,
                branch_id: *branch_id,
            })
        })?;

    if target_branch.upstream.is_some() && !project_repository.project().ok_with_force_push {
        // amending to a pushed head commit will cause a force push that is not allowed
        return Err(errors::VirtualBranchError::ForcePushNotAllowed(
            errors::ForcePushNotAllowed {
                project_id: project_repository.project().id,
            },
        ));
    }

    if project_repository
        .l(
            target_branch.head,
            project_repository::LogUntil::Commit(default_target.sha),
        )?
        .is_empty()
    {
        return Err(errors::VirtualBranchError::BranchHasNoCommits);
    }

    // find commit oid
    let amend_commit = project_repository
        .git_repository
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
        return Err(errors::VirtualBranchError::TargetOwnerhshipNotFound(
            target_ownership.clone(),
        ));
    }

    // apply diffs_to_amend to the commit tree
    let new_tree_oid = write_tree_onto_commit(project_repository, commit_oid, &diffs_to_amend)?;
    let new_tree = project_repository
        .git_repository
        .find_tree(new_tree_oid)
        .context("failed to find new tree")?;

    let parents = amend_commit
        .parents()
        .context("failed to find head commit parents")?;

    let commit_oid = project_repository
        .git_repository
        .commit(
            None,
            &amend_commit.author(),
            &amend_commit.committer(),
            &amend_commit.message().to_str_lossy(),
            &new_tree,
            &parents.iter().collect::<Vec<_>>(),
        )
        .context("failed to create commit")?;

    // now rebase upstream commits, if needed
    let upstream_commits = project_repository.l(
        target_branch.head,
        project_repository::LogUntil::Commit(amend_commit.id()),
    )?;
    // if there are no upstream commits, we're done
    if upstream_commits.is_empty() {
        target_branch.head = commit_oid;
        vb_state.set_branch(target_branch.clone())?;
        super::integration::update_gitbutler_integration(&vb_state, project_repository)?;
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
        super::integration::update_gitbutler_integration(&vb_state, project_repository)?;
        Ok(commit_oid)
    } else {
        Err(errors::VirtualBranchError::RebaseFailed)
    }
}

// move a given commit in a branch up one or down one
// if the offset is positive, move the commit down one
// if the offset is negative, move the commit up one
// rewrites the branch head to the new head commit
pub fn reorder_commit(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    commit_oid: git::Oid,
    offset: i32,
) -> Result<(), errors::VirtualBranchError> {
    let vb_state = project_repository.project().virtual_branches();

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to read default target")?
    else {
        return Err(errors::VirtualBranchError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let mut branch = match vb_state.get_branch(branch_id) {
        Ok(branch) => Ok(branch),
        Err(reader::Error::NotFound) => Err(errors::VirtualBranchError::BranchNotFound(
            errors::BranchNotFound {
                branch_id: *branch_id,
                project_id: project_repository.project().id,
            },
        )),
        Err(error) => Err(errors::VirtualBranchError::Other(error.into())),
    }?;

    // find the commit to offset from
    let commit = project_repository
        .git_repository
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
        let mut ids_to_rebase = project_repository.l(
            branch.head,
            project_repository::LogUntil::Commit(commit.id()),
        )?;
        let last_oid = ids_to_rebase.pop().context("failed to pop last oid")?;
        ids_to_rebase.push(commit_oid);
        ids_to_rebase.push(last_oid);

        match cherry_rebase_group(project_repository, parent_oid, &mut ids_to_rebase) {
            Ok(Some(new_head)) => {
                branch.head = new_head;
                vb_state
                    .set_branch(branch.clone())
                    .context("failed to write branch")?;

                super::integration::update_gitbutler_integration(&vb_state, project_repository)
                    .context("failed to update gitbutler integration")?;
            }
            _ => {
                return Err(errors::VirtualBranchError::RebaseFailed);
            }
        }
    } else {
        //  move commit down
        if default_target.sha == parent_oid {
            // can't move the commit down past the target
            return Ok(());
        }

        let target = parent.parent(0).context("failed to find target")?;
        let target_oid = target.id();

        // get a list of the commits to rebase
        let mut ids_to_rebase = project_repository.l(
            branch.head,
            project_repository::LogUntil::Commit(commit.id()),
        )?;
        ids_to_rebase.push(parent_oid);
        ids_to_rebase.push(commit_oid);

        match cherry_rebase_group(project_repository, target_oid, &mut ids_to_rebase) {
            Ok(Some(new_head)) => {
                branch.head = new_head;
                vb_state
                    .set_branch(branch.clone())
                    .context("failed to write branch")?;

                super::integration::update_gitbutler_integration(&vb_state, project_repository)
                    .context("failed to update gitbutler integration")?;
            }
            _ => {
                return Err(errors::VirtualBranchError::RebaseFailed);
            }
        }
    }

    Ok(())
}

// create and insert a blank commit (no tree change) either above or below a commit
// if offset is positive, insert below, if negative, insert above
// return the oid of the new head commit of the branch with the inserted blank commit
pub fn insert_blank_commit(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    commit_oid: git::Oid,
    user: Option<&users::User>,
    offset: i32,
) -> Result<(), errors::VirtualBranchError> {
    let vb_state = project_repository.project().virtual_branches();

    let mut branch = match vb_state.get_branch(branch_id) {
        Ok(branch) => Ok(branch),
        Err(reader::Error::NotFound) => Err(errors::VirtualBranchError::BranchNotFound(
            errors::BranchNotFound {
                branch_id: *branch_id,
                project_id: project_repository.project().id,
            },
        )),
        Err(error) => Err(errors::VirtualBranchError::Other(error.into())),
    }?;

    // find the commit to offset from
    let mut commit = project_repository
        .git_repository
        .find_commit(commit_oid)
        .context("failed to find commit")?;

    if offset > 0 {
        commit = commit.parent(0).context("failed to find parent")?;
    }

    let commit_tree = commit.tree().unwrap();
    let blank_commit_oid = project_repository.commit(user, "", &commit_tree, &[&commit], None)?;

    if commit.id() == branch.head && offset < 0 {
        // inserting before the first commit
        branch.head = blank_commit_oid;
        vb_state
            .set_branch(branch.clone())
            .context("failed to write branch")?;
        super::integration::update_gitbutler_integration(&vb_state, project_repository)
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
                vb_state
                    .set_branch(branch.clone())
                    .context("failed to write branch")?;

                super::integration::update_gitbutler_integration(&vb_state, project_repository)
                    .context("failed to update gitbutler integration")?;
            }
            _ => {
                return Err(errors::VirtualBranchError::RebaseFailed);
            }
        }
    }

    Ok(())
}

// remove a commit in a branch by rebasing all commits _except_ for it onto it's parent
// if successful, it will update the branch head to the new head commit
pub fn undo_commit(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    commit_oid: git::Oid,
) -> Result<(), errors::VirtualBranchError> {
    let vb_state = project_repository.project().virtual_branches();

    let mut branch = match vb_state.get_branch(branch_id) {
        Ok(branch) => Ok(branch),
        Err(reader::Error::NotFound) => Err(errors::VirtualBranchError::BranchNotFound(
            errors::BranchNotFound {
                branch_id: *branch_id,
                project_id: project_repository.project().id,
            },
        )),
        Err(error) => Err(errors::VirtualBranchError::Other(error.into())),
    }?;

    let commit = project_repository
        .git_repository
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
            _ => {
                return Err(errors::VirtualBranchError::RebaseFailed);
            }
        }
    }

    if new_commit_oid != commit_oid {
        branch.head = new_commit_oid;
        vb_state
            .set_branch(branch.clone())
            .context("failed to write branch")?;

        super::integration::update_gitbutler_integration(&vb_state, project_repository)
            .context("failed to update gitbutler integration")?;
    }

    Ok(())
}

// cherry-pick based rebase, which handles empty commits
// this function takes a commit range and generates a Vector of commit oids
// and then passes them to `cherry_rebase_group` to rebase them onto the target commit
pub fn cherry_rebase(
    project_repository: &project_repository::Repository,
    target_commit_oid: git::Oid,
    start_commit_oid: git::Oid,
    end_commit_oid: git::Oid,
) -> Result<Option<git::Oid>, anyhow::Error> {
    // get a list of the commits to rebase
    let mut ids_to_rebase = project_repository.l(
        end_commit_oid,
        project_repository::LogUntil::Commit(start_commit_oid),
    )?;

    if ids_to_rebase.is_empty() {
        return Ok(None);
    }

    let new_head_id =
        cherry_rebase_group(project_repository, target_commit_oid, &mut ids_to_rebase)?;

    Ok(new_head_id)
}

// takes a vector of commit oids and rebases them onto a target commit and returns the
// new head commit oid if it's successful
// the difference between this and a libgit2 based rebase is that this will successfully
// rebase empty commits (two commits with identical trees)
fn cherry_rebase_group(
    project_repository: &project_repository::Repository,
    target_commit_oid: git::Oid,
    ids_to_rebase: &mut [git::Oid],
) -> Result<Option<git::Oid>, anyhow::Error> {
    ids_to_rebase.reverse();
    // now, rebase unchanged commits onto the new commit
    let commits_to_rebase = ids_to_rebase
        .iter()
        .map(|oid| project_repository.git_repository.find_commit(*oid))
        .collect::<Result<Vec<_>, _>>()
        .context("failed to read commits to rebase")?;

    let new_head_id = commits_to_rebase
        .into_iter()
        .fold(
            project_repository
                .git_repository
                .find_commit(target_commit_oid)
                .context("failed to find new commit"),
            |head, to_rebase| {
                let head = head?;

                let mut cherrypick_index = project_repository
                    .git_repository
                    .cherry_pick(&head, &to_rebase)
                    .context("failed to cherry pick")?;

                if cherrypick_index.has_conflicts() {
                    bail!("failed to rebase");
                }

                let merge_tree_oid = cherrypick_index
                    .write_tree_to(&project_repository.git_repository)
                    .context("failed to write merge tree")?;

                let merge_tree = project_repository
                    .git_repository
                    .find_tree(merge_tree_oid)
                    .context("failed to find merge tree")?;

                let commit_oid = project_repository
                    .git_repository
                    .commit(
                        None,
                        &to_rebase.author(),
                        &to_rebase.committer(),
                        &to_rebase.message().to_str_lossy(),
                        &merge_tree,
                        &[&head],
                    )
                    .context("failed to create commit")?;

                project_repository
                    .git_repository
                    .find_commit(commit_oid)
                    .context("failed to find commit")
            },
        )?
        .id();

    Ok(Some(new_head_id))
}

// runs a simple libgit2 based in-memory rebase on a commit range onto a target commit
// possibly not used in favor of cherry_rebase
pub fn simple_rebase(
    project_repository: &project_repository::Repository,
    target_commit_oid: git::Oid,
    start_commit_oid: git::Oid,
    end_commit_oid: git::Oid,
) -> Result<Option<git::Oid>, anyhow::Error> {
    let repo = &project_repository.git_repository;
    let (_, committer) = project_repository.git_signatures(None)?;

    let mut rebase_options = git2::RebaseOptions::new();
    rebase_options.quiet(true);
    rebase_options.inmemory(true);
    let mut rebase = repo
        .rebase(
            Some(end_commit_oid),
            Some(start_commit_oid),
            Some(target_commit_oid),
            Some(&mut rebase_options),
        )
        .context("failed to rebase")?;

    let mut rebase_success = true;
    // check to see if these commits have already been pushed
    let mut last_rebase_head = target_commit_oid;
    while rebase.next().is_some() {
        let index = rebase
            .inmemory_index()
            .context("failed to get inmemory index")?;
        if index.has_conflicts() {
            rebase_success = false;
            break;
        }

        if let Ok(commit_id) = rebase.commit(None, &committer.clone().into(), None) {
            last_rebase_head = commit_id.into();
        } else {
            rebase_success = false;
            break;
        }
    }

    if rebase_success {
        // rebase worked out, rewrite the branch head
        rebase.finish(None).context("failed to finish rebase")?;
        Ok(Some(last_rebase_head))
    } else {
        // rebase failed, do a merge commit
        rebase.abort().context("failed to abort rebase")?;
        Ok(None)
    }
}

pub fn cherry_pick(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    target_commit_oid: git::Oid,
) -> Result<Option<git::Oid>, errors::CherryPickError> {
    if conflicts::is_conflicting::<&Path>(project_repository, None)? {
        return Err(errors::CherryPickError::Conflict(errors::ProjectConflict {
            project_id: project_repository.project().id,
        }));
    }

    let vb_state = project_repository.project().virtual_branches();

    let mut branch = vb_state
        .get_branch(branch_id)
        .context("failed to read branch")?;

    if !branch.applied {
        // todo?
        return Err(errors::CherryPickError::NotApplied);
    }

    let target_commit = project_repository
        .git_repository
        .find_commit(target_commit_oid)
        .map_err(|error| match error {
            git::Error::NotFound(_) => errors::CherryPickError::CommitNotFound(target_commit_oid),
            error => errors::CherryPickError::Other(error.into()),
        })?;

    let branch_head_commit = project_repository
        .git_repository
        .find_commit(branch.head)
        .context("failed to find branch tree")?;

    let default_target = vb_state
        .try_get_default_target()
        .context("failed to read default target")?
        .context("no default target set")?;

    // if any other branches are applied, unapply them
    let applied_branches = vb_state
        .list_branches()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|b| b.applied)
        .collect::<Vec<_>>();

    let integration_commit_id =
        super::integration::get_workspace_head(&vb_state, project_repository)?;

    let (applied_statuses, _) = get_applied_status(
        project_repository,
        &integration_commit_id,
        &default_target.sha,
        applied_branches,
    )?;

    let branch_files = applied_statuses
        .iter()
        .find(|(b, _)| b.id == *branch_id)
        .map(|(_, f)| f)
        .context("branch status not found")?;

    // create a wip commit. we'll use it to offload cherrypick conflicts calculation to libgit.
    let wip_commit = {
        let wip_tree_oid = write_tree(project_repository, &branch.head, branch_files)?;
        let wip_tree = project_repository
            .git_repository
            .find_tree(wip_tree_oid)
            .context("failed to find tree")?;

        let signature = git::Signature::now("GitButler", "gitbutler@gitbutler.com")
            .context("failed to make gb signature")?;
        let oid = project_repository
            .git_repository
            .commit(
                None,
                &signature,
                &signature,
                "wip cherry picking commit",
                &wip_tree,
                &[&branch_head_commit],
            )
            .context("failed to commit wip work")?;
        project_repository
            .git_repository
            .find_commit(oid)
            .context("failed to find wip commit")?
    };

    let mut cherrypick_index = project_repository
        .git_repository
        .cherry_pick(&wip_commit, &target_commit)
        .context("failed to cherry pick")?;

    // unapply other branches
    for other_branch in applied_statuses
        .iter()
        .filter(|(b, _)| b.id != branch.id)
        .map(|(b, _)| b)
    {
        unapply_branch(project_repository, &other_branch.id).context("failed to unapply branch")?;
    }

    let commit_oid = if cherrypick_index.has_conflicts() {
        // checkout the conflicts
        project_repository
            .git_repository
            .checkout_index(&mut cherrypick_index)
            .allow_conflicts()
            .conflict_style_merge()
            .force()
            .checkout()
            .context("failed to checkout conflicts")?;

        // mark conflicts
        let conflicts = cherrypick_index
            .conflicts()
            .context("failed to get conflicts")?;
        let mut merge_conflicts = Vec::new();
        for path in conflicts.flatten() {
            if let Some(ours) = path.our {
                let path = std::str::from_utf8(&ours.path)
                    .context("failed to convert path")?
                    .to_string();
                merge_conflicts.push(path);
            }
        }
        conflicts::mark(project_repository, &merge_conflicts, Some(branch.head))?;

        None
    } else {
        let merge_tree_oid = cherrypick_index
            .write_tree_to(&project_repository.git_repository)
            .context("failed to write merge tree")?;
        let merge_tree = project_repository
            .git_repository
            .find_tree(merge_tree_oid)
            .context("failed to find merge tree")?;

        let branch_head_commit = project_repository
            .git_repository
            .find_commit(branch.head)
            .context("failed to find branch head commit")?;

        let commit_oid = project_repository
            .git_repository
            .commit(
                None,
                &target_commit.author(),
                &target_commit.committer(),
                &target_commit.message().to_str_lossy(),
                &merge_tree,
                &[&branch_head_commit],
            )
            .context("failed to create commit")?;

        // checkout final_tree into the working directory
        project_repository
            .git_repository
            .checkout_tree(&merge_tree)
            .force()
            .remove_untracked()
            .checkout()
            .context("failed to checkout final tree")?;

        // update branch status
        branch.head = commit_oid;
        vb_state
            .set_branch(branch.clone())
            .context("failed to write branch")?;

        Some(commit_oid)
    };

    super::integration::update_gitbutler_integration(&vb_state, project_repository)
        .context("failed to update gitbutler integration")?;

    Ok(commit_oid)
}

/// squashes a commit from a virtual branch into it's parent.
pub fn squash(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    commit_oid: git::Oid,
) -> Result<(), errors::SquashError> {
    if conflicts::is_conflicting::<&Path>(project_repository, None)? {
        return Err(errors::SquashError::Conflict(errors::ProjectConflict {
            project_id: project_repository.project().id,
        }));
    }

    let vb_state = project_repository.project().virtual_branches();

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to read default target")?
    else {
        return Err(errors::SquashError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let mut branch = vb_state
        .get_branch(branch_id)
        .map_err(|error| match error {
            reader::Error::NotFound => {
                errors::SquashError::BranchNotFound(errors::BranchNotFound {
                    project_id: project_repository.project().id,
                    branch_id: *branch_id,
                })
            }
            error => errors::SquashError::Other(error.into()),
        })?;

    let branch_commit_oids = project_repository.l(
        branch.head,
        project_repository::LogUntil::Commit(default_target.sha),
    )?;

    if !branch_commit_oids.contains(&commit_oid) {
        return Err(errors::SquashError::CommitNotFound(commit_oid));
    }

    let commit_to_squash = project_repository
        .git_repository
        .find_commit(commit_oid)
        .context("failed to find commit")?;

    let parent_commit = commit_to_squash
        .parent(0)
        .context("failed to find parent commit")?;

    let pushed_commit_oids = branch.upstream_head.map_or_else(
        || Ok(vec![]),
        |upstream_head| {
            project_repository.l(
                upstream_head,
                project_repository::LogUntil::Commit(default_target.sha),
            )
        },
    )?;

    if pushed_commit_oids.contains(&parent_commit.id())
        && !project_repository.project().ok_with_force_push
    {
        // squashing into a pushed commit will cause a force push that is not allowed
        return Err(errors::SquashError::ForcePushNotAllowed(
            errors::ForcePushNotAllowed {
                project_id: project_repository.project().id,
            },
        ));
    }

    if !branch_commit_oids.contains(&parent_commit.id()) {
        return Err(errors::SquashError::CantSquashRootCommit);
    }

    // create a commit that:
    //  * has the tree of the target commit
    //  * has the message combined of the target commit and parent commit
    //  * has parents of the parents commit.
    let parents = parent_commit
        .parents()
        .context("failed to find head commit parents")?;

    let new_commit_oid = project_repository
        .git_repository
        .commit(
            None,
            &commit_to_squash.author(),
            &commit_to_squash.committer(),
            &format!(
                "{}\n{}",
                parent_commit.message(),
                commit_to_squash.message(),
            ),
            &commit_to_squash.tree().context("failed to find tree")?,
            &parents.iter().collect::<Vec<_>>(),
        )
        .context("failed to commit")?;

    let ids_to_rebase = {
        let ids = branch_commit_oids
            .split(|oid| oid.eq(&commit_oid))
            .collect::<Vec<_>>();
        ids.first().copied()
    }
    .ok_or(errors::SquashError::CommitNotFound(commit_oid))?;
    let mut ids_to_rebase = ids_to_rebase.to_vec();

    match cherry_rebase_group(project_repository, new_commit_oid, &mut ids_to_rebase) {
        Ok(Some(new_head_id)) => {
            // save new branch head
            branch.head = new_head_id;
            vb_state
                .set_branch(branch.clone())
                .context("failed to write branch")?;

            super::integration::update_gitbutler_integration(&vb_state, project_repository)
                .context("failed to update gitbutler integration")?;
            Ok(())
        }
        _ => Err(errors::SquashError::Other(anyhow!("rebase error"))),
    }
}

// changes a commit message for commit_oid, rebases everything above it, updates branch head if successful
pub fn update_commit_message(
    project_repository: &project_repository::Repository,
    branch_id: &BranchId,
    commit_oid: git::Oid,
    message: &str,
) -> Result<(), errors::UpdateCommitMessageError> {
    if message.is_empty() {
        return Err(errors::UpdateCommitMessageError::EmptyMessage);
    }

    if conflicts::is_conflicting::<&Path>(project_repository, None)? {
        return Err(errors::UpdateCommitMessageError::Conflict(
            errors::ProjectConflict {
                project_id: project_repository.project().id,
            },
        ));
    }

    let vb_state = project_repository.project().virtual_branches();

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to read default target")?
    else {
        return Err(errors::UpdateCommitMessageError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let mut branch = vb_state
        .get_branch(branch_id)
        .map_err(|error| match error {
            reader::Error::NotFound => {
                errors::UpdateCommitMessageError::BranchNotFound(errors::BranchNotFound {
                    project_id: project_repository.project().id,
                    branch_id: *branch_id,
                })
            }
            error => errors::UpdateCommitMessageError::Other(error.into()),
        })?;

    let branch_commit_oids = project_repository.l(
        branch.head,
        project_repository::LogUntil::Commit(default_target.sha),
    )?;

    if !branch_commit_oids.contains(&commit_oid) {
        return Err(errors::UpdateCommitMessageError::CommitNotFound(commit_oid));
    }

    let pushed_commit_oids = branch.upstream_head.map_or_else(
        || Ok(vec![]),
        |upstream_head| {
            project_repository.l(
                upstream_head,
                project_repository::LogUntil::Commit(default_target.sha),
            )
        },
    )?;

    if pushed_commit_oids.contains(&commit_oid) && !project_repository.project().ok_with_force_push
    {
        // updating the message of a pushed commit will cause a force push that is not allowed
        return Err(errors::UpdateCommitMessageError::ForcePushNotAllowed(
            errors::ForcePushNotAllowed {
                project_id: project_repository.project().id,
            },
        ));
    }

    let target_commit = project_repository
        .git_repository
        .find_commit(commit_oid)
        .context("failed to find commit")?;

    let parents = target_commit
        .parents()
        .context("failed to find head commit parents")?;

    let new_commit_oid = project_repository
        .git_repository
        .commit(
            None,
            &target_commit.author(),
            &target_commit.committer(),
            message,
            &target_commit.tree().context("failed to find tree")?,
            &parents.iter().collect::<Vec<_>>(),
        )
        .context("failed to commit")?;

    let ids_to_rebase = {
        let ids = branch_commit_oids
            .split(|oid| oid.eq(&commit_oid))
            .collect::<Vec<_>>();
        ids.first().copied()
    }
    .ok_or(errors::UpdateCommitMessageError::CommitNotFound(commit_oid))?;
    let mut ids_to_rebase = ids_to_rebase.to_vec();

    match cherry_rebase_group(project_repository, new_commit_oid, &mut ids_to_rebase) {
        Ok(Some(new_head_id)) => {
            // save new branch head
            branch.head = new_head_id;
            vb_state
                .set_branch(branch.clone())
                .context("failed to write branch")?;

            super::integration::update_gitbutler_integration(&vb_state, project_repository)
                .context("failed to update gitbutler integration")?;
            Ok(())
        }
        _ => Err(errors::UpdateCommitMessageError::Other(anyhow!(
            "rebase error"
        ))),
    }
}

/// moves commit from the branch it's in to the top of the target branch
pub fn move_commit(
    project_repository: &project_repository::Repository,
    target_branch_id: &BranchId,
    commit_oid: git::Oid,
    user: Option<&users::User>,
    signing_key: Option<&keys::PrivateKey>,
) -> Result<(), errors::MoveCommitError> {
    if project_repository.is_resolving() {
        return Err(errors::MoveCommitError::Conflicted(
            errors::ProjectConflict {
                project_id: project_repository.project().id,
            },
        ));
    }

    let vb_state = project_repository.project().virtual_branches();

    let applied_branches = vb_state
        .list_branches()
        .context("failed to read virtual branches")?
        .into_iter()
        .filter(|b| b.applied)
        .collect::<Vec<_>>();

    if !applied_branches.iter().any(|b| b.id == *target_branch_id) {
        return Err(errors::MoveCommitError::BranchNotFound(
            errors::BranchNotFound {
                project_id: project_repository.project().id,
                branch_id: *target_branch_id,
            },
        ));
    }

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to get default target")?
    else {
        return Err(errors::MoveCommitError::DefaultTargetNotSet(
            errors::DefaultTargetNotSet {
                project_id: project_repository.project().id,
            },
        ));
    };

    let integration_commit_id =
        super::integration::get_workspace_head(&vb_state, project_repository)?;

    let (mut applied_statuses, _) = get_applied_status(
        project_repository,
        &integration_commit_id,
        &default_target.sha,
        applied_branches,
    )?;

    let (ref mut source_branch, source_status) = applied_statuses
        .iter_mut()
        .find(|(b, _)| b.head == commit_oid)
        .ok_or_else(|| errors::MoveCommitError::CommitNotFound(commit_oid))?;

    let source_branch_non_comitted_files = source_status;

    let source_branch_head = project_repository
        .git_repository
        .find_commit(commit_oid)
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
    let branch_head_diff = diff::trees(
        &project_repository.git_repository,
        &source_branch_head_parent_tree,
        &source_branch_head_tree,
    )?;

    let branch_head_diff: HashMap<_, _> = diff::diff_files_into_hunks(branch_head_diff).collect();
    let is_source_locked = source_branch_non_comitted_files
        .iter()
        .any(|(path, hunks)| {
            branch_head_diff.get(path).map_or(false, |head_diff_hunks| {
                hunks.iter().any(|hunk| {
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
        return Err(errors::MoveCommitError::SourceLocked);
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
        let mut destination_branch =
            vb_state
                .get_branch(target_branch_id)
                .map_err(|error| match error {
                    reader::Error::NotFound => {
                        errors::MoveCommitError::BranchNotFound(errors::BranchNotFound {
                            project_id: project_repository.project().id,
                            branch_id: *target_branch_id,
                        })
                    }
                    error => errors::MoveCommitError::Other(error.into()),
                })?;

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
            .git_repository
            .find_tree(new_destination_tree_oid)
            .context("failed to find tree")?;

        let new_destination_head_oid = project_repository
            .commit(
                user,
                &source_branch_head.message().to_str_lossy(),
                &new_destination_tree,
                &[&project_repository
                    .git_repository
                    .find_commit(destination_branch.head)
                    .context("failed to get dst branch head commit")?],
                signing_key,
            )
            .context("failed to commit")?;

        destination_branch.head = new_destination_head_oid;
        vb_state.set_branch(destination_branch.clone())?;
    }

    super::integration::update_gitbutler_integration(&vb_state, project_repository)
        .context("failed to update gitbutler integration")?;

    Ok(())
}

pub fn create_virtual_branch_from_branch(
    project_repository: &project_repository::Repository,
    upstream: &git::Refname,
    signing_key: Option<&keys::PrivateKey>,
    user: Option<&users::User>,
) -> Result<BranchId, errors::CreateVirtualBranchFromBranchError> {
    if !matches!(upstream, git::Refname::Local(_) | git::Refname::Remote(_)) {
        return Err(errors::CreateVirtualBranchFromBranchError::BranchNotFound(
            upstream.clone(),
        ));
    }

    let vb_state = project_repository.project().virtual_branches();

    let Some(default_target) = vb_state
        .try_get_default_target()
        .context("failed to get default target")?
    else {
        return Err(
            errors::CreateVirtualBranchFromBranchError::DefaultTargetNotSet(
                errors::DefaultTargetNotSet {
                    project_id: project_repository.project().id,
                },
            ),
        );
    };

    if let git::Refname::Remote(remote_upstream) = upstream {
        if default_target.branch.eq(remote_upstream) {
            return Err(
                errors::CreateVirtualBranchFromBranchError::CantMakeBranchFromDefaultTarget,
            );
        }
    }

    let repo = &project_repository.git_repository;
    let head_reference = match repo.find_reference(upstream) {
        Ok(head) => Ok(head),
        Err(git::Error::NotFound(_)) => Err(
            errors::CreateVirtualBranchFromBranchError::BranchNotFound(upstream.clone()),
        ),
        Err(error) => Err(errors::CreateVirtualBranchFromBranchError::Other(
            error.into(),
        )),
    }?;
    let head_commit = head_reference
        .peel_to_commit()
        .context("failed to peel to commit")?;
    let head_commit_tree = head_commit.tree().context("failed to find tree")?;

    let all_virtual_branches = vb_state
        .list_branches()
        .context("failed to read virtual branches")?
        .into_iter()
        .collect::<Vec<branch::Branch>>();

    let order = all_virtual_branches.len();

    let selected_for_changes = (!all_virtual_branches
        .iter()
        .any(|b| b.selected_for_changes.is_some()))
    .then_some(chrono::Utc::now().timestamp_millis());

    let now = crate::time::now_ms();

    // only set upstream if it's not the default target
    let upstream_branch = match upstream {
        git::Refname::Other(_) | git::Refname::Virtual(_) => {
            // we only support local or remote branches
            return Err(errors::CreateVirtualBranchFromBranchError::BranchNotFound(
                upstream.clone(),
            ));
        }
        git::Refname::Remote(remote) => Some(remote.clone()),
        git::Refname::Local(local) => local.remote().cloned(),
    };

    // add file ownership based off the diff
    let target_commit = repo
        .find_commit(default_target.sha)
        .map_err(|error| errors::CreateVirtualBranchFromBranchError::Other(error.into()))?;
    let merge_base_oid = repo
        .merge_base(target_commit.id(), head_commit.id())
        .map_err(|error| errors::CreateVirtualBranchFromBranchError::Other(error.into()))?;
    let merge_base_tree = repo
        .find_commit(merge_base_oid)
        .map_err(|error| errors::CreateVirtualBranchFromBranchError::Other(error.into()))?
        .tree()
        .map_err(|error| errors::CreateVirtualBranchFromBranchError::Other(error.into()))?;

    // do a diff between the head of this branch and the target base
    let diff = diff::trees(
        &project_repository.git_repository,
        &merge_base_tree,
        &head_commit_tree,
    )
    .context("failed to diff trees")?;

    // assign ownership to the branch
    let ownership = diff.iter().fold(
        branch::BranchOwnershipClaims::default(),
        |mut ownership, (file_path, file)| {
            for hunk in &file.hunks {
                ownership.put(
                    format!(
                        "{}:{}",
                        file_path.display(),
                        VirtualBranchHunk::gen_id(hunk.new_start, hunk.new_lines)
                    )
                    .parse()
                    .unwrap(),
                );
            }
            ownership
        },
    );

    let branch = branch::Branch {
        id: BranchId::generate(),
        name: upstream
            .branch()
            .expect("always a branch reference")
            .to_string(),
        notes: String::new(),
        applied: false,
        upstream_head: upstream_branch.is_some().then_some(head_commit.id()),
        upstream: upstream_branch,
        tree: head_commit_tree.id(),
        head: head_commit.id(),
        created_timestamp_ms: now,
        updated_timestamp_ms: now,
        ownership,
        order,
        selected_for_changes,
    };

    vb_state
        .set_branch(branch.clone())
        .context("failed to write branch")?;

    project_repository.add_branch_reference(&branch)?;

    match apply_branch(project_repository, &branch.id, signing_key, user) {
        Ok(()) => Ok(branch.id),
        Err(errors::ApplyBranchError::BranchConflicts(_)) => {
            // if branch conflicts with the workspace, it's ok. keep it unapplied
            Ok(branch.id)
        }
        Err(error) => Err(errors::CreateVirtualBranchFromBranchError::ApplyBranch(
            error,
        )),
    }
}

/// Just like [`diffy::apply()`], but on error it will attach hashes of the input `base_image` and `patch`.
pub fn apply<S: AsRef<[u8]>>(base_image: S, patch: &Patch<'_, [u8]>) -> Result<BString> {
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
    project_repository: &project_repository::Repository,
    files: &HashMap<PathBuf, Vec<GitHunk>>,
) -> Result<()> {
    let conflicting_files = conflicts::conflicting_files(project_repository)?;
    for (file_path, non_commited_hunks) in files {
        let mut conflicted = false;
        if conflicting_files.contains(&file_path.display().to_string()) {
            // check file for conflict markers, resolve the file if there are none in any hunk
            for hunk in non_commited_hunks {
                if hunk.diff_lines.contains_str(b"<<<<<<< ours") {
                    conflicted = true;
                }
                if hunk.diff_lines.contains_str(b">>>>>>> theirs") {
                    conflicted = true;
                }
            }
            if !conflicted {
                conflicts::resolve(project_repository, &file_path.display().to_string()).unwrap();
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
        assert_eq!(normalize_branch_name("foo!branch"), "foo-branch");
    }
}
