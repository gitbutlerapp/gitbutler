//! The machinery used to alter and mutate commits in various ways whilst adjusting descendant commits within a [reference frame](ReferenceFrame).

use anyhow::{Context, bail};
use bstr::BString;
use but_core::RepositoryExt;
use but_rebase::{RebaseOutput, commit::DateMode, merge::ConflictErrorContext};
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::{StackId, VirtualBranchesHandle, VirtualBranchesState};
use gix::{prelude::ObjectIdExt as _, refs::transaction::PreviousValue};

use crate::{DiffSpec, commit_engine::reference_frame::InferenceMode};

pub(crate) mod tree;
use tree::{CreateTreeOutcome, create_tree};

pub(crate) mod index;
/// Utility types
pub mod reference_frame;
mod refs;

mod hunks;
pub use hunks::apply_hunks;
pub use tree::apply_worktree_changes;

use crate::WorkspaceCommit;

/// Types for use in the frontend with serialization support.
pub mod ui;

/// The place to apply the [change-specifications](DiffSpec) to.
///
/// Note that any commit this instance points to will be the basis to apply all changes to.
#[derive(Debug, Clone)]
pub enum Destination {
    /// Create a new commit on top of the given `parent_commit_id`, so it will be the sole parent
    /// of the newly created commit, making it its ancestor.
    NewCommit {
        /// If `None`, the base-state for the new commit will be an empty tree and the new commit will be the first one
        /// (i.e. have no parent). This is the case when `HEAD` is unborn. If `HEAD` is detached, this is a failure.
        ///
        /// To create a commit at the position of the first commit of a branch, the parent has to be the merge-base with the *target branch*.
        parent_commit_id: Option<gix::ObjectId>,
        /// The stack and reference the commit is supposed to go into. It is necessary to disambiguate the reference update.
        stack_segment: Option<StackSegmentId>,
        /// Use `message` as a commit message for the new commit.
        message: String,
    },
    /// Amend all changes to the given commit, leaving all other aspects of the commit unchanged.
    AmendCommit {
        /// The commit to use as a base to amend to. It will be rewritten, retaining its parents.
        commit_id: gix::ObjectId,
        /// If `Some()`, set the commit message as well.
        new_message: Option<String>,
    },
}

/// The stack and the branch the commit is supposed to go into.
#[derive(Debug, Clone)]
pub struct StackSegmentId {
    /// Identifies the stack the commit is destined to (without it, ambiguity is still possible, e.g. when all stacks have no commits)
    pub stack_id: StackId,
    /// The name of the ref pointing to the tip of the stack segment the commit is supposed to go into. It is necessary to disambiguate the reference update.
    pub segment_ref: gix::refs::FullName,
}

impl Destination {
    pub(self) fn stack_segment(&self) -> Option<&StackSegmentId> {
        match self {
            Destination::NewCommit { stack_segment, .. } => stack_segment.as_ref(),
            Destination::AmendCommit { .. } => None,
        }
    }
}

/// Identify the commit that contains the patches to be moved, along with the branch that should be rewritten.
#[derive(Debug, Clone, Copy)]
pub struct MoveSourceCommit {
    /// The commit that acts as the source of all changes. Note that these changes will be *removed* from the
    /// commit, which gets rewritten in the process.
    pub commit_id: gix::ObjectId,
    /// The commit at the *very top* of the branch which has the commit that acts as source of changes in its ancestry.
    pub branch_tip: gix::ObjectId,
}

/// The range of a hunk as denoted by a 1-based starting line, and the amount of lines from there.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct HunkRange {
    /// The number of the first line in the hunk, 1 based.
    pub start: u32,
    /// The amount of lines in the range.
    ///
    /// If `0`, this is an empty hunk.
    pub lines: u32,
}

/// A type used in [`CreateCommitOutcome`] to indicate how a reference was changed so it keeps pointing
/// to the correct commit.
#[derive(Debug, PartialEq)]
pub struct UpdatedReference {
    /// The reference itself.
    // TODO: The virtual variant could contain stack-id as well, but it remains to be seen how useful this is.
    reference: but_core::Reference,
    /// The commit to which `reference` pointed before the update.
    old_commit_id: gix::ObjectId,
    /// The commit to which `reference` points now.
    new_commit_id: gix::ObjectId,
}

/// Additional information about the outcome of a [`create_commit()`] call.
#[derive(Debug)]
pub struct CreateCommitOutcome {
    /// Changes that were removed from a commit because they caused conflicts when rebasing dependent commits,
    /// when merging the workspace commit, or because the specified hunks didn't match exactly due to changes
    /// that happened in the meantime, or if a file without a change was specified.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
    /// The newly created commit, or `None` if no commit could be created as all changes-requests were rejected.
    pub new_commit: Option<gix::ObjectId>,
    /// If `new_commit` is `Some(_)`, this field is `Some(_)` as well and denotes the base-tree + all changes.
    /// If the applied changes were from the worktree, it's `HEAD^{tree}` + changes.
    /// Otherwise, it's `<commit>^{tree}` + changes.
    pub changed_tree_pre_cherry_pick: Option<gix::ObjectId>,
    /// The rewritten references `(old, new, reference)`, along with their `old` and `new` commit location, along
    /// with the reference itself.
    /// If `new_commit` is `None`, this array will be an empty.
    pub references: Vec<UpdatedReference>,
    /// `Some(_)` if a rebase was performed.
    pub rebase_output: Option<RebaseOutput>,
    /// An index based on the existing index on disk that matches *the tree at `HEAD`*.
    /// Note that a couple of extensions that relate to paths will have been dropped to assure consistency - we don't have
    /// `unpack_trees` just yet.
    /// The index wasn't written yet, but could be to match `HEAD^{commit}`.
    pub index: Option<gix::index::File>,
}

/// Provide a description of why a [`DiffSpec`] was rejected for application to the tree of a commit.
#[derive(Default, Debug, Copy, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RejectionReason {
    /// All changes were applied, but they didn't end up effectively change the tree to something differing from the target tree.
    /// This means the changes were a no-op.
    /// Is that even possible? The code says so, for good measure.
    // We don't really have a default, this is just for convenience
    #[default]
    NoEffectiveChanges,
    /// The final cherry-pick to bring the new tree down onto the target tree (merge it in) failed with a conflict.
    CherryPickMergeConflict,
    /// The final merge of the workspace commit failed with a conflict.
    WorkspaceMergeConflict,
    /// The final merge of the workspace commit failed with a conflict,
    /// but the involved file wasn't anything the user provided as diff-spec.
    WorkspaceMergeConflictOfUnrelatedFile,
    /// This is just a theoretical possibility that *could* happen if somebody deletes a file that was there before *right after* we checked its
    /// metadata and found that it still exists.
    /// So if you see this, you could also have won the lottery.
    WorktreeFileMissingForObjectConversion,
    /// When performing a unified diff, it had to refused as the file was too large or turned out to be binary.
    /// Note that this only happens for binary files if there is no `diff.<name>.textconv` filters configured.
    FileToLargeOrBinary,
    /// A change with multiple hunks to be applied wasn't present in the base-tree.
    /// Previously this was possible when untracked files were added with their single hunk specified, but now this shouldn't be happening anymore.
    PathNotFoundInBaseTree,
    /// There was a change, but the path pointed to something that wasn't a file or a link.
    /// You would see this if also in case of submodules or repositories to be added with hunks, which shouldn't be easy to do accidentally even.
    UnsupportedDirectoryEntry,
    /// The base version of a file to apply worktree changes to as present in a Git tree had an undiffable entry type.
    /// This can happen if the target tree has an entry that isn't of the same type as the source worktree changes.
    UnsupportedTreeEntry,
    /// The DiffSpec points to an actual change, or a subset of that change using a file path and optionally hunks into that file.
    /// However, at least one hunk was not fully contained.
    MissingDiffSpecAssociation,
}

/// Alter the single `destination` in a given `frame` with as many `changes` as possible and write new objects into `repo`,
/// but only if the commit succeeds.
///
/// If `move_source` is `Some(source)`, all changes are considered to originate from the given commit to move out of, otherwise they originate from the worktree.
/// `context_lines` is the amount of lines of context included in each [`HunkHeader`], and the value that will be used to recover the existing hunks,
/// so that the hunks can be matched.
///
/// Return additional information that helps to understand to what extent the commit was created, as the commit might not contain all the [`DiffSpecs`](DiffSpec)
/// that were requested if they failed to apply.
///
/// Note that no [`index`](CreateCommitOutcome::index) is produced here as the `HEAD` isn't queried and doesn't play a role.
///
/// No reference is touched in the process.
///
/// ### Hunk-based discarding
///
/// When an instance in `changes` contains hunks, these are the hunks to be committed. If they match a whole hunk in the worktree changes,
/// it will be committed in its entirety.
///
/// ### Sub-Hunk discarding
///
/// It's possible to specify ranges of hunks to discard. To do that, they need an *anchor*. The *anchor* is the pair of
/// `(line_number, line_count)` that should not be changed, paired with the *other* pair with the new `(line_number, line_count)`
/// to discard.
///
/// For instance, when there is a single patch `-1,10 +1,10` and we want to commit the removed 5th line *and* the added 5th line,
/// we'd specify *just* two selections, one in the old via `-5,1 +1,10` and one in the new via `-1,10 +5,1`.
/// This works because internally, it will always match the hunks (and sub-hunks) with their respective pairs obtained through a
/// worktree status, using the anchor, and apply an additional processing step to get the actual old/new hunk pairs to use when
/// building the buffer to commit.
pub fn create_commit(
    repo: &gix::Repository,
    destination: Destination,
    move_source: Option<MoveSourceCommit>,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<CreateCommitOutcome> {
    let parents = match &destination {
        Destination::NewCommit {
            parent_commit_id: None,
            ..
        } => Vec::new(),
        Destination::NewCommit {
            parent_commit_id: Some(parent),
            ..
        } => vec![*parent],
        Destination::AmendCommit { commit_id, .. } => commit_id
            .attach(repo)
            .object()?
            .peel_to_commit()?
            .parent_ids()
            .map(|id| id.detach())
            .collect(),
    };

    if !matches!(destination, Destination::AmendCommit { .. }) && parents.len() > 1 {
        bail!("cannot currently handle more than 1 parent")
    }

    let target_tree = match &destination {
        Destination::NewCommit {
            parent_commit_id: None,
            ..
        } => gix::ObjectId::empty_tree(repo.object_hash()),
        Destination::NewCommit {
            parent_commit_id: Some(base_commit),
            ..
        }
        | Destination::AmendCommit {
            commit_id: base_commit,
            ..
        } => but_core::Commit::from_id(base_commit.attach(repo))?
            .tree_id_or_auto_resolution()?
            .detach(),
    };

    let CreateTreeOutcome {
        rejected_specs,
        destination_tree,
        changed_tree_pre_cherry_pick,
    } = create_tree(repo, target_tree, move_source, changes, context_lines)?;
    let new_commit = if let Some(new_tree) = destination_tree {
        match destination {
            Destination::NewCommit {
                message,
                parent_commit_id: _,
                stack_segment: _,
            } => {
                let (author, committer) = repo.commit_signatures()?;
                let new_commit = create_possibly_signed_commit(
                    repo, author, committer, &message, new_tree, parents, None,
                )?;
                Some(new_commit)
            }
            Destination::AmendCommit {
                commit_id,
                new_message,
            } => {
                let mut commit = but_core::Commit::from_id(commit_id.attach(repo))?;
                commit.tree = new_tree;
                if let Some(message) = new_message {
                    commit.message = message.into();
                }
                Some(but_rebase::commit::create(
                    repo,
                    commit.inner,
                    DateMode::CommitterUpdateAuthorUpdate,
                )?)
            }
        }
    } else {
        None
    };
    Ok(CreateCommitOutcome {
        rejected_specs,
        new_commit,
        changed_tree_pre_cherry_pick,
        references: Vec::new(),
        rebase_output: None,
        index: None,
    })
}

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
    move_source: Option<MoveSourceCommit>,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<CreateCommitOutcome> {
    let mut out = create_commit(
        repo,
        destination.clone(),
        move_source,
        changes.clone(),
        context_lines,
    )?;

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
                            .map(|stack| crate::ui::StackEntryNoOpt::try_new(repo, stack))
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

            refs::rewrite(
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
#[expect(clippy::too_many_arguments)]
pub fn create_commit_and_update_refs_with_project(
    repo: &gix::Repository,
    project: &gitbutler_project::Project,
    maybe_stackid: Option<StackId>,
    destination: Destination,
    move_source: Option<MoveSourceCommit>,
    changes: Vec<DiffSpec>,
    context_lines: u32,
    _perm: &mut WorktreeWritePermission,
) -> anyhow::Result<CreateCommitOutcome> {
    let vbh = VirtualBranchesHandle::new(project.gb_dir());
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
    let out = create_commit_and_update_refs(
        repo,
        frame,
        &mut vb,
        destination,
        move_source,
        changes,
        context_lines,
    )?;

    vbh.write_file(&vb)?;
    Ok(out)
}

/// Create a commit exactly as specified, and sign it depending on Git and GitButler specific Git configuration.
fn create_possibly_signed_commit(
    repo: &gix::Repository,
    author: gix::actor::Signature,
    committer: gix::actor::Signature,
    message: &str,
    tree: gix::ObjectId,
    parents: impl IntoIterator<Item = impl Into<gix::ObjectId>>,
    commit_headers: Option<but_core::commit::HeadersV2>,
) -> anyhow::Result<gix::ObjectId> {
    let commit = gix::objs::Commit {
        message: message.into(),
        tree,
        author,
        committer,
        encoding: None,
        parents: parents.into_iter().map(Into::into).collect(),
        extra_headers: (&commit_headers.unwrap_or_default()).into(),
    };
    but_rebase::commit::create(repo, commit, DateMode::CommitterKeepAuthorKeep)
}

/// Less pure but a simpler version of [`create_commit_and_update_refs_with_project`]
pub fn create_commit_simple(
    ctx: &CommandContext,
    stack_id: StackId,
    parent_id: Option<gix::ObjectId>,
    worktree_changes: Vec<DiffSpec>,
    message: String,
    stack_branch_name: String,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<CreateCommitOutcome> {
    let repo = but_core::open_repo_for_merging(ctx.project().worktree_path())?;
    // If parent_id was not set but a stack branch name was provided, pick the current head of that branch as parent.
    let parent_commit_id: Option<gix::ObjectId> = match parent_id {
        Some(id) => Some(id),
        None => {
            let state = VirtualBranchesHandle::new(ctx.project().gb_dir());
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
        ctx.project(),
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
        None,
        worktree_changes,
        ctx.app_settings().context_lines,
        perm,
    );

    let outcome = outcome?;
    if !outcome.rejected_specs.is_empty() {
        tracing::warn!(?outcome.rejected_specs, "Failed to commit at least one hunk");
    }
    Ok(outcome)
}
