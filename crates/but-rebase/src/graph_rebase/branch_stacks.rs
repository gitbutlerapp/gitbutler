//! How the local branches of a rewrite stack up, read off its step graph.

use std::collections::HashSet;

use anyhow::Result;
use but_core::{
    RefMetadata, WORKSPACE_REF_NAME,
    branch::resolve_tracking_branch_ref_name,
    ref_metadata::{
        StackId, StackKind, Workspace, WorkspaceCommitRelation, WorkspaceStack,
        WorkspaceStackBranch,
    },
};
use gix::{
    bstr::ByteSlice as _,
    refs::{Category, FullName},
};
use petgraph::{Direction, visit::EdgeRef as _};

use crate::graph_rebase::{
    Checkout, Pick, Step, StepGraph, StepGraphIndex, SuccessfulRebase,
    workspace::is_workspace_commit,
};

/// The stacks of local branches in a rewrite, each from tip to base.
///
/// A stack follows first parents until it reaches the target or a step that an
/// earlier stack already owns, so no branch is in more than one of them.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BranchStacks {
    /// One stack per parent of the workspace commit, in parent order, or `None` if
    /// `HEAD` isn't on the workspace reference with a workspace commit beneath it.
    pub workspace: Option<Vec<Vec<FullName>>>,
    /// The stack `HEAD` is on if there is no workspace commit, followed by one
    /// stack per linked worktree.
    pub ad_hoc: Vec<Vec<FullName>>,
}

impl BranchStacks {
    /// The workspace metadata once it describes these stacks, if `HEAD` is in a workspace.
    ///
    /// A stack keeps the id and the position of the stack that held its lowest known branch
    /// before, and stacks that are new to the metadata follow in parent order.
    pub(crate) fn workspace_metadata<M: RefMetadata>(
        &self,
        meta: &M,
    ) -> Result<Option<M::Handle<Workspace>>> {
        let Some(stacks) = &self.workspace else {
            return Ok(None);
        };
        let workspace_ref: FullName = WORKSPACE_REF_NAME.try_into()?;
        let mut workspace = meta.workspace(workspace_ref.as_ref())?;
        let mut kept_ids = HashSet::new();
        let mut stacks: Vec<_> = stacks
            .iter()
            .map(|branches| WorkspaceStack {
                id: branches
                    .iter()
                    .rev()
                    .find_map(|branch| {
                        let (stack, _) = workspace.find_owner_indexes_by_name(
                            branch.as_ref(),
                            StackKind::AppliedAndUnapplied,
                        )?;
                        Some(workspace.stacks.get(stack)?.id)
                    })
                    .filter(|id| kept_ids.insert(*id))
                    .unwrap_or_else(StackId::generate),
                branches: branches
                    .iter()
                    .map(|ref_name| WorkspaceStackBranch {
                        ref_name: ref_name.clone(),
                        archived: false,
                    })
                    .collect(),
                workspacecommit_relation: WorkspaceCommitRelation::Merged,
            })
            .collect();
        stacks.sort_by_key(|stack| {
            workspace
                .stacks
                .iter()
                .position(|previous| previous.id == stack.id)
                .unwrap_or(usize::MAX)
        });
        workspace.stacks = stacks;
        Ok(Some(workspace))
    }

    /// Make `meta` describe these stacks, returning whether that changed anything.
    pub(crate) fn persist<M: RefMetadata>(&self, meta: &mut M) -> Result<bool> {
        let mut changed = false;
        if let Some(workspace) = self.workspace_metadata(meta)?
            && *meta.workspace(workspace.as_ref())? != *workspace
        {
            meta.set_workspace(&workspace)?;
            changed = true;
        }
        if meta.can_persist_branch_stack_order() {
            for stack in &self.ad_hoc {
                let Some(tip) = stack.first() else { continue };
                if meta.branch_stack_order(tip.as_ref())?.as_ref() != Some(stack) {
                    meta.set_branch_stack_order(stack)?;
                    changed = true;
                }
            }
        }
        Ok(changed)
    }
}

fn first_parents(
    graph: &StepGraph,
    start: StepGraphIndex,
) -> impl Iterator<Item = StepGraphIndex> + '_ {
    std::iter::successors(Some(start), |ix| {
        graph
            .edges_directed(*ix, Direction::Outgoing)
            .min_by_key(|edge| edge.weight().order)
            .map(|edge| edge.target())
    })
}

fn first_commit(graph: &StepGraph, start: StepGraphIndex) -> Option<StepGraphIndex> {
    first_parents(graph, start).find(|ix| matches!(graph[*ix], Step::Pick(_)))
}

fn find_pick(graph: &StepGraph, id: gix::ObjectId) -> Option<StepGraphIndex> {
    graph
        .node_indices()
        .find(|ix| matches!(&graph[*ix], Step::Pick(Pick { id: pick, .. }) if *pick == id))
}

fn find_reference(graph: &StepGraph, name: &gix::refs::FullNameRef) -> Option<StepGraphIndex> {
    graph.node_indices().find(
        |ix| matches!(&graph[*ix], Step::Reference { refname, .. } if refname.as_ref() == name),
    )
}

impl<M: RefMetadata> SuccessfulRebase<'_, '_, M> {
    /// The stacks of local branches this rewrite results in.
    pub fn branch_stacks(&self) -> Result<BranchStacks> {
        let graph = &self.graph;
        let project_meta = &self.workspace.graph.project_meta;
        let base = project_meta
            .target_commit_id
            .and_then(|id| find_pick(graph, id))
            .or_else(|| {
                let target_ref = project_meta.target_ref.as_ref()?;
                first_commit(graph, find_reference(graph, target_ref.as_ref())?)
            });

        let mut head = None;
        let mut worktrees = Vec::new();
        for checkout in &self.checkouts {
            match checkout {
                Checkout::Head { selector, .. } => {
                    head = Some(self.history.normalize_selector(*selector)?.id)
                }
                Checkout::Worktree { selector, .. } => {
                    worktrees.push(self.history.normalize_selector(*selector)?.id)
                }
            }
        }
        let head = head.or_else(|| find_pick(graph, self.repo.head_id().ok()?.detach()));
        let workspace_commit = head
            .filter(|head| {
                matches!(
                    &graph[*head],
                    Step::Reference { refname, .. } if refname.as_bstr() == WORKSPACE_REF_NAME
                )
            })
            .and_then(|head| {
                first_parents(graph, head)
                    .take_while(|ix| Some(*ix) != base)
                    .find(|ix| is_workspace_commit(graph, &self.repo, *ix))
            });

        let checked_out_in_worktrees: HashSet<_> = worktrees.iter().copied().collect();
        let mut owned = HashSet::new();
        let mut stack_from = |start: StepGraphIndex, own_checkout: Option<StepGraphIndex>| {
            let mut stack = Vec::new();
            for ix in first_parents(graph, start).take_while(|ix| Some(*ix) != base) {
                if checked_out_in_worktrees.contains(&ix) && Some(ix) != own_checkout {
                    continue;
                }
                if !owned.insert(ix) {
                    break;
                }
                stack.extend(self.stacked_branch(ix, base));
            }
            stack
        };

        let workspace = workspace_commit.map(|commit| {
            let mut parents: Vec<_> = graph.edges_directed(commit, Direction::Outgoing).collect();
            parents.sort_by_key(|edge| edge.weight().order);
            parents
                .into_iter()
                .map(|edge| stack_from(edge.target(), None))
                .filter(|stack| !stack.is_empty())
                .collect()
        });
        let head_stack = match workspace {
            Some(_) => None,
            None => head.map(|head| stack_from(head, None)),
        };
        let ad_hoc = head_stack
            .into_iter()
            .chain(
                worktrees
                    .iter()
                    .map(|worktree| stack_from(*worktree, Some(*worktree))),
            )
            .filter(|stack| !stack.is_empty())
            .collect();

        Ok(BranchStacks { workspace, ad_hoc })
    }

    fn stacked_branch(&self, ix: StepGraphIndex, base: Option<StepGraphIndex>) -> Option<FullName> {
        let Step::Reference { refname, .. } = &self.graph[ix] else {
            return None;
        };
        let is_user_branch = refname.category() == Some(Category::LocalBranch)
            && !refname.as_bstr().starts_with_str("refs/heads/gitbutler/");
        let is_the_target_branch = base.is_some()
            && first_commit(&self.graph, ix) == base
            && self.tracks_target(refname.as_ref());
        (is_user_branch && !is_the_target_branch).then(|| refname.clone())
    }

    fn tracks_target(&self, name: &gix::refs::FullNameRef) -> bool {
        let Some(target_ref) = self.workspace.graph.project_meta.target_ref.as_ref() else {
            return false;
        };
        resolve_tracking_branch_ref_name(name, &self.repo)
            .is_ok_and(|tracking| tracking.as_ref() == target_ref.as_ref())
    }
}
