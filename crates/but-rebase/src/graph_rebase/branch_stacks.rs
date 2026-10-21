//! How the local branches of a rewrite stack up, read off its step graph.

use std::collections::HashSet;

use anyhow::Result;
use but_core::{
    RefMetadata, WORKSPACE_REF_NAME,
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
    Checkout, Pick, Step, StepGraph, StepGraphIndex, SuccessfulRebase, traverse,
    workspace::is_workspace_commit,
};

/// The stacks of local branches in a rewrite, each from tip to base.
///
/// A stack follows first parents until it reaches the target or a step that an
/// earlier stack already owns, so no branch is in more than one of them. Like the
/// target itself, the references that merely rest on it are no branch of any stack.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BranchStacks {
    /// One stack per parent of the workspace commit, in parent order, or the stack `HEAD`
    /// is on if the workspace reference has no workspace commit beneath it. `None` if `HEAD`
    /// isn't on the workspace reference.
    pub workspace: Option<Vec<Vec<FullName>>>,
    /// The stack `HEAD` is on if it isn't on the workspace reference, followed by one
    /// stack per linked worktree.
    pub ad_hoc: Vec<Vec<FullName>>,
    /// Removed references
    pub removed_references: Vec<FullName>,
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
        workspace.stacks = stacks
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
        for branch in &self.removed_references {
            meta.remove(branch.as_ref())?;
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
        let base = match project_meta.target_commit_id {
            Some(id) => {
                let mappings = self.history.commit_mappings();
                let rewritten = mappings.get(&id).copied().unwrap_or(id);
                // An excluded boundary has no step. Do not replace it with another cutoff.
                find_pick(graph, rewritten)
            }
            None => {
                let target = project_meta.remote_target_ref(&self.repo)?.or_else(|| {
                    project_meta
                        .target_ref
                        .as_ref()
                        .map(|name| name.as_local_full_name_ref().to_owned())
                });
                // Prefer the rewritten ref position so pending ref-only moves are visible.
                // A traversal may omit the ref node but still include its tip commit.
                match target {
                    Some(name) => match find_reference(graph, name.as_ref()) {
                        Some(reference) => first_commit(graph, reference),
                        None => self
                            .repo
                            .try_find_reference(name.as_ref())?
                            .map(|mut reference| reference.peel_to_commit().map(|commit| commit.id))
                            .transpose()?
                            .and_then(|id| {
                                let mappings = self.history.commit_mappings();
                                let rewritten = mappings.get(&id).copied().unwrap_or(id);
                                find_pick(graph, rewritten)
                            }),
                    },
                    None => None,
                }
            }
        };

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
        let workspace_reference = head.filter(|head| {
            matches!(
                &graph[*head],
                Step::Reference { refname, .. } if refname.as_bstr() == WORKSPACE_REF_NAME
            )
        });
        let workspace_commit = workspace_reference.and_then(|head| {
            first_parents(graph, head)
                .take_while(|ix| Some(*ix) != base)
                .find(|ix| is_workspace_commit(graph, &self.repo, *ix))
        });

        let checked_out_in_worktrees: HashSet<_> = worktrees.iter().copied().collect();
        let workspace_ref: FullName = WORKSPACE_REF_NAME.try_into()?;
        let workspace_metadata = self.meta.workspace(workspace_ref.as_ref())?;
        let upstream: HashSet<_> = project_meta
            .remote_target_ref(&self.repo)?
            .as_ref()
            .and_then(|target_ref| find_reference(graph, target_ref.as_ref()))
            .and_then(|target_ref| first_commit(graph, target_ref))
            .or(base)
            .map(|tip| traverse::reachable_from(graph, tip).collect())
            .unwrap_or_default();
        let resting_on_base: HashSet<_> = base
            .and_then(|base| match &graph[base] {
                Step::Pick(Pick { id, .. }) => Some(*id),
                _ => None,
            })
            .and_then(|id| {
                let segment = self.workspace.graph.segment_by_commit_id(id).ok()?;
                segment.commits.iter().find(|commit| commit.id == id)
            })
            .into_iter()
            .flat_map(|commit| commit.refs.iter().map(|ri| &ri.ref_name))
            .collect();
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
                stack.extend(
                    self.stacked_branch(ix, &upstream, &workspace_metadata)
                        .filter(|name| !resting_on_base.contains(name)),
                );
            }
            stack
        };

        let workspace = workspace_reference.map(|workspace_reference| {
            let tips: Vec<_> = match workspace_commit {
                Some(commit) => {
                    let mut parents: Vec<_> =
                        graph.edges_directed(commit, Direction::Outgoing).collect();
                    parents.sort_by_key(|edge| edge.weight().order);
                    parents.into_iter().map(|edge| edge.target()).collect()
                }
                None => vec![workspace_reference],
            };
            tips.into_iter()
                .map(|tip| stack_from(tip, None))
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

        let removed_references = self
            .ref_edits
            .iter()
            .filter_map(|r| {
                if matches!(r.change, gix::refs::transaction::Change::Delete { .. }) {
                    Some(r.name.clone())
                } else {
                    None
                }
            })
            .collect();

        Ok(BranchStacks {
            workspace,
            ad_hoc,
            removed_references,
        })
    }

    fn stacked_branch(
        &self,
        ix: StepGraphIndex,
        upstream: &HashSet<StepGraphIndex>,
        workspace: &Workspace,
    ) -> Option<FullName> {
        let Step::Reference { refname, .. } = &self.graph[ix] else {
            return None;
        };
        let is_user_branch = refname.category() == Some(Category::LocalBranch)
            && !refname.as_bstr().starts_with_str("refs/heads/gitbutler/");
        // The target is not part of a stack while it is upstream, unless explicitly applied.
        let is_upstream_target_branch = first_commit(&self.graph, ix)
            .is_some_and(|commit| upstream.contains(&commit))
            && self.is_target(refname.as_ref())
            && workspace
                .find_owner_indexes_by_name(refname.as_ref(), StackKind::Applied)
                .is_none();
        (is_user_branch && !is_upstream_target_branch).then(|| refname.clone())
    }

    fn is_target(&self, name: &gix::refs::FullNameRef) -> bool {
        let Some(target_ref) = self.workspace.graph.project_meta.target_ref.as_ref() else {
            return false;
        };
        name == target_ref.as_local_full_name_ref()
    }
}
