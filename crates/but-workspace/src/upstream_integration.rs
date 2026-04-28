//! Integrating upstream changes

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};

use but_core::RefMetadata;
use but_graph::{
    petgraph::algo::k_shortest_path, projection::commit::is_managed_workspace_by_message,
};
use but_rebase::{
    commit::DateMode,
    graph_rebase::{
        Editor, GraphEditorOptions, LookupStep, Pick, Selector, Step, SuccessfulRebase, ToSelector,
        mutate::{InsertSide, RelativeTo},
    },
};

/// Whether a bottom most commit should be rebased, or a merge commit should be
/// created at the top of the commit run.
#[derive(Clone, Copy, PartialEq)]
pub enum BottomUpdateKind {
    /// Rebase the selected bottom-most commit onto the target branch.
    Rebase,
    /// Create a merge commit at the top of the selected stack.
    Merge,
}

/// Describes a particular bottom node and how it should be updated.
pub struct BottomUpdate {
    /// Describes how the associated branch should be updated.
    pub kind: BottomUpdateKind,
    /// A pointer to one of the bottom most commits in a stack.
    pub selector: RelativeTo,
}

enum UpdateTarget {
    Rebase {
        selector: RelativeTo,
    },
    Merge {
        /// The top of the branch which we'll either place the commit under if
        /// it's a reference, or on top of, if the branch has no top reference
        top: RelativeTo,
    },
}

/// The outcome of integrating upstream
pub struct IntegrateUpstreamOutcome<'ws, 'meta, M: RefMetadata> {
    /// The updated worskpace metadata.
    pub ws_meta: but_core::ref_metadata::Workspace,
    /// The rebased outcome.
    pub rebase: SuccessfulRebase<'ws, 'meta, M>,
}

#[derive(Clone)]
struct AnnotatedNode {
    to_rebase: bool,
    historically_integrated: bool,
    content_integrated: bool,
}

impl AnnotatedNode {
    fn new() -> Self {
        Self {
            to_rebase: false,
            historically_integrated: false,
            content_integrated: false,
        }
    }
}

/// Describes a sub-graph of commits from beneath workspace commit (or from HEAD
/// with a direct checkout) until the target commit or it's decendants.
#[derive(Clone)]
struct Stack {
    to_merge: bool,
    nodes: HashMap<Selector, AnnotatedNode>,
    heads: HashSet<Selector>,
    bottoms: HashSet<Selector>,
}

/// Is friggin good man!
pub fn integrate_upstream<'ws, 'meta, M: RefMetadata>(
    workspace: &'ws mut but_graph::projection::Workspace,
    meta: &'meta mut M,
    repo: &gix::Repository,
    updates: Vec<BottomUpdate>,
) -> Result<IntegrateUpstreamOutcome<'ws, 'meta, M>> {
    let mut ws_meta = workspace
        .metadata
        .clone()
        .context("Cannot update a workspace with no metadata")?;
    let target_sha = ws_meta
        .target_commit_id
        .context("Cannot update a workspace without a target sha")?;
    let target_ref = workspace
        .target_ref
        .clone()
        .context("Cannot update a workspace with no target ref")?;

    let entrypoint = workspace.graph.lookup_entrypoint()?;
    let head_commit = entrypoint
        .commit
        .context("Cannot update workspace without head commit")?;
    let head_commit = repo.find_commit(head_commit.id)?;
    let head_is_workspace_commit = is_managed_workspace_by_message(head_commit.message_raw()?);

    let editor_options = GraphEditorOptions {
        extra_refs: vec![target_ref.ref_name.as_ref()],
        ..GraphEditorOptions::default()
    };
    let mut editor = Editor::create_with_opts(workspace, meta, repo, &editor_options)?;
    let updates_with_selectors = updates
        .iter()
        .map(|update| Ok((update.selector.to_selector(&editor)?, update.kind)))
        .collect::<Result<Vec<_>, anyhow::Error>>()?;

    let target_ref_selector = target_ref.ref_name.to_selector(&editor)?;
    let target_sha_selector = target_sha.to_selector(&editor)?;

    let from_target_ref = traverse_nodes(&editor, target_ref_selector)?;
    let from_target_sha = traverse_nodes(&editor, target_sha_selector)?;

    let mut stacks = collect_stacks(
        head_commit,
        head_is_workspace_commit,
        &editor,
        from_target_ref,
    )?;

    // Validate described updates and find commits to rebase
    for stack in &mut stacks {
        let relevant_updates = updates_with_selectors
            .iter()
            .filter(|(s, _)| stack.bottoms.contains(s))
            .collect::<Vec<_>>();

        if relevant_updates
            .iter()
            .any(|(_, kind)| *kind == BottomUpdateKind::Merge)
        {
            if relevant_updates.len() > 1 {
                bail!("Found multiple updates for a stack using the merge stratergy");
            }
            if stack.heads.len() != 1 && stack.bottoms.len() != 1 {
                bail!(
                    "Merge stratergy must only be used on stacks with one head and one bottom commit"
                );
            }

            stack.to_merge = true
        } else {
            // currently the only other kind is rebase.
            let mut tips = relevant_updates.iter().map(|(s, _)| *s).collect::<Vec<_>>();
            let mut seen = tips.iter().cloned().collect::<HashSet<_>>();

            while let Some(tip) = tips.pop() {
                for c in editor
                    .direct_children(tip)?
                    .iter()
                    .filter_map(|(c, _)| stack.nodes.contains_key(c).then_some(*c))
                {
                    if seen.insert(c) {
                        tips.push(c);
                    }
                }
            }

            for seen in seen {
                if let Some(attrs) = stack.nodes.get_mut(&seen) {
                    attrs.to_rebase = true;
                }
            }
        }
    }

    for stack in &stacks {
        if stack.to_merge {
            let head = stack
                .heads
                .iter()
                .next()
                .context("BUG: Head should exist")?;
            let head_step = editor.lookup_step(*head)?;

            let insert_side = match head_step {
                Step::Pick(_) | Step::None => InsertSide::Above,
                Step::Reference { .. } => InsertSide::Below,
            };

            let mut merge_commit = editor.empty_commit()?;
            merge_commit.message = format!("Merge {} into merge", target_ref.ref_name).into();
            let merge_commit =
                editor.new_commit_untracked(merge_commit, DateMode::CommitterKeepAuthorKeep)?;
            let merge_commit = editor.insert(
                *head,
                Step::Pick(Pick::new_untracked_pick(merge_commit)),
                insert_side,
            )?;
            editor.add_edge(merge_commit, target_ref_selector, 1)?;
        } else {
            let mut edges_to_replace = vec![];

            // Currently, if I have a diamond (A<-B, A<-C, B<-D, C<-D), and `C`
            // was historically integrated, we end up with both `B` and `D` with
            // a graph (target<-B, target<-D, B<-D).
            //
            // The edge `target<-D` is superflous.
            //
            // We should be able to drop edges under the following condition:
            // "If a commit that has an edge we would consider re-parenting; if
            // it has a parent commit that also has an edge that we're going to
            // re-parent to pointing to target, we drop this commit's edge
            // instead"
            for (node, attrs) in stack.nodes.iter() {
                if attrs.historically_integrated {
                    continue;
                };
                if !attrs.to_rebase {
                    continue;
                };

                for (parent, _) in editor.direct_parents(*node)? {
                    let Some(p_attrs) = stack.nodes.get(&parent) else {
                        continue;
                    };

                    if p_attrs.historically_integrated {
                        edges_to_replace.push((*node, parent));
                    }
                }
            }
        }
    }

    todo!()
    // ws_meta.target_commit_id = Some(target_ref_commit);
    // Ok(IntegrateUpstreamOutcome {
    //     ws_meta,
    //     rebase: editor.rebase()?,
    // })
}

fn collect_stacks<'ws, 'meta, M: RefMetadata>(
    head_commit: gix::Commit<'_>,
    head_is_workspace_commit: bool,
    editor: &Editor<'ws, 'meta, M>,
    from_target_ref: HashSet<Selector>,
) -> Result<Vec<Stack>> {
    let mut stacks = if head_is_workspace_commit {
        editor
            .direct_children(head_commit.id)?
            .into_iter()
            .map(|(c, _)| Stack {
                to_merge: false,
                nodes: HashMap::from([(c, AnnotatedNode::new())]),
                heads: HashSet::from([c]),
                bottoms: HashSet::new(),
            })
            .collect()
    } else {
        let c = editor.select_commit(head_commit.id)?;
        vec![Stack {
            to_merge: false,
            nodes: HashMap::from([(c, AnnotatedNode::new())]),
            heads: HashSet::from([c]),
            bottoms: HashSet::new(),
        }]
    };
    for stack in &mut stacks {
        let mut tips = stack.nodes.keys().copied().collect::<Vec<_>>();

        while let Some(tip) = tips.pop() {
            for (parent, order) in editor.direct_parents(tip)? {
                if order != 0 && from_target_ref.contains(&parent) {
                    continue;
                }

                if stack.nodes.insert(parent, AnnotatedNode::new()).is_some() {
                    tips.push(parent);
                }
            }
        }
    }
    let mut output_stacks = vec![];
    while let Some(mut out) = stacks.pop() {
        for bix in (0..stacks.len()).rev() {
            #[expect(clippy::indexing_slicing)]
            if out.nodes.keys().any(|o| stacks[bix].nodes.contains_key(o)) {
                let b = stacks.swap_remove(bix);

                out.nodes.extend(b.nodes);
                out.heads.extend(b.heads);
            }
        }

        output_stacks.push(out);
    }

    for stack in &mut output_stacks {
        let Stack { nodes, bottoms, .. } = stack;

        for node in nodes.keys() {
            if editor
                .direct_parents(*node)?
                .iter()
                .all(|(p, _)| !nodes.contains_key(p))
            {
                bottoms.insert(*node);
            }
        }

        for (node, attrs) in nodes.iter_mut() {
            if from_target_ref.contains(node) {
                attrs.historically_integrated = true;
            }
        }
    }

    Ok(output_stacks)
}

/// Find all the parent nodes from and including the provided tip.
fn traverse_nodes<'ws, 'meta, M: RefMetadata>(
    editor: &Editor<'ws, 'meta, M>,
    tip: Selector,
) -> Result<HashSet<Selector>> {
    let mut seen = HashSet::from([tip]);
    let mut tips = vec![tip];

    while let Some(tip) = tips.pop() {
        for (parent, _) in editor.direct_parents(tip)? {
            if seen.insert(parent) {
                tips.push(parent);
            }
        }
    }

    Ok(seen)
}
