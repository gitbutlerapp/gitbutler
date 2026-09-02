use std::collections::{BTreeMap, BTreeSet};

use but_graph::{Commit, CommitFlags, StopCondition, Workspace, branch_graph::Branch};
use renderdag::{Ancestor, GraphRowRenderer, Renderer as _};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
enum Node {
    Commit(gix::ObjectId),
    Reference(gix::refs::FullName),
}

struct RenderNode {
    glyph: &'static str,
    label: String,
    parents: Vec<Node>,
}

/// Render the commit and reference DAG of the workspace's [`BranchGraph`](but_graph::BranchGraph)
/// without exposing branch indices: commits are `●`, references `◎`, and a reference sits between
/// the commits that connect through it.
pub fn graph_dag(workspace: &Workspace) -> String {
    let branches: Vec<Branch> = workspace.branches().unwrap_or_default().to_vec();
    let commit_ids: BTreeSet<_> = branches
        .iter()
        .flat_map(|branch| branch.commits.iter().map(|commit| commit.id))
        .collect();
    let entrypoint_branch = branches.iter().position(|branch| branch.is_entrypoint);
    // A detached `HEAD` has no ref of its own: the entrypoint is the commit, even when the
    // traversal named the run it sits on after a branch pointing there.
    let is_detached = workspace.ref_name().is_none();
    let detached_entrypoint_id = entrypoint_branch
        .map(|idx| &branches[idx])
        .filter(|branch| branch.ref_name.is_none() || is_detached)
        .and_then(|branch| branch.commits.first())
        .map(|commit| commit.id)
        .or_else(|| {
            is_detached
                .then_some(workspace.entrypoint_commit_id)
                .flatten()
        });
    let commit_graph = workspace.commit_graph();
    let stop_condition = |commit: &Commit| -> Option<StopCondition> {
        let mut condition = StopCondition::empty();
        if commit.parent_ids.is_empty() {
            condition |= StopCondition::FirstCommit;
        } else if commit_graph.walked_parent_count(commit.id) == 0 {
            condition |= StopCondition::Limit;
        }
        if commit.flags.contains(CommitFlags::ShallowBoundary) {
            condition |= StopCondition::ShallowBoundary;
        }
        (!condition.is_empty()).then_some(condition)
    };
    let show_worktrees = workspace.has_multiple_worktrees;

    let mut nodes = BTreeMap::new();
    for branch in &branches {
        for (cidx, commit) in branch.commits.iter().enumerate() {
            // Parents come from the record: the next commit of the run, or, for the last one,
            // the branches it connects to, in parent order - a reference sits between the
            // commits that connect through it.
            let mut parents: Vec<Node> = match branch.commits.get(cidx + 1) {
                Some(next) => vec![Node::Commit(next.id)],
                None => {
                    let mut children: Vec<_> = branch.outgoing.clone();
                    children.sort_by_key(|(_, order)| *order);
                    children
                        .into_iter()
                        .flat_map(|(child, _)| branch_entry_nodes(&branches, child))
                        .collect()
                }
            };
            if parents.is_empty() {
                parents = commit
                    .parent_ids
                    .iter()
                    .filter(|id| commit_ids.contains(*id))
                    .copied()
                    .map(Node::Commit)
                    .collect();
            }
            deduplicate(&mut parents);
            nodes.insert(
                Node::Commit(commit.id),
                RenderNode {
                    glyph: "●",
                    label: commit_label(
                        commit,
                        detached_entrypoint_id == Some(commit.id),
                        stop_condition(commit),
                        workspace.hard_limit_hit,
                        show_worktrees,
                    ),
                    parents,
                },
            );
        }

        let Some(ref_name) = branch.ref_name.as_ref() else {
            continue;
        };
        let mut parents = branch
            .commits
            .first()
            .map(|commit| vec![Node::Commit(commit.id)])
            .unwrap_or_else(|| {
                branch
                    .outgoing
                    .iter()
                    .flat_map(|(child, _)| branch_entry_nodes(&branches, *child))
                    .collect()
            });
        deduplicate(&mut parents);
        nodes.insert(
            Node::Reference(ref_name.clone()),
            RenderNode {
                glyph: "◎",
                label: reference_label(workspace, branch, show_worktrees, is_detached),
                parents,
            },
        );
    }

    for branch in &branches {
        for commit in &branch.commits {
            for ref_info in &commit.refs {
                let reference = Node::Reference(ref_info.ref_name.clone());
                nodes
                    .entry(reference)
                    .and_modify(|node| {
                        if node.parents.is_empty() {
                            node.parents.push(Node::Commit(commit.id));
                        }
                    })
                    .or_insert_with(|| RenderNode {
                        glyph: "◎",
                        label: ref_debug_string(
                            ref_info.ref_name.as_ref(),
                            ref_info.worktree.as_ref(),
                            show_worktrees,
                        ),
                        parents: vec![Node::Commit(commit.id)],
                    });
            }
        }
    }

    if nodes.is_empty() {
        return "<UNBORN>".into();
    }

    let mut renderer = GraphRowRenderer::<Node>::new()
        .output()
        .with_min_row_height(1)
        .build_box_drawing();
    let mut out = String::new();
    for node in topological_order(&nodes) {
        let rendered = &nodes[&node];
        out.push_str(
            &renderer.next_row(
                node,
                rendered
                    .parents
                    .iter()
                    .cloned()
                    .map(Ancestor::Parent)
                    .collect(),
                rendered.glyph.into(),
                rendered.label.clone(),
            ),
        );
    }
    out.trim_end().into()
}

fn commit_label(
    commit: &Commit,
    is_entrypoint: bool,
    stop_condition: Option<StopCondition>,
    hard_limit: bool,
    show_worktrees: bool,
) -> String {
    let _ = show_worktrees;
    format!(
        "{ep}{end}{kind}{hex}{flags}",
        ep = if is_entrypoint { "👉" } else { "" },
        end = stop_condition
            .map(|condition| condition.debug_string(hard_limit))
            .unwrap_or_default(),
        kind = if commit.flags.is_remote() {
            "🟣"
        } else {
            "·"
        },
        hex = commit.id.to_hex_with_len(7),
        flags = if !commit.flags.is_empty() {
            format!(" ({})", commit.flags.debug_string())
        } else {
            "".to_string()
        },
    )
}

fn reference_label(
    workspace: &Workspace,
    branch: &Branch,
    show_worktrees: bool,
    is_detached: bool,
) -> String {
    let ref_name = branch.ref_name.as_ref().expect("reference branch");
    let is_entrypoint = branch.is_entrypoint && !is_detached;
    // Metadata and remote pairing live on the projected stack segments; the branch graph carries
    // only names and topology.
    let segment = workspace
        .stacks
        .iter()
        .flat_map(|stack| stack.segments.iter())
        .find(|segment| segment.ref_name() == Some(ref_name.as_ref()));
    let is_workspace = workspace.ref_name() == Some(ref_name.as_ref()) && workspace.has_metadata();
    // Branch metadata: from the stack row, else from the workspace's stack listing for branches
    // the projection keeps out of the rows (like ones checked out in linked worktrees).
    let listed_in_workspace = workspace.metadata.as_ref().is_some_and(|md| {
        md.stacks(but_core::ref_metadata::StackKind::Applied)
            .any(|stack| stack.branches.iter().any(|b| b.ref_name == *ref_name))
    });
    let metadata = match &branch.metadata {
        Some(but_graph::SegmentMetadata::Workspace(_)) => "📕",
        Some(but_graph::SegmentMetadata::Branch(_)) => "📙",
        None if is_workspace => "📕",
        None if segment.is_some_and(|s| s.metadata.is_some()) || listed_in_workspace => "📙",
        None => "",
    };
    // The remote pairing: from the stack row, else deduced by name like the traversal does for
    // any local branch whose remote tracking branch is part of the graph.
    let deduced_remote = || -> Option<gix::refs::FullName> {
        use bstr::ByteSlice;
        let short = ref_name.as_bstr().strip_prefix(b"refs/heads/")?;
        workspace.symbolic_remote_names.iter().find_map(|remote| {
            let candidate: gix::refs::FullName =
                format!("refs/remotes/{remote}/{}", short.as_bstr())
                    .try_into()
                    .ok()?;
            workspace
                .branches()
                .unwrap_or_default()
                .iter()
                .any(|b| b.ref_name.as_ref() == Some(&candidate))
                .then_some(candidate)
        })
    };
    let remote = segment
        .and_then(|s| s.remote_tracking_ref_name.clone())
        .or_else(|| {
            (ref_name.category() == Some(gix::reference::Category::LocalBranch))
                .then(deduced_remote)
                .flatten()
        })
        .map(|name| format!(" <> {}", ref_debug_string(name.as_ref(), None, false)))
        .unwrap_or_default();
    let worktree = branch
        .worktree
        .as_ref()
        .or_else(|| {
            segment
                .and_then(|s| s.ref_info.as_ref())
                .and_then(|ri| ri.worktree.as_ref())
        })
        .or_else(|| {
            workspace
                .ref_info
                .as_ref()
                .filter(|ri| ri.ref_name == *ref_name)
                .and_then(|ri| ri.worktree.as_ref())
        })
        .or_else(|| {
            workspace
                .stacks
                .iter()
                .flat_map(|stack| stack.segments.iter())
                .flat_map(|s| s.commits.iter().flat_map(|c| c.refs.iter()))
                .find(|ri| ri.ref_name == *ref_name)
                .and_then(|ri| ri.worktree.as_ref())
        });
    format!(
        "{}{metadata}{}{remote}",
        if is_entrypoint { "👉" } else { "" },
        ref_debug_string(ref_name.as_ref(), worktree, show_worktrees)
    )
}

/// Shorten `ref_name` so it's still clear if it is a special ref (like tag) or not.
fn ref_debug_string(
    ref_name: &gix::refs::FullNameRef,
    worktree: Option<&but_graph::Worktree>,
    show_owned_by_repo: bool,
) -> String {
    use bstr::ByteSlice;
    use gix::reference::Category;
    let (cat, sn) = ref_name.category_and_short_name().expect("valid refs");
    format!(
        "{}{ws}",
        if matches!(cat, Category::LocalBranch | Category::RemoteBranch) {
            sn
        } else {
            ref_name
                .as_bstr()
                .strip_prefix(b"refs/")
                .map(|n| n.as_bstr())
                .unwrap_or(ref_name.as_bstr())
        },
        ws = worktree
            .map(|wt| wt.debug_string_with_graph_context(ref_name, show_owned_by_repo))
            .unwrap_or_default()
    )
}

fn branch_entry_nodes(branches: &[Branch], idx: usize) -> Vec<Node> {
    fn recurse(branches: &[Branch], idx: usize, seen: &mut BTreeSet<usize>) -> Vec<Node> {
        if !seen.insert(idx) {
            return Vec::new();
        }
        let branch = &branches[idx];
        let nodes = if let Some(ref_name) = branch.ref_name.as_ref() {
            vec![Node::Reference(ref_name.clone())]
        } else if let Some(commit) = branch.commits.first() {
            vec![Node::Commit(commit.id)]
        } else {
            let mut children: Vec<_> = branch.outgoing.clone();
            children.sort_by_key(|(_, order)| *order);
            children
                .into_iter()
                .flat_map(|(child, _)| recurse(branches, child, seen))
                .collect()
        };
        seen.remove(&idx);
        nodes
    }

    let mut nodes = recurse(branches, idx, &mut BTreeSet::new());
    deduplicate(&mut nodes);
    nodes
}

fn deduplicate(nodes: &mut Vec<Node>) {
    let mut seen = BTreeSet::new();
    nodes.retain(|node| seen.insert(node.clone()));
}

fn topological_order(nodes: &BTreeMap<Node, RenderNode>) -> Vec<Node> {
    let mut children = nodes
        .keys()
        .cloned()
        .map(|node| (node, 0usize))
        .collect::<BTreeMap<_, _>>();
    for rendered in nodes.values() {
        for parent in &rendered.parents {
            if let Some(count) = children.get_mut(parent) {
                *count += 1;
            }
        }
    }

    fn visit(
        node: Node,
        nodes: &BTreeMap<Node, RenderNode>,
        children: &mut BTreeMap<Node, usize>,
        seen: &mut BTreeSet<Node>,
        out: &mut Vec<Node>,
    ) {
        if children[&node] != 0 || !seen.insert(node.clone()) {
            return;
        }
        out.push(node.clone());
        for parent in &nodes[&node].parents {
            children
                .entry(parent.clone())
                .and_modify(|count| *count -= 1);
        }
        for parent in &nodes[&node].parents {
            visit(parent.clone(), nodes, children, seen, out);
        }
    }

    let tips = children
        .iter()
        .filter_map(|(node, children)| (*children == 0).then_some(node.clone()))
        .collect::<Vec<_>>();
    let mut out = Vec::with_capacity(nodes.len());
    let mut seen = BTreeSet::new();
    for tip in tips {
        visit(tip, nodes, &mut children, &mut seen, &mut out);
    }
    out
}

/// Structure queries over the workspace's branch records, for tests that used to ask the record
/// graph about its segments.
pub mod topology {
    use but_graph::{CommitFlags, StopCondition, Workspace, branch_graph::Branch};

    /// The branch records the workspace carries.
    pub fn branches(ws: &Workspace) -> &[Branch] {
        ws.branches().unwrap_or_default()
    }

    /// The branch named `name`, with its index.
    pub fn branch_by_ref<'a>(
        ws: &'a Workspace,
        name: &gix::refs::FullNameRef,
    ) -> Option<(usize, &'a Branch)> {
        branches(ws)
            .iter()
            .enumerate()
            .find(|(_, b)| b.ref_name.as_ref().map(|rn| rn.as_ref()) == Some(name))
    }

    /// The branches nothing connects to: the tips.
    pub fn tip_branches(ws: &Workspace) -> Vec<usize> {
        let branches = branches(ws);
        let referenced: std::collections::BTreeSet<usize> = branches
            .iter()
            .flat_map(|b| b.outgoing.iter().map(|(t, _)| *t))
            .collect();
        (0..branches.len())
            .filter(|idx| !referenced.contains(idx))
            .collect()
    }

    /// The branches that connect to nothing: the bases.
    pub fn base_branches(ws: &Workspace) -> Vec<usize> {
        branches(ws)
            .iter()
            .enumerate()
            .filter(|(_, b)| b.outgoing.is_empty())
            .map(|(idx, _)| idx)
            .collect()
    }

    /// The branches connecting into `idx`, with the parent order of the connection.
    pub fn incoming(ws: &Workspace, idx: usize) -> Vec<(usize, u32)> {
        branches(ws)
            .iter()
            .enumerate()
            .flat_map(|(source, b)| {
                b.outgoing
                    .iter()
                    .filter(|(t, _)| *t == idx)
                    .map(move |(_, order)| (source, *order))
            })
            .collect()
    }

    /// The branches whose history was cut short by a traversal limit: their last commit has
    /// parents the traversal never walked.
    pub fn partial_branches(ws: &Workspace) -> Vec<usize> {
        let commit_graph = ws.commit_graph();
        branches(ws)
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.commits.last().is_some_and(|c| {
                    !c.parent_ids.is_empty() && commit_graph.walked_parent_count(c.id) == 0
                })
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    /// The number of connections between branches.
    pub fn num_connections(ws: &Workspace) -> usize {
        branches(ws).iter().map(|b| b.outgoing.len()).sum()
    }

    /// The branch owning `commit_id` and the commit's position in it.
    pub fn find_commit(ws: &Workspace, commit_id: gix::ObjectId) -> Option<(usize, usize)> {
        branches(ws).iter().enumerate().find_map(|(idx, b)| {
            b.commits
                .iter()
                .position(|c| c.id == commit_id)
                .map(|cidx| (idx, cidx))
        })
    }

    /// Why the traversal stopped below the last commit of branch `idx`, if it did.
    pub fn stop_condition(ws: &Workspace, idx: usize) -> Option<StopCondition> {
        let commit = branches(ws).get(idx)?.commits.last()?;
        let mut condition = StopCondition::empty();
        if commit.flags.contains(CommitFlags::ShallowBoundary) {
            condition |= StopCondition::ShallowBoundary;
        } else if commit.parent_ids.is_empty() {
            condition |= StopCondition::FirstCommit;
        } else if ws.commit_graph().walked_parent_count(commit.id) == 0 {
            condition |= StopCondition::Limit;
        }
        (!condition.is_empty()).then_some(condition)
    }

    /// The commit branch `idx` resolves to, navigating past empty branches through single
    /// connections; `None` when a fork makes that ambiguous.
    pub fn tip_skip_empty(ws: &Workspace, mut idx: usize) -> Option<gix::ObjectId> {
        let branches = branches(ws);
        for _ in 0..branches.len().max(1) {
            let branch = branches.get(idx)?;
            if let Some(commit) = branch.commits.first() {
                return Some(commit.id);
            }
            match branch.outgoing.as_slice() {
                [(next, _)] => idx = *next,
                _ => return None,
            }
        }
        None
    }
}
