use anyhow::Context as _;
use bstr::ByteSlice;
use but_core::DiffSpec;
use but_core::ref_metadata::MaybeDebug;

use crate::WorkspaceCommit;

/// Build a merge-base override tree from `HEAD^{tree}` + `consumed` changes
/// (additive-only). During checkout, the 3-way snapshot merge uses this as its
/// base so consumed hunks cancel out and don't reappear as uncommitted changes.
///
/// Two kinds of changes are excluded to keep the tree additive-only:
///
/// * `previous_path` is stripped so rename-source deletions don't leak in.
/// * Full-file deletions (empty `hunk_headers`, file absent from worktree) are
///   skipped — including them would remove the path from the base, causing the
///   snapshot merge to misinterpret the worktree copy as a new addition.
fn compute_merge_base_override(
    repo: &gix::Repository,
    consumed: Vec<DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<gix::ObjectId> {
    let head_tree = repo.head_tree_id_or_empty()?;
    let mut specs: Vec<_> = consumed.into_iter().map(Ok).collect();
    if specs.is_empty() {
        return Ok(head_tree.detach());
    }
    let (committed_tree, _base) =
        but_core::tree::apply_worktree_changes(head_tree.into(), repo, &mut specs, context_lines)?;
    Ok(committed_tree.detach())
}

/// Which checkout the changes to commit are read from, and hence which one has to
/// cancel them out during materialization.
pub enum ChangeSource<'a> {
    /// The worktree of the repository the editor was created for.
    Head,
    /// A linked worktree, whose branch may live anywhere in the editor graph.
    ///
    /// The commit being created or amended may still live anywhere else - on a
    /// workspace stack or on the branch of another worktree.
    Worktree {
        /// A plain from-disk open of the linked worktree, as returned by
        /// [`crate::worktrees::open_worktree_repo()`].
        ///
        /// It must share the editor repo's object database and have no object
        /// memory: new objects are written loose to disk, which makes them
        /// immediately visible to the editor's in-memory repository.
        repo: &'a gix::Repository,
        /// The stable worktree name, i.e. the directory name under
        /// `$GIT_COMMON_DIR/worktrees/`.
        ///
        /// Committing fails without mutating the editor graph when this worktree
        /// has no checkout recorded in the editor - it is unknown, archived, or
        /// worktree tips weren't seeded into the graph.
        name: &'a bstr::BStr,
    },
}

impl ChangeSource<'_> {
    /// The repository the `changes` are read from, and whose `HEAD^{tree}` the
    /// merge-base override is built on.
    fn repo<'a, M: but_core::RefMetadata>(
        &'a self,
        editor: &'a but_rebase::graph_rebase::Editor<'_, M>,
    ) -> &'a gix::Repository {
        match self {
            ChangeSource::Head => editor.repo(),
            ChangeSource::Worktree { repo, .. } => repo,
        }
    }
}

/// Tell the editor which of `all_changes` were consumed, so the checkout that
/// provided them doesn't reintroduce them as uncommitted changes.
fn cancel_consumed_changes<M: but_core::RefMetadata>(
    editor: &mut but_rebase::graph_rebase::Editor<'_, M>,
    source: &ChangeSource<'_>,
    all_changes: Vec<DiffSpec>,
    rejected_specs: &[(but_core::tree::create_tree::RejectionReason, DiffSpec)],
    context_lines: u32,
) -> anyhow::Result<()> {
    let rejected_paths: std::collections::BTreeSet<_> =
        rejected_specs.iter().map(|(_, spec)| &spec.path).collect();
    let consumed: Vec<_> = all_changes
        .into_iter()
        .filter(|spec| !rejected_paths.contains(&spec.path))
        .collect();
    if consumed.is_empty() {
        return Ok(());
    }
    let merge_base = compute_merge_base_override(source.repo(editor), consumed, context_lines)?;
    match source {
        ChangeSource::Head => editor.set_merge_base_override(merge_base),
        ChangeSource::Worktree { name, .. } => {
            editor.set_worktree_merge_base_override(name, merge_base)?
        }
    }
    Ok(())
}

pub mod reword;
pub use reword::reword;
pub mod commit_create;
pub use commit_create::{CommitCreateOutcome, commit_create};
pub mod commit_amend;
pub use commit_amend::{CommitAmendOutcome, commit_amend};
pub mod insert_blank_commit;
pub use insert_blank_commit::insert_blank_commit;
pub mod move_changes;
pub use move_changes::{MoveChangesOutcome, move_changes_between_commits};
pub mod uncommit_changes;
pub use uncommit_changes::{
    UncommitChangesFailure, UncommitChangesFromCommitsOutcome, UncommitChangesOutcome,
    UncommitChangesSource, uncommit_changes, uncommit_changes_from_commits,
};
pub mod move_commit;
pub use move_commit::move_commits;
pub mod cherry_pick;
pub use cherry_pick::cherry_pick_commits;
pub mod discard_commit;
pub use discard_commit::discard_commits;
pub mod squash_commits;
pub use squash_commits::{SquashCommitsOutcome, squash_commits};

/// A minimal stack for use by [WorkspaceCommit::new_from_stacks()].
#[derive(Clone)]
pub struct Stack {
    /// The tip of the top-most branch, i.e., the most recent commit that would become the parent of new commits of the topmost stack branch.
    pub tip: gix::ObjectId,
    /// The tip branch's FULL ref name, shown (shortened) in the workspace commit message.
    pub ref_name: Option<gix::refs::FullName>,
}

impl std::fmt::Debug for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Stack { tip, ref_name } = self;
        write!(
            f,
            "Stack {{ tip: {tip}, name: {name:?} }}",
            tip = tip.to_hex_with_len(7),
            name = MaybeDebug(&ref_name.as_ref().map(|rn| rn.shorten().to_owned()))
        )
    }
}

/// Structures related to creating a merge-commit along with the respective tree.
pub mod merge {
    use anyhow::{Context as _, bail};
    use but_core::{
        RepositoryExt,
        ref_metadata::{MaybeDebug, WorkspaceCommitRelation},
    };
    use gix::prelude::ObjectIdExt;
    use tracing::instrument;

    use super::Stack;
    use crate::WorkspaceCommit;

    /// A optionally named tip that can be merged.
    #[derive(Debug, Clone)]
    pub struct Seed {
        /// The name of the reference that points to `commit_id`, or `None` if there is no such reference.
        /// The name is for use in the generated workspace commit message.
        pub name: Option<gix::refs::FullName>,
        /// The commit that should be merged into the workspace commit.
        pub commit_id: gix::ObjectId,
    }

    /// Tips resolved from workspace metadata, with references that metadata mentioned but the graph
    /// couldn't resolve.
    /// Returned by [WorkspaceCommit::tips_from_metadata()].
    pub struct ResolvedTips {
        /// Tips in the order they should appear as workspace commit parents.
        pub tips: Vec<Seed>,
        /// Metadata stack tips that couldn't be found in the graph.
        /// This is usually a problem, as the Graph is expected to contain everything of interest.
        pub missing_stacks: Vec<gix::refs::FullName>,
    }

    /// A minimal stack for to represent a stack that conflicted.
    #[derive(Clone)]
    pub struct ConflictingStack {
        /// The tip that could not be merged in.
        pub tip: gix::ObjectId,
        /// The name of the references to be merged, it pointed to `tip`.
        pub ref_name: Option<gix::refs::FullName>,
    }

    impl std::fmt::Debug for ConflictingStack {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let ConflictingStack { ref_name, tip } = self;
            f.debug_struct("ConflictingStack")
                .field("tip", tip)
                .field("ref_name", &MaybeDebug(ref_name))
                .finish()
        }
    }

    /// The outcome of a workspace-merge operation via [WorkspaceCommit::from_new_merge_with_metadata()].
    #[derive(Debug)]
    pub struct Outcome {
        /// The produced workspace commit, as written to the repository.
        pub workspace_commit_id: gix::ObjectId,
        /// The names and the tips of the stacks that were successfully merged, one for each
        /// parent of the `workspace_commit`.
        pub stacks: Vec<Stack>,
        /// The stacks that were listed in the input, and whose tips couldn't be found in the graph.
        pub missing_stacks: Vec<gix::refs::FullName>,
        /// All information about each stack, in order of occurrence, that could ultimately not be merged.
        pub conflicting_stacks: Vec<ConflictingStack>,
    }

    impl Outcome {
        /// Return `true` if the outcome isn't perfect, as conflicts happened while merging that led to unmerged stacks.
        pub fn has_conflicts(&self) -> bool {
            !self.conflicting_stacks.is_empty()
        }
    }

    /// Merging - create a merge-commit along with its tree.
    impl WorkspaceCommit<'_> {
        /// Resolve workspace metadata and anonymous stacks into merge tips.
        ///
        /// This preserves metadata ordering, reports missing metadata stacks, and inserts anonymous
        /// stacks at their projected parent numbers while avoiding duplicate commit tips.
        ///
        /// `stacks` are the workspace metadata stacks whose top branches should become named
        /// merge tips unless they are marked outside of the workspace.
        ///
        /// `anon_stacks` are unnamed projected tips paired with the parent number they occupied in
        /// the workspace projection, used to preserve anonymous parents not represented in metadata.
        ///
        /// `workspace` resolves metadata branch names to commit ids, and answers whether a stack is empty.
        pub fn tips_from_metadata<'a>(
            stacks: impl IntoIterator<Item = &'a but_core::ref_metadata::WorkspaceStack>,
            anon_stacks: impl IntoIterator<Item = (usize, Seed)>,
            workspace: &but_graph::Workspace,
        ) -> anyhow::Result<ResolvedTips> {
            let mut missing_stacks = Vec::new();
            // `None` entries (outside/missing stacks) still occupy their slot: the anonymous-tip
            // insertion below is positional over this list.
            let mut tips_with_metadata_numbers: Vec<Option<Seed>> = Vec::new();
            let entries: Vec<_> = stacks
                .into_iter()
                .filter_map(|s| s.branches.first().map(|b| (b, s.workspacecommit_relation)))
                .collect();
            // NAME WHAT THE MERGE HOLDS, write side (see the crate docs): the projection can only
            // name a parent that exists, so a SOLE stack is materialized even when EMPTY. The
            // never-a-parent rule below exists because git cannot repeat a parent and an
            // order-dependent resolution may drop a CONTAINED one — with exactly one stack there is
            // nothing to repeat and nothing to be contained in, and skipping it leaves the merge
            // holding no stack at all.
            let sole_applied_stack = entries
                .iter()
                .filter(|(_, relation)| relation.is_in_workspace())
                .count()
                == 1;
            for (top_segment, relation) in entries {
                let seed = match relation {
                    WorkspaceCommitRelation::Merged => {
                        let stack_tip_name = top_segment.ref_name.as_ref();
                        match workspace.commit_id_by_ref_name(stack_tip_name) {
                            None => {
                                missing_stacks.push(top_segment.ref_name.to_owned());
                                None
                            }
                            // An EMPTY stack is never materialized as a merge parent
                            // (user ruling 2026-07-23: git cannot repeat a parent and
                            // order-dependent resolution may drop a contained one) —
                            // the stored layout represents its lane, and readers stay
                            // accurate when a legacy merge carries one anyway.
                            Some(_)
                                if !sole_applied_stack
                                    && workspace.find_branch(stack_tip_name).is_some_and(
                                        |(stack, _)| {
                                            stack.segments.iter().all(|s| s.commits.is_empty())
                                        },
                                    ) =>
                            {
                                None
                            }
                            Some(commit_id) => Some(Seed {
                                name: Some(stack_tip_name.to_owned()),
                                commit_id,
                            }),
                        }
                    }
                    WorkspaceCommitRelation::MergeFrom { .. } => {
                        // These would join the parents list without being merged; callers that
                        // want them re-merged pass them as `Merged`. Nothing constructs this
                        // relation yet, so error instead of building a wrong merge.
                        anyhow::bail!(
                            "Cannot build a workspace merge for '{}': its unmerged-tree (MergeFrom) relation is not supported yet",
                            top_segment.ref_name
                        );
                    }
                    WorkspaceCommitRelation::Outside => None,
                };
                tips_with_metadata_numbers.push(seed);
            }
            // A commit cannot be a parent twice — git says so, not a heuristic — so stacks landing
            // on one commit contribute ONE slot. This is also the honest answer to "does this stack
            // contain anything" when the projection cannot give one: the empty check above asks
            // where the branch sits in the CURRENT projection, and a branch being applied is not in
            // it yet, so the check abstained and every such stack minted a slot. Applying a third
            // branch already at the tip then wrote a merge with the same parent three times.
            // Slots stay in place as `None`, since the anonymous insertion below is positional.
            let mut seen_tips = std::collections::HashSet::new();
            for seed in tips_with_metadata_numbers.iter_mut() {
                if seed
                    .as_ref()
                    .is_some_and(|s| !seen_tips.insert(s.commit_id))
                {
                    *seed = None;
                }
            }
            let named_tips = tips_with_metadata_numbers
                .iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>();
            let mut anon_stacks = anon_stacks.into_iter().collect::<Vec<_>>();
            anon_stacks.sort_by_key(|(idx, _)| *idx);
            for (idx, anon_tip) in anon_stacks {
                if named_tips.iter().any(|t| t.commit_id == anon_tip.commit_id) {
                    // prevent duplication of tips, make calling this easier as well.
                    continue;
                }
                tips_with_metadata_numbers
                    .insert(idx.min(tips_with_metadata_numbers.len()), Some(anon_tip));
            }
            let tips: Vec<Seed> = tips_with_metadata_numbers.into_iter().flatten().collect();
            Ok(ResolvedTips {
                tips,
                missing_stacks,
            })
        }

        /// like [`Self::from_new_merge_with_metadata`], but supports tips, which makes it possible to re-merge anything
        /// even if the tip is unnamed.
        /// Note that [`missing_stacks`](Outcome::missing_stacks) is never set.
        ///
        /// ### Algorithm
        ///
        /// Fold the tips left to right into one tree, enforcing the lowest merge-base by carrying
        /// the previous iteration's base forward. A conflicting tip is skipped; a conflicting HERO
        /// tip instead skips its nearest merged predecessor and restarts, since the hero must land.
        /// Once the hero merges after skips, `schedule_merge_trials_after_hero` re-arms the
        /// not-yet-proven skips as trials, the fold restarts once more, and each trial is settled
        /// by `resolve_merge_trial` — so only tips that truly conflict with the hero stay out.
        /// `conclude_tips_merge` partitions the verdicts and writes the merge commit.
        pub fn from_new_merge_with_tips(
            tips: impl IntoIterator<Item = Seed>,
            workspace: &but_graph::Workspace,
            repo: &gix::Repository,
            hero_stack: Option<&gix::refs::FullNameRef>,
        ) -> anyhow::Result<Outcome> {
            use Instruction as I;
            let mut tips: Vec<(Instruction, Seed)> =
                tips.into_iter().map(|t| (I::Merge, t)).collect();

            let mut ran_merge_trials_loop_safety = false;
            #[expect(clippy::indexing_slicing)]
            'retry_loop: loop {
                let mut prev_base_commit_id = None;
                let mut merge_tree_id = None;
                let mut previous_tip = None;
                let (merge_options, conflict_kind) = repo.merge_options_fail_fast()?;
                let labels_uninteresting_as_no_conflict_allowed = repo.default_merge_labels();
                'tips_loop: for tip_idx in 0..tips.len() {
                    let (
                        mode,
                        Seed {
                            name: ref_name,
                            commit_id,
                            ..
                        },
                    ) = &mut tips[tip_idx];
                    let this_commit_id = *commit_id;
                    if mode.should_skip() {
                        continue;
                    }
                    let this_tree_id = peel_to_tree(commit_id.attach(repo))?;
                    if let Some((prev_tree_id, prev_commit_id)) = previous_tip {
                        let (base_tree_id, base_commit_id) = {
                            // This is critical: we enforce using the lowest merge-base by using
                            // the previous iterations merge-base.
                            // This is the same as computing the merge-base between the new
                            // (non-existing merge-commit) and the next tip.
                            let left = prev_base_commit_id.unwrap_or(prev_commit_id);
                            compute_merge_base(workspace, repo, left, this_commit_id)?
                        };

                        let mut merge = repo.merge_trees(
                            base_tree_id,
                            merge_tree_id.unwrap_or(prev_tree_id),
                            this_tree_id,
                            labels_uninteresting_as_no_conflict_allowed,
                            merge_options.clone(),
                        )?;
                        let is_hero = hero_stack.is_some_and(|hero| {
                            Some(hero) == ref_name.as_ref().map(|rn| rn.as_ref())
                        });
                        if merge.has_unresolved_conflicts(conflict_kind) {
                            if matches!(mode, I::MergeTrial { .. }) {
                                bail!(
                                    "BUG: Found {ref_name:?} in merge-trial, even though these shouldn't fail without the hero merged in"
                                );
                            }
                            if is_hero {
                                // We definitely want this one, so must restart the whole operation
                                // while disallowing the most recent allowed tip.
                                let err_msg = format!(
                                    "BUG: if there was no allowed stack in front of {ref_name:?}, then we aren't here as no merge can be done with just one branch"
                                );
                                let presumed_conflicting_tip = tips[..tip_idx]
                                    .iter_mut()
                                    .rev()
                                    .find(|(mode, ..)| !mode.should_skip())
                                    .context(err_msg)?;
                                presumed_conflicting_tip.0 = I::Skip;
                                continue 'retry_loop;
                            } else {
                                // Ignore this stack, continue with the others.
                                *mode = I::Skip;
                                continue 'tips_loop;
                            }
                        } else if is_hero {
                            let has_merge_trials = schedule_merge_trials_after_hero(
                                &mut tips[..tip_idx],
                                this_commit_id,
                                this_tree_id,
                            )?;
                            if has_merge_trials {
                                if ran_merge_trials_loop_safety {
                                    bail!(
                                        "BUG: somehow we managed to try to run merge-trials twice, probably leading to an infinite loop"
                                    );
                                }
                                ran_merge_trials_loop_safety = true;
                                continue 'retry_loop;
                            }
                            // We are past possible trials and proceed as usual, with future conflicting stacks just being dropped.
                        } else if let I::MergeTrial {
                            hero_commit_id,
                            hero_tree_id,
                        } = *mode
                        {
                            *mode = resolve_merge_trial(
                                workspace,
                                repo,
                                &mut merge,
                                base_commit_id,
                                hero_commit_id,
                                hero_tree_id,
                                labels_uninteresting_as_no_conflict_allowed,
                                &merge_options,
                                conflict_kind,
                            )?;
                            if matches!(mode, I::CertainConflict) {
                                // Now that we know it's actually a conflict, do not retain more state so
                                // the conflicting one isn't recorded in the merge.
                                continue 'tips_loop;
                            }
                        }
                        prev_base_commit_id = Some(base_commit_id);
                        merge_tree_id = merge.tree.write()?.detach().into();
                    }
                    previous_tip = Some((this_tree_id, this_commit_id));
                }

                return conclude_tips_merge(&tips, merge_tree_id, previous_tip, repo);
            }
        }

        /// Using the names of the `stacks` stored in [workspace metadata](but_core::ref_metadata::Workspace),
        /// create a new workspace commit with their tips extracted from `workspace`. Note that stacks that don't exist in the graph aren't fatal.
        /// Also, this will create a workspace commit as it's desired, but not as it is, and the caller should assure that all branches are present.
        ///
        /// Use `anon_stacks` with `(parent_index, tip)` to fill-in anonymous commits that aren't listed in metadata,
        /// as they have *no known name*. We will make sure that no commit in `anon_stacks` is a duplicate with a `stack`, and
        /// we will insert them at `parent_index` into the resulting list so they don't change their position.
        ///
        /// Use `hero_stack` to highlight a stack that you definitely want merged in, and would rather not merge other stacks for it.
        /// This can lead to a situation where only the hero stack is applied.
        /// If there is only one stack, it just uses the tree of that stack. It's an error if `stacks` is empty.
        /// `repo` is expected to be configured to be suitable for merges, and it *should* be configured to write objects into memory
        /// unless the caller knows that any result of the merge is acceptable.
        ///
        /// ### Shortcoming: inefficient conflict behaviour
        ///
        /// In order to find out exactly which branches conflicts, we repeat the whole operations with different configuration.
        /// One could be better and only repeat what didn't change, to avoid repeating unnecessarily.
        /// But that shouldn't usually matter unless in the biggest repositories with tree-merge times past a 500ms or so.
        #[instrument(
            name = "re-merge workspace commit",
            level = "debug",
            skip(stacks, anon_stacks, workspace, repo),
            err(Debug)
        )]
        pub fn from_new_merge_with_metadata<'a>(
            stacks: impl IntoIterator<Item = &'a but_core::ref_metadata::WorkspaceStack>,
            anon_stacks: impl IntoIterator<Item = (usize, Seed)>,
            workspace: &but_graph::Workspace,
            repo: &gix::Repository,
            hero_stack: Option<&gix::refs::FullNameRef>,
        ) -> anyhow::Result<Outcome> {
            let ResolvedTips {
                tips,
                missing_stacks,
            } = Self::tips_from_metadata(stacks, anon_stacks, workspace)?;
            let mut out = Self::from_new_merge_with_tips(tips, workspace, repo, hero_stack)?;
            out.missing_stacks = missing_stacks;
            Ok(out)
        }
    }

    /// Per-tip verdict while folding the workspace merge in
    /// [`WorkspaceCommit::from_new_merge_with_tips`].
    #[derive(Debug)]
    enum Instruction {
        /// Merge this tip into the fold.
        Merge,
        /// A previously skipped tip re-armed for testing: merge it, then test-merge the hero on
        /// top to learn whether it truly conflicts with the hero or was skipped by accident.
        MergeTrial {
            /// The hero's commit, the trial's other side.
            hero_commit_id: gix::ObjectId,
            /// The hero's tree, the trial's other side.
            hero_tree_id: gix::ObjectId,
        },
        /// Leave this tip out of the fold (it conflicted, or is under suspicion).
        Skip,
        /// Proven to conflict — permanently out.
        CertainConflict,
    }

    impl Instruction {
        fn should_skip(&self) -> bool {
            match self {
                Instruction::Merge | Instruction::MergeTrial { .. } => false,
                Instruction::Skip | Instruction::CertainConflict => true,
            }
        }
    }

    /// After the hero merged despite earlier skips, decide which skips were real conflicts:
    /// the FIRST skip is a certain conflict (the hero conflicted right after it), every later
    /// one becomes a [`Instruction::MergeTrial`] to be proven individually — imagine
    /// `G1 X X X X X H`: only some of the X may truly conflict with `H`, and each is tested by
    /// merging `H` right after it. Returns whether any trials were scheduled (the fold must
    /// restart to run them).
    fn schedule_merge_trials_after_hero(
        tips_before_hero: &mut [(Instruction, Seed)],
        hero_commit_id: gix::ObjectId,
        hero_tree_id: gix::ObjectId,
    ) -> anyhow::Result<bool> {
        let mut saw_first_certain_conflict = false;
        let mut has_merge_trials = false;
        for (mode, _) in tips_before_hero {
            match mode {
                Instruction::Merge => continue,
                Instruction::MergeTrial { .. } => {
                    bail!("BUG: found a merge-trial, even though trial should be concluded by now")
                }
                Instruction::CertainConflict => saw_first_certain_conflict = true,
                Instruction::Skip => {
                    if saw_first_certain_conflict {
                        *mode = Instruction::MergeTrial {
                            hero_commit_id,
                            hero_tree_id,
                        };
                        has_merge_trials = true;
                    } else {
                        *mode = Instruction::CertainConflict;
                        saw_first_certain_conflict = true;
                    }
                }
            }
        }
        Ok(has_merge_trials)
    }

    /// Settle one [`Instruction::MergeTrial`]: the tip under trial merged cleanly into the fold
    /// (`merge`), so test-merge the hero on top of that result — a clean merge acquits the tip
    /// ([`Instruction::Merge`]), a conflict convicts it ([`Instruction::CertainConflict`]).
    #[expect(clippy::too_many_arguments)]
    fn resolve_merge_trial(
        workspace: &but_graph::Workspace,
        repo: &gix::Repository,
        merge: &mut gix::merge::tree::Outcome<'_>,
        base_commit_id: gix::ObjectId,
        hero_commit_id: gix::ObjectId,
        hero_tree_id: gix::ObjectId,
        labels: gix::merge::blob::builtin_driver::text::Labels<'_>,
        merge_options: &gix::merge::tree::Options,
        conflict_kind: gix::merge::tree::TreatAsUnresolved,
    ) -> anyhow::Result<Instruction> {
        let base_tree_id = compute_merge_base(workspace, repo, base_commit_id, hero_commit_id)?.0;
        let trial = repo.merge_trees(
            base_tree_id,
            merge.tree.write()?,
            hero_tree_id,
            labels,
            merge_options.clone(),
        )?;
        Ok(if trial.has_unresolved_conflicts(conflict_kind) {
            Instruction::CertainConflict
        } else {
            Instruction::Merge
        })
    }

    /// Conclude the fold: partition the tips by verdict into merged stacks and conflicting
    /// stacks, then write the workspace merge commit over the folded tree.
    fn conclude_tips_merge(
        tips: &[(Instruction, Seed)],
        merge_tree_id: Option<gix::ObjectId>,
        previous_tip: Option<(gix::ObjectId, gix::ObjectId)>,
        repo: &gix::Repository,
    ) -> anyhow::Result<Outcome> {
        let (stacks, conflicting_stacks) = tips.iter().fold(
            (Vec::new(), Vec::new()),
            |(mut stacks, mut conflicting_stacks),
             (
                mode,
                Seed {
                    name: ref_name,
                    commit_id,
                },
            )| {
                if mode.should_skip() {
                    conflicting_stacks.push(ConflictingStack {
                        tip: *commit_id,
                        ref_name: ref_name.clone(),
                    });
                } else {
                    stacks.push(Stack {
                        tip: *commit_id,
                        ref_name: ref_name.clone(),
                    });
                }
                (stacks, conflicting_stacks)
            },
        );

        if stacks.is_empty() {
            bail!(
                "BUG: Cannot merge nothing, no tips ended up in the graph: `conflicting_stacks` = {conflicting_stacks:?}, `tips` = : {tips:?}"
            )
        }

        let merge_tree_id = merge_tree_id
            .or({
                // Just one stack?
                previous_tip.map(|t| t.0)
            })
            .context("having stacks means the loop ran once")?;

        // Finally, create the merge-commit itself.
        let mut ws_commit =
            WorkspaceCommit::new_from_stacks(stacks.iter().cloned(), repo.object_hash());
        ws_commit.tree = merge_tree_id;
        WorkspaceCommit::fixup_times(&mut ws_commit, repo);

        let workspace_commit_id = repo.write_object(&ws_commit)?.detach();
        Ok(Outcome {
            workspace_commit_id,
            stacks,
            missing_stacks: vec![], /* this is never set here as all tips are already resolved */
            conflicting_stacks,
        })
    }

    fn compute_merge_base(
        workspace: &but_graph::Workspace,
        repo: &gix::Repository,
        left: gix::ObjectId,
        right: gix::ObjectId,
    ) -> anyhow::Result<(gix::ObjectId, gix::ObjectId)> {
        let base_commit_id = workspace
            .commit_graph()
            .merge_base(left, right)
            .with_context(|| {
                format!(
                    "Couldn't find merge-base between {left} and {right} - they are disjoint in the commit-graph"
                )
            })?;
        Ok((peel_to_tree(base_commit_id.attach(repo))?, base_commit_id))
    }

    fn peel_to_tree(commit: gix::Id) -> anyhow::Result<gix::ObjectId> {
        let commit = but_core::Commit::from_id(commit)?;
        Ok(commit.tree_id_or_auto_resolution()?.detach())
    }
}

/// Construction
impl<'repo> WorkspaceCommit<'repo> {
    const GITBUTLER_WORKSPACE_COMMIT_TITLE: &'static str = "GitButler Workspace Commit";

    /// Decode the object at `commit_id` and keep its data for later query.
    pub fn from_id(commit_id: gix::Id<'repo>) -> anyhow::Result<Self> {
        let commit = commit_id
            .object()?
            .try_into_commit()?
            .decode()?
            .try_into()?;
        Ok(WorkspaceCommit {
            id: commit_id,
            inner: commit,
        })
    }

    /// A way to create a commit from `workspace` stacks, with the `tree` being used as the tree of the workspace commit.
    /// It's supposed to be the legitimate merge of the stacks contained in `workspace`.
    /// Note that it will be written to `repo` immediately for persistence, with its object id returned.
    pub fn from_graph_workspace_and_tree(
        workspace: &but_graph::Workspace,
        repo: &'repo gix::Repository,
        tree: gix::ObjectId,
    ) -> anyhow::Result<Self> {
        let stacks: Vec<_> = workspace
            .stacks
            .iter()
            .map(|s| {
                let name = s.ref_name().map(|rn| rn.shorten().to_owned());
                let s = Stack {
                    ref_name: s.ref_name().map(ToOwned::to_owned),
                    tip: s.resting_commit().with_context(|| {
                        format!(
                            "Could not find any commit to serve as tip for stack {id:?} with name {name:?}",
                            id = s.id
                        )
                    })?,
                };
                anyhow::Ok(s)
            })
            .collect::<Result<_, _>>()?;
        // A lane is not automatically a parent. Lanes resting on ONE commit contribute one slot —
        // git cannot repeat a parent — so an empty lane, whose resting commit is a sibling's, adds
        // no ancestry and gets none: the declaration is its representation. Taking the projection
        // at face value here wrote a merge with the same parent once per lane, and once written a
        // repeat reads back as ancestry, so every further empty lane added another.
        let mut resting = std::collections::HashSet::new();
        let stacks: Vec<_> = stacks
            .into_iter()
            .filter(|s: &Stack| resting.insert(s.tip))
            .collect();
        // The parents are the projection's stacks, minus the repeats filtered above.
        // We know the workspace commit is the same as the current HEAD, no need to merge, nothing changed
        // use the same tree.
        let mut ws_commit = Self::new_from_stacks(stacks, repo.object_hash());
        ws_commit.tree = tree;

        Self::fixup_times(&mut ws_commit, repo);
        let id = repo.write_object(&ws_commit)?;
        Ok(Self {
            id,
            inner: ws_commit,
        })
    }

    /// also rewrite the author and committer time, just to be sure we respect all settings. `new_from_stacks` doesn't have a repo.
    fn fixup_times(ws_commit: &mut gix::objs::Commit, repo: &gix::Repository) {
        fn try_time(
            sig: Option<Result<gix::actor::SignatureRef<'_>, gix::config::time::Error>>,
        ) -> Option<gix::date::Time> {
            sig.transpose().ok().flatten().and_then(|s| s.time().ok())
        }
        if let Some(committer_time) = try_time(repo.committer()) {
            ws_commit.committer.time = committer_time;
        }
        if let Some(author_time) = try_time(repo.committer()) {
            ws_commit.author.time = author_time;
        }
    }

    /// Create a new commit which presents itself as the merge of all the given `stacks`.
    ///
    /// Note that the returned commit lives entirely in memory and would still have to be written to disk.
    /// It still needs its tree set to something non-empty.
    ///
    /// `object_hash` is needed to create an empty tree hash.
    pub fn new_from_stacks(
        stacks: impl IntoIterator<Item = impl Into<Stack>>,
        object_hash: gix::hash::Kind,
    ) -> gix::objs::Commit {
        let stacks = stacks.into_iter().map(Into::into).collect::<Vec<_>>();
        // message that says how to get back to where they were
        let mut message = Self::GITBUTLER_WORKSPACE_COMMIT_TITLE.to_string();
        message.push_str("\n\n");
        if !stacks.is_empty() {
            message
                .push_str("This is a merge commit of the virtual branches in your workspace.\n\n");
        } else {
            message
                .push_str("This is placeholder commit and will be replaced by a merge of your virtual branches.\n\n");
        }
        message.push_str(
            "Due to GitButler managing multiple virtual branches, you cannot switch back and\n",
        );
        message.push_str("forth between git branches and virtual branches easily. \n\n");

        message.push_str(
            "If you switch to another branch, GitButler will need to be reinitialized.\n",
        );
        message.push_str("If you commit on this branch, GitButler will throw it away.\n\n");
        if !stacks.is_empty() {
            message.push_str("Here are the branches that are currently applied:\n");
            for branch in &stacks {
                if let Some(name) = branch.ref_name.as_ref().map(|rn| rn.shorten()) {
                    message.push_str(" - ");
                    message.push_str(name.to_str_lossy().as_ref());
                    message.push('\n');
                }

                message.push_str("   branch head: ");
                message.push_str(&branch.tip.to_string());
                message.push('\n');
            }
        }
        message.push_str("For more information about what we're doing here, check out our docs:\n");
        message
            .push_str("https://docs.gitbutler.com/features/branch-management/integration-branch\n");

        let author = commit_signature(commit_time("GIT_COMMITTER_DATE"));
        gix::objs::Commit {
            tree: gix::ObjectId::empty_tree(object_hash),
            parents: stacks.iter().map(|s| s.tip).collect(),
            committer: author.clone(),
            author,
            encoding: Some("UTF-8".into()),
            message: message.into(),
            extra_headers: Vec::new(),
        }
    }
}

fn commit_signature(time: gix::date::Time) -> gix::actor::Signature {
    gix::actor::Signature {
        name: "GitButler".into(),
        email: "gitbutler@gitbutler.com".into(),
        time,
    }
}

/// Return the time of a commit as `now` unless the `overriding_variable_name` contains a parseable date,
/// which is used instead.
fn commit_time(overriding_variable_name: &str) -> gix::date::Time {
    std::env::var(overriding_variable_name)
        .ok()
        .and_then(|time| gix::date::parse(&time, Some(std::time::SystemTime::now())).ok())
        .unwrap_or_else(gix::date::Time::now_local_or_utc)
}

/// Query
impl WorkspaceCommit<'_> {
    /// Return `true` if this commit is managed by GitButler.
    /// If `false`, this is the tip of the stack itself which will be put underneath a *managed* workspace commit
    /// once another branch is added to the workspace.
    pub fn is_managed(&self) -> bool {
        but_graph::workspace::commit::is_managed_workspace_by_message(self.message.as_bstr())
    }
}

impl std::ops::Deref for WorkspaceCommit<'_> {
    type Target = gix::objs::Commit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for WorkspaceCommit<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
