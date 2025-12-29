use anyhow::Context as _;
use bstr::{BString, ByteSlice};
use but_core::ref_metadata::MaybeDebug;

use crate::WorkspaceCommit;

pub mod reword;
pub use reword::function::reword;
pub mod insert_blank_commit;
pub use insert_blank_commit::function::insert_blank_commit;

/// A minimal stack for use by [WorkspaceCommit::new_from_stacks()].
#[derive(Clone)]
pub struct Stack {
    /// The tip of the top-most branch, i.e., the most recent commit that would become the parent of new commits of the topmost stack branch.
    pub tip: gix::ObjectId,
    /// The short name of the stack, which is the name of the top-most branch,
    /// like `main` or `feature/branch` or `origin/tracking-some-PR` or something entirely made up.
    pub name: Option<BString>,
}

impl std::fmt::Debug for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Stack { tip, name } = self;
        write!(
            f,
            "Stack {{ tip: {tip}, name: {name:?} }}",
            tip = tip.to_hex_with_len(7),
            name = MaybeDebug(name)
        )
    }
}

/// Structures related to creating a merge-commit along with the respective tree.
pub mod merge {
    use anyhow::{Context as _, bail};
    use but_core::RepositoryExt;
    use but_core::ref_metadata::{MaybeDebug, WorkspaceCommitRelation};
    use but_graph::SegmentIndex;
    use gix::prelude::ObjectIdExt;
    use tracing::instrument;

    use super::Stack;
    use crate::WorkspaceCommit;

    /// A optionally named tip that can be merged.
    #[derive(Debug, Clone)]
    pub struct Tip {
        /// The name of the reference that points to `commit_id`, or `None` if there is no such reference.
        /// The name is for use in the generated workspace commit message.
        pub name: Option<gix::refs::FullName>,
        /// The commit that should be merged into the workspace commit.
        pub commit_id: gix::ObjectId,
        /// The index to the top-most segment of the stack in the graph for use in merge-base computation.
        pub segment_idx: SegmentIndex,
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
        /// like [`Self::from_new_merge_with_metadata`], but supports tips, which makes it possible to re-merge anything
        /// even if the tip is unnamed.
        /// Note that [`missing_stacks`](Outcome::missing_stacks) is never set.
        pub fn from_new_merge_with_tips(
            tips: impl IntoIterator<Item = Tip>,
            graph: &but_graph::Graph,
            repo: &gix::Repository,
            hero_stack: Option<&gix::refs::FullNameRef>,
        ) -> anyhow::Result<Outcome> {
            #[derive(Debug)]
            enum Instruction {
                Merge,
                MergeTrial {
                    hero_sidx: SegmentIndex,
                    hero_tree_id: gix::ObjectId,
                },
                Skip,
                CertainConflict,
            }
            use Instruction as I;
            impl Instruction {
                fn should_skip(&self) -> bool {
                    match self {
                        I::Merge | I::MergeTrial { .. } => false,
                        I::Skip | I::CertainConflict => true,
                    }
                }
            }
            let mut tips: Vec<(Instruction, Tip)> =
                tips.into_iter().map(|t| (I::Merge, t)).collect();

            let mut ran_merge_trials_loop_safety = false;
            #[allow(clippy::indexing_slicing)]
            'retry_loop: loop {
                let mut prev_base_sidx = None;
                let mut merge_tree_id = None;
                let mut previous_tip = None;
                let (merge_options, conflict_kind) = repo.merge_options_fail_fast()?;
                let labels_uninteresting_as_no_conflict_allowed = repo.default_merge_labels();
                'tips_loop: for tip_idx in 0..tips.len() {
                    let (
                        mode,
                        Tip {
                            name: ref_name,
                            commit_id,
                            segment_idx: sidx,
                        },
                    ) = &mut tips[tip_idx];
                    let sidx = *sidx;
                    if mode.should_skip() {
                        continue;
                    }
                    let this_tree_id = peel_to_tree(commit_id.attach(repo))?;
                    if let Some((prev_tree_id, prev_sidx)) = previous_tip {
                        let (base_tree_id, base_sidx) = {
                            // This is critical: we enforce using the lowest merge-base by using
                            // the previous iterations merge-base.
                            // This is the same as computing the merge-base between the new
                            // (non-existing merge-commit) and the next tip.
                            let left = prev_base_sidx.unwrap_or(prev_sidx);
                            compute_merge_base(graph, repo, left, sidx)?
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
                            // Look back and see if there is any skipped stacks. If so, we now merged the hero branch successfully,
                            //
                            // This means that skipping some worked. Now we want to try to re-enable previously disabled ones to learn if they
                            // were really at fault. Imagine `G1 X X X X X H` with H being hero and G1 being the good ones.
                            // It's notable how multiple branches of these X can be good, but some in the middle can also be bad - imagine
                            // one file being wrong in one, and another in another stack, so two stacks are causing conflicts while some
                            // in between are not causing conflicts.
                            // With this, we might find that it's actually `G1 X G2 G3 X G4 H`, and we don't unnecessarily unapply unrelated branches.
                            // However, we only know that the first X is definitely a conflict, and all others we have to test one after another
                            // by test-merging H right after the X under test.

                            // First, mark the first X as conflict as we know it for sure.
                            let mut saw_first_certain_conflict = false;
                            let mut has_merge_trials = false;
                            for (mode, _) in &mut tips[..tip_idx] {
                                match mode {
                                    I::Merge => continue,
                                    I::MergeTrial { .. } => {
                                        bail!(
                                            "BUG: found a merge-trial, even though trial should be concluded by now"
                                        )
                                    }
                                    I::CertainConflict => saw_first_certain_conflict = true,
                                    I::Skip => {
                                        if saw_first_certain_conflict {
                                            *mode = I::MergeTrial {
                                                hero_sidx: sidx,
                                                hero_tree_id: this_tree_id,
                                            };
                                            has_merge_trials = true;
                                        } else {
                                            *mode = I::CertainConflict;
                                            saw_first_certain_conflict = true;
                                        }
                                    }
                                }
                            }

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
                            hero_sidx,
                            hero_tree_id,
                        } = *mode
                        {
                            // This stack merged cleanly, and now we have to merge the hero into that result to see if it works.
                            // This tells us if this is stack merges cleanly or causes a real conflict in conjunction with hero.
                            let base_tree_id =
                                compute_merge_base(graph, repo, base_sidx, hero_sidx)?.0;
                            let merge = repo.merge_trees(
                                base_tree_id,
                                merge.tree.write()?,
                                hero_tree_id,
                                labels_uninteresting_as_no_conflict_allowed,
                                merge_options.clone(),
                            )?;
                            let trial_outcome = if merge.has_unresolved_conflicts(conflict_kind) {
                                I::CertainConflict
                            } else {
                                I::Merge
                            };
                            *mode = trial_outcome;
                            if matches!(mode, I::CertainConflict) {
                                // Now that we know it's actually a conflict, do not retain more state so
                                // the conflicting one isn't recorded in the merge.
                                continue 'tips_loop;
                            }
                        }
                        prev_base_sidx = Some(base_sidx);
                        merge_tree_id = merge.tree.write()?.detach().into();
                    }
                    previous_tip = Some((this_tree_id, sidx));
                }

                let (stacks, conflicting_stacks) = tips.iter().fold(
                    (Vec::new(), Vec::new()),
                    |(mut stacks, mut conflicting_stacks),
                     (
                        mode,
                        Tip {
                            name: ref_name,
                            commit_id,
                            ..
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
                                name: ref_name.as_ref().map(|rn| rn.shorten().to_owned()),
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
                    Self::new_from_stacks(stacks.iter().cloned(), repo.object_hash());
                ws_commit.tree = merge_tree_id;
                Self::fixup_times(&mut ws_commit, repo);

                let workspace_commit_id = repo.write_object(&ws_commit)?.detach();
                return Ok(Outcome {
                    workspace_commit_id,
                    stacks,
                    missing_stacks: vec![], /* this is never set here as all tips are already resolved */
                    conflicting_stacks,
                });
            }
        }

        /// Using the names of the `stacks` stored in [workspace metadata](but_core::ref_metadata::Workspace),
        /// create a new workspace commit with their tips extracted from `graph`. Note that stacks that don't exist in `graph` aren't fatal.
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
        /// IMPORTANT: This inherently needs the tips to be represented by named branches, so this can't be used to
        ///            re-merge a workspace with lost or renamed branches. It is, however, good to 'fix' workspaces
        ///            whose tips were advanced and now are outside the workspace, provide the ref that advanced still exists.
        ///
        /// ### Shortcoming: inefficient conflict behaviour
        ///
        /// In order to find out exactly which branches conflicts, we repeat the whole operations with different configuration.
        /// One could be better and only repeat what didn't change, to avoid repeating unnecessarily.
        /// But that shouldn't usually matter unless in the biggest repositories with tree-merge times past a 500ms or so.
        #[instrument(name = "re-merge workspace commit", level = tracing::Level::DEBUG, skip(stacks, anon_stacks, graph, repo), err(Debug))]
        pub fn from_new_merge_with_metadata<'a>(
            stacks: impl IntoIterator<Item = &'a but_core::ref_metadata::WorkspaceStack>,
            anon_stacks: impl IntoIterator<Item = (usize, Tip)>,
            graph: &but_graph::Graph,
            repo: &gix::Repository,
            hero_stack: Option<&gix::refs::FullNameRef>,
        ) -> anyhow::Result<Outcome> {
            let mut missing_stacks = Vec::new();
            let mut tips: Vec<_> = stacks
                .into_iter()
                .filter_map(|s| s.branches.first().map(|b| (b, s.workspacecommit_relation)))
                .filter_map(|(top_segment, relation)| {
                    match relation {
                        WorkspaceCommitRelation::Merged => {}
                        WorkspaceCommitRelation::MergeFrom { .. } => {
                            // These need to be part of the parents list, but shouldn't be merged.
                            // If the caller wants to retry them, they can be passed here as "Merged".
                            todo!("this is a placeholder for where we will have to start handling this UnmergedTree")
                        }
                        WorkspaceCommitRelation::Outside => return None,
                    }
                    let stack_tip_name = top_segment.ref_name.as_ref();
                    match graph.segment_and_commit_by_ref_name(stack_tip_name) {
                        None => {
                            missing_stacks.push(top_segment.ref_name.to_owned());
                            None
                        }
                        Some((segment, commit)) => Some(Tip {
                            name: Some(stack_tip_name.to_owned()),
                            commit_id: commit.id,
                            segment_idx: segment.id,
                        }),
                    }
                })
                .collect();
            for (idx, anon_tip) in anon_stacks {
                if tips.iter().any(|t| {
                    t.commit_id == anon_tip.commit_id || t.segment_idx == anon_tip.segment_idx
                }) {
                    // prevent duplication of tips, make calling this easier as well.
                    continue;
                }
                tips.insert(idx, anon_tip)
            }
            let mut out = Self::from_new_merge_with_tips(tips, graph, repo, hero_stack)?;
            out.missing_stacks = missing_stacks;
            Ok(out)
        }
    }

    fn compute_merge_base(
        graph: &but_graph::Graph,
        repo: &gix::Repository,
        left: SegmentIndex,
        right: SegmentIndex,
    ) -> anyhow::Result<(gix::ObjectId, SegmentIndex)> {
        let base_sidx = graph.first_merge_base(left, right).with_context(|| {
            format!(
                "Couldn't find merge-base between segments {l} and {r}",
                l = left.index(),
                r = right.index()
            )
        })?;
        let base_commit_id = graph
            .tip_skip_empty(base_sidx)
            .with_context(|| {
                format!(
                    "Base segment {base} between {l} and {r} didn't have  single commit reachable",
                    base = base_sidx.index(),
                    l = left.index(),
                    r = right.index()
                )
            })?
            .id
            .attach(repo);
        Ok((peel_to_tree(base_commit_id)?, base_sidx))
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
        workspace: &but_graph::projection::Workspace,
        repo: &'repo gix::Repository,
        tree: gix::ObjectId,
    ) -> anyhow::Result<Self> {
        let stacks: Vec<_> = workspace
            .stacks
            .iter()
            .map(|s| {
                let name = s.ref_name().map(|rn| rn.shorten().to_owned());
                let s = Stack {
                    tip: s.tip_skip_empty().or(s.base()).with_context(|| {
                        format!(
                            "Could not find any commit to serve as tip for stack {id:?} with name {name:?}",
                            id = s.id
                        )
                    })?,
                    name,
                };
                anyhow::Ok(s)
            })
            .collect::<Result<_, _>>()?;
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
                if let Some(name) = &branch.name {
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
            extra_headers: vec![],
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
        but_graph::projection::commit::is_managed_workspace_by_message(self.message.as_bstr())
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
