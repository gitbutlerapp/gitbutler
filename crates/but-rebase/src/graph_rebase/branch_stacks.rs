//! How the local branches of a rewrite stack up, read off its step graph.

use std::collections::HashSet;

use anyhow::Result;
use but_core::{
    WORKSPACE_REF_NAME,
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
    Checkout, Pick, Step, StepGraph, StepGraphIndex, SuccessfulRebase, traverse,
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
    /// Removed references
    pub removed_references: Vec<FullName>,
}

impl BranchStacks {
    /// The workspace metadata once it describes these stacks, if `HEAD` is in a workspace.
    ///
    /// A stack keeps the id and the position of the stack that held its lowest known branch
    /// before, and stacks that are new to the metadata follow in parent order.
    pub(crate) fn workspace_metadata(&self, meta: &but_db::Metadata) -> Result<Option<Workspace>> {
        let Some(stacks) = &self.workspace else {
            return Ok(None);
        };
        let workspace_ref: FullName = WORKSPACE_REF_NAME.try_into()?;
        let mut workspace = meta
            .workspace(workspace_ref.as_ref())
            .cloned()
            .unwrap_or_default();
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
    pub(crate) fn persist(&self, db: &mut but_db::ConnectionMut<'_, '_>) -> Result<bool> {
        let meta = db.meta()?;
        let mut changed = false;
        if let Some(workspace) = self.workspace_metadata(&meta)? {
            let workspace_ref: FullName = WORKSPACE_REF_NAME.try_into()?;
            if meta.workspace(workspace_ref.as_ref()) != Some(&workspace) {
                db.meta_mut()?
                    .set_workspace(workspace_ref.as_ref(), &workspace)?;
                changed = true;
            }
        }
        // Workspace writes may remove the inherited order of linked-worktree branches.
        let meta = db.meta()?;
        for stack in &self.ad_hoc {
            let Some(tip) = stack.first() else { continue };
            if meta.branch_stack_order(tip.as_ref()) != Some(stack.as_slice()) {
                db.meta_mut()?.set_branch_stack_order(stack)?;
                changed = true;
            }
        }
        for branch in &self.removed_references {
            db.meta_mut()?.remove(branch.as_ref())?;
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

impl SuccessfulRebase<'_, '_, '_> {
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
        let workspace_ref: FullName = WORKSPACE_REF_NAME.try_into()?;
        let workspace_metadata = self
            .db
            .meta()?
            .workspace(workspace_ref.as_ref())
            .cloned()
            .unwrap_or_default();
        let upstream: HashSet<_> = project_meta
            .target_ref
            .as_ref()
            .and_then(|target_ref| find_reference(graph, target_ref.as_ref()))
            .and_then(|target_ref| first_commit(graph, target_ref))
            .or(base)
            .map(|tip| traverse::reachable_from(graph, tip).collect())
            .unwrap_or_default();
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
                stack.extend(self.stacked_branch(ix, &upstream, &workspace_metadata));
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
        // The local counterpart of the target often sits at the base of stacks, and is no
        // branch of theirs until it has commits of its own, or the metadata says so.
        let is_upstream_target_branch = first_commit(&self.graph, ix)
            .is_some_and(|commit| upstream.contains(&commit))
            && self.tracks_target(refname.as_ref())
            && workspace
                .find_owner_indexes_by_name(refname.as_ref(), StackKind::Applied)
                .is_none();
        (is_user_branch && !is_upstream_target_branch).then(|| refname.clone())
    }

    fn tracks_target(&self, name: &gix::refs::FullNameRef) -> bool {
        let Some(target_ref) = self.workspace.graph.project_meta.target_ref.as_ref() else {
            return false;
        };
        resolve_tracking_branch_ref_name(name, &self.repo)
            .is_ok_and(|tracking| tracking.as_ref() == target_ref.as_ref())
    }
}
