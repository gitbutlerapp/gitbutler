//! The machinery used to alter and mutate commits in various ways whilst adjusting descendant commits within a [reference frame](ReferenceFrame).

use std::path::Path;

use anyhow::{Context as _, bail};
use bstr::BString;
use but_core::{DiffSpec, ref_metadata::StackId, tree::create_tree::RejectionReason};
use but_ctx::{Context, access::WorktreeWritePermission};
use but_rebase::merge::ConflictErrorContext;
use gitbutler_stack::{VirtualBranchesHandle, VirtualBranchesState};
use gix::{prelude::ObjectIdExt, refs::transaction::PreviousValue};

use crate::{
    WorkspaceCommit,
    commit_engine::{CreateCommitOutcome, Destination, StackSegmentId, create_commit},
    legacy::commit_engine::reference_frame::InferenceMode,
};

pub(super) mod index;
/// Utility types
pub mod reference_frame;
mod refs;

/// All information to know where in the commit-graph the rewritten commit is located to figure out
/// which descendant commits to rewrite.
#[derive(Debug, Clone, Default)]
pub struct ReferenceFrame {
    /// The commit that merges all stacks together for a unified view.
    /// It includes the `branch_tip` naturally, if provided.
    pub workspace_tip: Option<gix::ObjectId>,
    /// If given, and if rebases are necessary, this is the tip (top-most commit) whose ancestry contains
    /// the commit which is committed onto, or amended to.
    ///
    /// Note that in case of moves, right now the source *and* destination need to be contained.
    pub branch_tip: Option<gix::ObjectId>,
}

/// Less pure but a simpler version of [`create_commit_and_update_refs_with_project`]
pub fn create_commit_simple(
    ctx: &Context,
    stack_id: StackId,
    parent_id: Option<gix::ObjectId>,
    worktree_changes: Vec<DiffSpec>,
    message: String,
    stack_branch_name: String,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<CreateCommitOutcome> {
    let repo = ctx.clone_repo_for_merging()?;
    // If parent_id was not set but a stack branch name was provided, pick the current head of that branch as parent.
    let parent_commit_id: Option<gix::ObjectId> = match parent_id {
        Some(id) => Some(id),
        None => {
            let state = VirtualBranchesHandle::new(ctx.project_data_dir());
            let stack = state.get_stack(stack_id)?;
            if !stack.heads(true).contains(&stack_branch_name) {
                return Err(anyhow::anyhow!(
                    "Stack {stack_id} does not have branch {stack_branch_name}"
                ));
            }
            let reference = repo
                .try_find_reference(&stack_branch_name)
                .map_err(anyhow::Error::from)?;
            if let Some(mut r) = reference {
                Some(r.peel_to_commit().map_err(anyhow::Error::from)?.id)
            } else {
                return Err(anyhow::anyhow!("No branch {stack_branch_name} found"));
            }
        }
    };
    let outcome = create_commit_and_update_refs_with_project(
        &repo,
        &ctx.project_data_dir(),
        Some(stack_id),
        Destination::NewCommit {
            parent_commit_id,
            message: message.clone(),
            stack_segment: Some(StackSegmentId {
                stack_id,
                segment_ref: format!("refs/heads/{stack_branch_name}")
                    .try_into()
                    .map_err(anyhow::Error::from)?,
            }),
        },
        worktree_changes,
        ctx.settings().context_lines,
        perm,
    );

    let outcome = outcome?;
    if !outcome.rejected_specs.is_empty() {
        tracing::warn!(?outcome.rejected_specs, "Failed to commit at least one hunk");
    }
    Ok(outcome)
}

/// Like [`create_commit()`], but allows to also update virtual branches and git references pointing to commits
/// after rebasing all descendants, along with re-merging possible workspace merge commits.
///
/// `frame` contains the virtual branches to be modified to point to the rebased versions of commits, but also to inform
/// about the available stacks in the workspace and helps to find the stack that contains the affects commits.
///
/// `vb` is a snapshot of all virtual branches we have to possibly rewrite. They also inform about the stacks active
/// in the workspace, if any.
///
/// Note that conflicts that occur during the rebase will be swallowed, putting the commit into a conflicted state.
/// Finally, the index will be written so it matches the `HEAD^{commit}` *if* there were worktree changes.
///
/// An index is produced so it matches the tree at `HEAD^{tree}` while keeping all content already present a possibly
/// existing `.git/index`. Updated or added files will have their `stat` forcefully updated.
///
/// ### Performance Note
///
/// As commit traversals will be performed for better performance, an
/// [object cache](gix::Repository::object_cache_size_if_unset()) should be configured.
pub fn create_commit_and_update_refs(
    repo: &gix::Repository,
    frame: ReferenceFrame,
    vb: &mut VirtualBranchesState,
    destination: Destination,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<CreateCommitOutcome> {
    let mut out = create_commit(repo, destination.clone(), changes.clone(), context_lines)?;

    let Some(new_commit) = out.new_commit else {
        return Ok(out);
    };

    let (commit_to_find, is_amend) = match destination {
        Destination::NewCommit {
            parent_commit_id, ..
        } => (parent_commit_id, false),
        Destination::AmendCommit { commit_id, .. } => (Some(commit_id), true),
    };

    if let Some(commit_in_graph) = commit_to_find {
        let mut all_refs_by_id = gix::hashtable::HashMap::<_, Vec<_>>::default();
        let mut checked_out_ref_name = None;
        let checked_out_ref = repo.head_ref()?.and_then(|mut r| {
            let id = r.peel_to_id().ok()?.detach();
            checked_out_ref_name = Some(r.inner.name.clone());
            Some((id, r.inner.name))
        });
        let (platform_storage, platform_storage_2);
        let checked_out_and_gitbutler_refs =
            checked_out_ref
                .into_iter()
                // TODO: remove this as `refs/gitbutler/` won't contain relevant refs anymore.
                .chain({
                    platform_storage_2 = repo.references()?;
                    platform_storage_2
                        .prefixed("refs/gitbutler/")?
                        .filter_map(Result::ok)
                        .filter_map(|r| r.try_id().map(|id| (id.detach(), r.inner.name)))
                })
                .chain(
                    // When amending, we want to update all branches that pointed to the old commit to now point to the new commit.
                    if is_amend {
                        Box::new({
                            platform_storage = repo.references()?;
                            platform_storage
                                .prefixed("refs/heads/")?
                                .filter_map(Result::ok)
                                .filter_map(|r| {
                                    let is_checked_out = checked_out_ref_name.as_ref().is_some_and(
                                        |checked_out_ref| checked_out_ref == &r.inner.name,
                                    );
                                    if is_checked_out {
                                        None
                                    } else {
                                        r.try_id().map(|id| (id.detach(), r.inner.name))
                                    }
                                })
                        }) as Box<dyn Iterator<Item = _>>
                    } else {
                        Box::new(std::iter::empty())
                    },
                );
        for (commit_id, git_reference) in checked_out_and_gitbutler_refs {
            all_refs_by_id
                .entry(commit_id)
                .or_default()
                .push(git_reference);
        }

        // Special case: commit/amend on top of `HEAD` and no merge above: no rebase necessary
        if frame.workspace_tip.is_none()
            && repo.head_id().ok().map(|id| id.detach()) == Some(commit_in_graph)
            && all_refs_by_id.contains_key(&commit_in_graph)
        {
            refs::rewrite(
                repo,
                vb,
                all_refs_by_id,
                [(commit_in_graph, new_commit)],
                &mut out.references,
                destination.stack_segment(),
            )?;
        } else {
            let Some(branch_tip) = frame.branch_tip else {
                bail!(
                    "HEAD isn't connected to the affected commit and a rebase is necessary, but no branch tip was provided"
                );
            };
            // Use the branch tip to find all commits leading up to the one that was affected
            // - these are the commits to rebase.
            let mut found_marker = false;
            let commits_to_rebase: Vec<_> = branch_tip
                .attach(repo)
                .ancestors()
                .first_parent_only()
                .all()?
                .filter_map(Result::ok)
                .take_while(|info| {
                    if info.id == commit_in_graph {
                        found_marker = true;
                        false
                    } else {
                        true
                    }
                })
                .map(|info| info.id)
                .collect();
            if !found_marker {
                bail!(
                    "Branch tip at {branch_tip} didn't contain the affected commit {commit_in_graph} - cannot rebase"
                );
            }

            let workspace_tip = frame
                .workspace_tip
                .filter(|ws_tip| !commits_to_rebase.contains(ws_tip));
            let rebase = {
                fn conflicts_to_specs(
                    outcome: &mut CreateCommitOutcome,
                    conflicts: &[BString],
                    changes: &[DiffSpec],
                ) -> anyhow::Result<()> {
                    outcome
                        .rejected_specs
                        .extend(conflicts.iter().map(|conflicting_rela_path| {
                            changes
                                .iter()
                                .find_map(|spec| {
                                    (spec.path == *conflicting_rela_path
                                        || spec.previous_path.as_ref()
                                            == Some(conflicting_rela_path))
                                    .then_some((
                                        RejectionReason::WorkspaceMergeConflict,
                                        spec.to_owned(),
                                    ))
                                })
                                .unwrap_or_else(|| {
                                    (
                                        RejectionReason::WorkspaceMergeConflictOfUnrelatedFile,
                                        DiffSpec {
                                            previous_path: None,
                                            path: conflicting_rela_path.to_owned(),
                                            hunk_headers: vec![],
                                        },
                                    )
                                })
                        }));
                    outcome.new_commit = None;
                    outcome.changed_tree_pre_cherry_pick = None;
                    Ok(())
                }
                // Set commits leading up to the tip on top of the new commit, serving as base.
                let mut builder = but_rebase::Rebase::new(repo, new_commit, Some(commit_in_graph))?;
                builder.steps(commits_to_rebase.into_iter().rev().map(|commit_id| {
                    but_rebase::RebaseStep::Pick {
                        commit_id,
                        new_message: None,
                    }
                }))?;
                if let Some(workspace_tip) = workspace_tip {
                    // Special Hack (https://github.com/gitbutlerapp/gitbutler/pull/7976)
                    // See if `branch_tip` isn't yet in the workspace-tip if it is managed, and if so, add it
                    // so it's going to be re-merged.
                    let wsc = WorkspaceCommit::from_id(workspace_tip.attach(repo))?;
                    let commit_id = if wsc.is_managed() /* we can change the commit */
                        && !wsc.inner.parents.contains(&branch_tip) /* the branch tip we know isn't yet merged */
                        // but the tip is known to the workspace
                        && vb.branches.values().any(|s| {
                        s.head_oid(repo)
                            .is_ok_and(|head_id| head_id == branch_tip)
                    }) {
                        let mut stacks: Vec<_> = vb
                            .branches
                            .values()
                            .filter(|stack| stack.in_workspace)
                            .map(|stack| crate::legacy::ui::StackEntryNoOpt::try_new(repo, stack))
                            .collect::<Result<_, _>>()?;
                        stacks.sort_by(|a, b| a.name().cmp(&b.name()));
                        let new_wc = WorkspaceCommit::new_from_stacks(stacks, repo.object_hash());
                        repo.write_object(&new_wc)?.detach()
                    } else {
                        workspace_tip
                    };
                    // We can assume the workspace tip is connected to a pick (or else the rebase will fail)
                    builder.steps([but_rebase::RebaseStep::Pick {
                        commit_id,
                        new_message: None,
                    }])?;
                    match builder.rebase() {
                        Ok(mut outcome) => {
                            if commit_id != workspace_tip {
                                let Some(rewritten_old) =
                                    outcome.commit_mapping.iter_mut().find_map(
                                        |(_base, old, _new)| (old == &commit_id).then_some(old),
                                    )
                                else {
                                    bail!(
                                        "BUG: Needed to find modified {commit_id} to set it back its previous value, but couldn't find it"
                                    );
                                };
                                *rewritten_old = workspace_tip;
                            }
                            outcome
                        }
                        Err(err) => {
                            return if let Some(conflicts) =
                                err.downcast_ref::<ConflictErrorContext>()
                            {
                                conflicts_to_specs(&mut out, &conflicts.paths, &changes)?;
                                Ok(out)
                            } else {
                                Err(err)
                            };
                        }
                    }
                } else {
                    match builder.rebase() {
                        Ok(rebase) => rebase,
                        Err(err) => {
                            return if let Some(conflicts) =
                                err.downcast_ref::<ConflictErrorContext>()
                            {
                                conflicts_to_specs(&mut out, &conflicts.paths, &changes)?;
                                Ok(out)
                            } else {
                                Err(err)
                            };
                        }
                    }
                }
            };

            crate::legacy::commit_engine::refs::rewrite(
                repo,
                vb,
                all_refs_by_id,
                rebase
                    .commit_mapping
                    .iter()
                    .map(|(_base, old, new)| (*old, *new))
                    .chain(Some((commit_in_graph, new_commit))),
                &mut out.references,
                destination.stack_segment(),
            )?;
            out.rebase_output = Some(rebase);
        }
        // Assume an index to be present and adjust it to match the new tree.

        let tree_index = repo.index_from_tree(&repo.head_tree_id()?)?;
        let mut disk_index = repo.open_index()?;
        index::apply_lhs_to_rhs(
            repo.workdir().expect("non-bare"),
            &tree_index,
            &mut disk_index,
        )?;
        out.index = disk_index.into();
    } else {
        // unborn branch special case.
        repo.reference(
            repo.head_name()?
                .context("unborn HEAD must contain a ref-name")?,
            new_commit,
            PreviousValue::Any,
            format!(
                "commit (initial): {title}",
                title = new_commit
                    .attach(repo)
                    .object()?
                    .into_commit()
                    .message()?
                    .title
            ),
        )?;
        let new_tree = new_commit.attach(repo).object()?.into_commit().tree_id()?;
        out.index = repo.index_from_tree(&new_tree)?.into();
    }

    if let Some(index) = out.index.as_mut() {
        index.write(Default::default())?;
    }
    Ok(out)
}

/// Like [`create_commit_and_update_refs()`], but integrates with an existing GitButler `project`
/// if present. Alternatively, it uses the current `HEAD` as only reference point.
/// Note that virtual branches will be updated and written back after this call, which will obtain
/// an exclusive workspace lock as well.
pub fn create_commit_and_update_refs_with_project(
    repo: &gix::Repository,
    project_data_dir: &Path,
    maybe_stackid: Option<StackId>,
    destination: Destination,
    changes: Vec<DiffSpec>,
    context_lines: u32,
    _perm: &mut WorktreeWritePermission,
) -> anyhow::Result<CreateCommitOutcome> {
    let vbh = VirtualBranchesHandle::new(project_data_dir);
    let mut vb = vbh.read_file()?;
    let frame = match maybe_stackid {
        None => {
            let (maybe_commit_id, maybe_stack_id) = match &destination {
                Destination::NewCommit {
                    parent_commit_id,
                    stack_segment,
                    ..
                } => (*parent_commit_id, stack_segment.clone().map(|s| s.stack_id)),
                Destination::AmendCommit { commit_id, .. } => (Some(*commit_id), None),
            };

            match (maybe_commit_id, maybe_stack_id) {
                (None, None) => ReferenceFrame::default(),
                (_, Some(stack_id)) => {
                    ReferenceFrame::infer(repo, &vb, InferenceMode::StackId(stack_id))?
                }
                (Some(commit_id), None) => {
                    ReferenceFrame::infer(repo, &vb, InferenceMode::CommitIdInStack(commit_id))?
                }
            }
        }
        Some(stack_id) => ReferenceFrame::infer(repo, &vb, InferenceMode::StackId(stack_id))?,
    };
    let out =
        create_commit_and_update_refs(repo, frame, &mut vb, destination, changes, context_lines)?;

    vbh.write_file(&vb)?;
    Ok(out)
}

impl Destination {
    pub(crate) fn stack_segment(&self) -> Option<&StackSegmentId> {
        match self {
            Destination::NewCommit { stack_segment, .. } => stack_segment.as_ref(),
            Destination::AmendCommit { .. } => None,
        }
    }
}
