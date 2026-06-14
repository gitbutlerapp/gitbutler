use std::collections::{BTreeMap, BTreeSet};

use but_core::ref_metadata::StackId;
use but_graph::workspace::StackCommitDebugFlags;
use termtree::Tree;

type StringTree = Tree<String>;

/// Visualize `graph` as a tree.
pub fn graph_workspace(workspace: &but_graph::Workspace) -> StringTree {
    graph_workspace_inner(workspace, None)
}

/// Visualize `graph` as a tree, and remap random stack ids to something deterministic.
pub fn graph_workspace_determinisitcally(workspace: &but_graph::Workspace) -> StringTree {
    graph_workspace_inner(workspace, Some(Default::default()))
}

fn graph_workspace_inner(
    workspace: &but_graph::Workspace,
    mut stack_id_map: Option<BTreeMap<StackId, StackId>>,
) -> StringTree {
    let commit_flags = if workspace.hard_limit_hit {
        StackCommitDebugFlags::HardLimitReached
    } else {
        Default::default()
    };
    let mut root = Tree::new(workspace.debug_string());
    for stack in &workspace.stacks {
        root.push(tree_for_stack(
            workspace.has_multiple_worktrees,
            stack,
            commit_flags,
            stack_id_map.as_mut(),
        ));
    }
    root
}

fn tree_for_stack(
    has_multiple_worktrees: bool,
    stack: &but_graph::workspace::Stack,
    commit_flags: StackCommitDebugFlags,
    stack_id_map: Option<&mut BTreeMap<StackId, StackId>>,
) -> StringTree {
    let mut root = Tree::new(stack.debug_string_with_worktree_context(
        has_multiple_worktrees,
        stack.id.zip(stack_id_map).map(|(id, map)| {
            let next_id = StackId::from_number_for_testing((map.len() + 1) as u128);
            *map.entry(id).or_insert(next_id)
        }),
    ));
    for segment in &stack.segments {
        root.push(tree_for_stack_segment(
            has_multiple_worktrees,
            segment,
            commit_flags,
        ));
    }
    root
}

fn tree_for_stack_segment(
    has_multiple_worktrees: bool,
    segment: &but_graph::workspace::StackSegment,
    commit_flags: StackCommitDebugFlags,
) -> StringTree {
    let mut root = Tree::new(segment.debug_string_with_worktree_context(has_multiple_worktrees));
    if let Some(outside) = &segment.commits_outside {
        for commit in outside {
            root.push(format!("{}*", commit.debug_string(commit_flags)));
        }
    }
    for commit in &segment.commits_on_remote {
        root.push(commit.debug_string(commit_flags | StackCommitDebugFlags::RemoteOnly));
    }
    for commit in &segment.commits {
        root.push(commit.debug_string(commit_flags));
    }
    root
}

/// Visualize the workspace's [`BranchGraph`](but_graph::BranchGraph) — the canonical full-topology
/// structure (named runs of commits, including the empty routing nodes the commit graph can't
/// express) — as a tree, rendered straight from what the `Workspace` carries, with no record graph.
pub fn branch_tree(workspace: &but_graph::Workspace) -> StringTree {
    let branches = workspace.branches().unwrap_or_default();
    if branches.is_empty() {
        return "<UNBORN>".to_string().into();
    }
    let hard = workspace.hard_limit_hit;
    // Roots are the tips: branches no other branch lists as an outgoing (parent) edge.
    let referenced: BTreeSet<usize> = branches
        .iter()
        .flat_map(|b| b.outgoing.iter().map(|(idx, _)| *idx))
        .collect();
    let mut root = Tree::new(String::new());
    let mut seen = BTreeSet::new();
    for idx in (0..branches.len()).filter(|idx| !referenced.contains(idx)) {
        root.push(recurse_branch(branches, idx, hard, &mut seen));
    }
    // Anything not reachable from a tip (shouldn't happen) is surfaced rather than hidden.
    for idx in 0..branches.len() {
        if !seen.contains(&idx) {
            root.push(recurse_branch(branches, idx, hard, &mut seen));
        }
    }
    root
}

fn recurse_branch(
    branches: &[but_graph::branch_graph::Branch],
    idx: usize,
    hard_limit_hit: bool,
    seen: &mut BTreeSet<usize>,
) -> StringTree {
    let branch = &branches[idx];
    let name = branch
        .ref_name
        .as_ref()
        .map(|n| format!("►{}", n.shorten()))
        .unwrap_or_else(|| "►anon:".to_string());
    let entrypoint = if branch.is_entrypoint { "👉" } else { "" };
    if seen.contains(&idx) {
        return format!("→:{idx}:{name}").into();
    }
    seen.insert(idx);
    let mut root = Tree::new(format!("{entrypoint}:{idx}:{name}"));
    let last = branch.commits.len().saturating_sub(1);
    for (i, commit) in branch.commits.iter().enumerate() {
        let refs = commit
            .refs
            .iter()
            .map(|ri| ri.debug_string())
            .collect::<Vec<_>>()
            .join(", ");
        let refs = if refs.is_empty() {
            String::new()
        } else {
            format!(" {refs}")
        };
        // A branch with no outgoing (parent) edge dead-ends at its last commit: mark whether that's
        // a true root (🏁, no parents) or a truncation by a traversal limit (✂, or ❌ for a hard
        // limit). Derived from the commit's own parents, so it needs no record graph.
        let stop = if i == last && branch.outgoing.is_empty() {
            if commit.parent_ids.is_empty() {
                "🏁"
            } else if hard_limit_hit {
                "❌"
            } else {
                "✂"
            }
        } else {
            ""
        };
        root.push(format!(
            "{stop}·{} ({}){}",
            commit.id.to_hex_with_len(7),
            commit.flags.debug_string(None),
            refs
        ));
    }
    for (parent_idx, _order) in &branch.outgoing {
        root.push(recurse_branch(branches, *parent_idx, hard_limit_hit, seen));
    }
    root
}
