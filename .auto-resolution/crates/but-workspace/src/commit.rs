use crate::WorkspaceCommit;
use crate::ui::StackEntryNoOpt;
use anyhow::Context;
use bstr::{BString, ByteSlice};

/// A minimal stack for use by [WorkspaceCommit::new_from_stacks()].
#[derive(Debug, Clone)]
pub struct Stack {
    /// The tip of the top-most branch, i.e., the most recent commit that would become the parent of new commits of the topmost stack branch.
    pub tip: gix::ObjectId,
    /// The short name of the stack, which is the name of the top-most branch, like `main` or `feature/branch` or `origin/tracking-some-PR`
    /// or something entirely made up.
    pub name: Option<BString>,
}

impl From<StackEntryNoOpt> for Stack {
    fn from(value: StackEntryNoOpt) -> Self {
        Stack {
            tip: value.tip,
            name: value.name().map(ToOwned::to_owned),
        }
    }
}

/// Structures related to creating a merge-commit along with the respective tree.
pub mod merge {
    use super::Stack;
    use crate::WorkspaceCommit;
    use anyhow::{Context, bail};
    use gitbutler_oxidize::GixRepositoryExt;
    use gix::prelude::ObjectIdExt;

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
        pub conflicting_stacks: Vec<()>,
    }

    impl Outcome {
        /// Return `true` if the outcome isn't perfect, as conflicts happened while merging that led to unmerged stacks.
        pub fn has_conflicts(&self) -> bool {
            !self.conflicting_stacks.is_empty()
        }
    }

    /// Merging - create a merge-commit along with its tree.
    impl WorkspaceCommit<'_> {
        /// Using the names of the `stacks` stored in [workspace metadata](but_core::ref_metadata::Workspace),
        /// create a new workspace commit with their tips extracted from `graph`. Note that stacks that don't exist in `graph` aren't fatal.
        ///
        /// Use `hero_stack` to highlight a stack that you definitely want merged in, and would rather not merge other stacks for it.
        /// This can lead to a situation where only the hero stack is applied.
        /// If there is only one stack, it just uses the tree of that stack. It's an error if `stacks` is empty.
        /// `repo` is expected to be configured to be suitable for merges, and it *should* be configured to write objects into memory
        /// unless the caller knows that any result of the merge is acceptable.
        ///
        /// IMPORTANT: This inherently needs the tips to be represented by named branches, so this can't be used to
        ///            re-merge a workspace with lost or renamed branches. It is, however, good to 'fix' workspaces
        ///            whose tips were advanced and now are outside the workspace.
        pub fn from_new_merge_with_metadata(
            stacks: &[but_core::ref_metadata::WorkspaceStack],
            graph: &but_graph::Graph,
            repo: &gix::Repository,
            hero_stack: Option<&gix::refs::FullNameRef>,
        ) -> anyhow::Result<Outcome> {
            let mut missing_stacks = Vec::new();
            let tips: Vec<_> = stacks
                .iter()
                .filter_map(|s| s.branches.first())
                .filter_map(|top_segment| {
                    let stack_tip_name = top_segment.ref_name.as_ref();
                    match graph.segment_and_commit_by_ref_name(stack_tip_name) {
                        None => {
                            missing_stacks.push(top_segment.ref_name.to_owned());
                            None
                        }
                        Some((segment, commit)) => Some((stack_tip_name, commit.id, segment.id)),
                    }
                })
                .collect();

            let conflicting_stacks = Vec::new();
            let mut prev_base_sidx = None;
            let mut merge_tree_id = None;
            let mut stacks = Vec::new();
            let mut previous_tip = None;
            let (merge_options, conflict_kind) = repo.merge_options_fail_fast()?;
            let labels_uninteresting_as_no_conflict_allowed = repo.default_merge_labels();
            for (ref_name, commit_id, sidx) in tips {
                let this_tree_id = peel_to_tree(commit_id.attach(repo))?;
                if let Some((prev_tree_id, prev_sidx)) = previous_tip {
                    let base_tree_id = {
                        // This is critical: we enforce using the lowest merge-base by using
                        // the previous iterations merge-base.
                        // This is the same as computing the merge-base between the new
                        // (non-existing merge-commit) and the next tip.
                        let first_sidx = prev_base_sidx.unwrap_or(prev_sidx);
                        let base_sidx =
                            graph.first_merge_base(first_sidx, sidx).with_context(|| {
                                format!(
                                    "Couldn't find merge-base between segments {l} and {r}",
                                    l = first_sidx.index(),
                                    r = sidx.index()
                                )
                            })?;
                        prev_base_sidx = Some(base_sidx);
                        let base_commit_id = graph.tip_skip_empty(base_sidx).with_context(|| {
                            format!(
                                "Base segment {base} between {l} and {r} didn't have  single commit reachable",
                                base = base_sidx.index(),
                                l = first_sidx.index(),
                                r = sidx.index()
                            )
                        })?.id.attach(repo);
                        peel_to_tree(base_commit_id)?
                    };

                    let mut merge = repo.merge_trees(
                        base_tree_id,
                        merge_tree_id.unwrap_or(prev_tree_id),
                        this_tree_id,
                        labels_uninteresting_as_no_conflict_allowed,
                        merge_options.clone(),
                    )?;
                    if merge.has_unresolved_conflicts(conflict_kind) {
                        bail!("TODO: conflict handling with hero-special: {hero_stack:?}");
                    }
                    merge_tree_id = merge.tree.write()?.detach().into();
                }
                stacks.push(Stack {
                    tip: commit_id,
                    name: Some(ref_name.shorten().to_owned()),
                });
                previous_tip = Some((this_tree_id, sidx));
            }

            if stacks.is_empty() {
                bail!("BUG: Cannot merge nothing, don't call me like that")
            }

            let merge_tree_id = merge_tree_id.unwrap_or_else(|| {
                // Just one stack?
                previous_tip
                    .expect("having stacks means the loop ran once")
                    .0
            });

            // Finally, create the merge-commit itself.
            let mut ws_commit = Self::new_from_stacks(stacks.iter().cloned(), repo.object_hash());
            ws_commit.tree = merge_tree_id;
            Self::fixup_times(&mut ws_commit, repo);

            let workspace_commit_id = repo.write_object(&ws_commit)?.detach();
            Ok(Outcome {
                workspace_commit_id,
                stacks,
                missing_stacks,
                conflicting_stacks,
            })
        }
    }

    fn peel_to_tree(commit: gix::Id) -> anyhow::Result<gix::ObjectId> {
        Ok(commit.object()?.peel_to_tree()?.id)
    }
}

/// Construction
impl<'repo> WorkspaceCommit<'repo> {
    const GITBUTLER_WORKSPACE_COMMIT_TITLE: &'static str = "GitButler Workspace Commit";

    /// Decode the object at `commit_id` and keep its data for later query.
    pub fn from_id(commit_id: gix::Id<'repo>) -> anyhow::Result<Self> {
        let commit = commit_id.object()?.try_into_commit()?.decode()?.into();
        Ok(WorkspaceCommit {
            id: commit_id,
            inner: commit,
        })
    }

    /// A way to create a commit from `workspace` stacks, with the `tree` being used as the tree of the workspace commit.
    /// It's supposed to be the legitimate merge of the stacks contained in `workspace`.
    /// Note that it will be written to `repo` immediately for persistence, with its object id returned.
    pub fn from_graph_workspace(
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
                    tip: s.tip_skip_empty().or(s.base()).with_context(|| format!("Could not find any commit to serve as tip for stack {id:?} with name {name:?}", id = s.id))?,
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

    /// also rewrite the author and commiter time, just to be sure we respect all settings. `new_from_stacks` doesn't have a repo.
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
            message.push_str("This is placeholder commit and will be replaced by a merge of your virtual branches.\n\n");
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
