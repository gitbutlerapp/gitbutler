//! Integrating upstream changes

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};

use but_core::RefMetadata;
use but_graph::workspace::commit::is_managed_workspace_by_message;
use but_rebase::{
    commit::DateMode,
    graph_rebase::{
        Editor, ExtraRef, GraphEditorOptions, LookupStep, Pick, Selector, Step, SuccessfulRebase,
        ToSelector,
        mutate::{InsertSide, RelativeTo},
    },
};

use crate::changeset::compute_similarity_by_commit_ids;
use crate::graph_manipulation::traverse_nodes;

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

/// The outcome of integrating upstream
pub struct IntegrateUpstreamOutcome<'ws, 'meta, M: RefMetadata> {
    /// The updated workspace metadata.
    pub ws_meta: but_core::ref_metadata::Workspace,
    /// The rebased outcome.
    pub rebase: SuccessfulRebase<'ws, 'meta, M>,
}

#[derive(Clone, Debug)]
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
/// with a direct checkout) until the target commit or it's descendants.
#[derive(Clone, Debug)]
struct Stack {
    to_merge: bool,
    nodes: HashMap<Selector, AnnotatedNode>,
    heads: HashSet<Selector>,
    bottoms: HashSet<Selector>,
}

/// Integrate upstream changes into the workspace by either:
/// - Rebasing a stack onto `target` and dropping commits that are included
///   content-wise upstream.
/// - Merging upstream changes into a stack.
///
/// When workspace is checked out, a stacks are considered the subgraphs between
/// the ws commit and `target.sha`. Otherwise, a stack is considered all the
/// steps between the head commit and the `target.sha`.
///
/// A is a graph of commits. A stack may have multiple head commits (commits
/// with no children in the workspace), and multiple bottom commits (commits
/// with no parents in the workspace).
///
/// Updates are performed by specifying a particular update operation for a
/// particular bottom commit.
///
/// All bottom commits can be updated by marking them to be rebased. If a stack
/// has one head and one bottom, it is eligible to have upstream merged into it.
///
/// ## Notes on the algorithm:
///
/// The algorithm works as follows:
///
/// ### Collecting the stacks:
/// - Stacks are identified as the separate sub-graphs between `workspace head`
///   and `target.sha`.
/// - Each node in a stack that is included in `target.ref` gets marked as
///   `historically_integrated`.
/// - Each node in a stack commit node that is determined to be
///   upstream-integrated gets marked as `content_integrated`.
/// - Any `Reference` or `None` node whose parents are all `content_integrated`
///   get marked as `contented_integrated`.
///
/// ### Resolving the updates
/// - We validate updates match a bottom in a stack, and that Merge updates are
///   only marked on stacks with one head and one bottom.
/// - For `Rebase` updates, we propagate a `to_rebase` flag to all the children
///   nodes of that bottom.
///
/// ### Performing merges
/// - We create a merge commit either the top `Pick` or `None` step, or beneath
///   the top `Reference` step.
///
/// ### Performing rebases
/// - We identify edges between commits that are not `historically_integrated`
///   and those that are. These edges get replaced with edges to `target.ref`
/// - We replace all steps marked as `content_integrated` that are not
///   `historically_integrated` with `None` steps.
pub fn integrate_upstream<'ws, 'meta, M: RefMetadata>(
    workspace: &'ws mut but_graph::Workspace,
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
    let target_ref_commit = repo.find_reference(&target_ref.ref_name)?.id();

    let entrypoint = workspace.graph.entrypoint()?;
    let head_commit = entrypoint
        .commit()
        .context("Cannot update workspace without head commit")?;
    let head_commit = repo.find_commit(head_commit.id)?;
    let head_is_workspace_commit = is_managed_workspace_by_message(head_commit.message_raw()?);

    let editor_options = GraphEditorOptions {
        extra_refs: vec![ExtraRef::immutable(target_ref.ref_name.as_ref())],
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
    let mut from_target_sha = traverse_nodes(&editor, target_sha_selector)?;
    from_target_sha.extend(editor.step_references(target_sha_selector)?);

    let mut stacks = collect_stacks(
        head_commit,
        head_is_workspace_commit,
        &editor,
        from_target_sha,
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
                bail!("Found multiple updates for a stack using the merge strategy");
            }
            if stack.heads.len() != 1 || stack.bottoms.len() != 1 {
                bail!(
                    "Merge strategy must only be used on stacks with one head and one bottom commit"
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
            let mut edges_to_replace = HashSet::new();

            // Currently, if I have a diamond (A<-B, A<-C, B<-D, C<-D), and `C`
            // was historically integrated, we end up with both `B` and `D` with
            // a graph (target<-B, target<-D, B<-D).
            //
            // The edge `target<-D` is superfluous.
            //
            // We should be able to drop edges under the following condition:
            // "If a commit that has an edge we would consider re-parenting; if
            // it has a parent commit that also has an edge that we're going to
            // re-parent to pointing to target, we drop this commit's edge
            // instead"
            for (node, attrs) in stack.nodes.iter() {
                if !attrs.to_rebase {
                    continue;
                };
                if attrs.historically_integrated {
                    continue;
                };
                if attrs.content_integrated {
                    editor.replace(*node, Step::None)?;
                }

                for (parent, _) in editor.direct_parents(*node)? {
                    let Some(p_attrs) = stack.nodes.get(&parent) else {
                        edges_to_replace.insert((*node, parent));
                        continue;
                    };

                    if p_attrs.historically_integrated {
                        edges_to_replace.insert((*node, parent));
                    }
                }
            }

            for (child, parent) in edges_to_replace {
                let removed = editor.remove_edges(child, parent)?;
                // Add back the lowest ordered parent that was removed.
                // We could add back multiple, but it's likely unintentional
                // that there were two parents in the first place.
                if let Some(removed) = removed.iter().min() {
                    editor.add_edge(child, target_ref_selector, *removed)?;
                }
            }
        }
    }

    ws_meta.target_commit_id = Some(target_ref_commit.detach());
    Ok(IntegrateUpstreamOutcome {
        ws_meta,
        rebase: editor.rebase()?,
    })
}

fn collect_stacks<'ws, 'meta, M: RefMetadata>(
    head_commit: gix::Commit<'_>,
    head_is_workspace_commit: bool,
    editor: &Editor<'ws, 'meta, M>,
    from_target_sha: HashSet<Selector>,
    from_target_ref: HashSet<Selector>,
) -> Result<Vec<Stack>> {
    let mut stacks = if head_is_workspace_commit {
        editor
            .direct_parents(head_commit.id)?
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
            for (parent, _order) in editor.direct_parents(tip)? {
                if from_target_sha.contains(&parent) {
                    continue;
                }

                if stack.nodes.insert(parent, AnnotatedNode::new()).is_none() {
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

    let upstream_commits = commit_ids(
        editor,
        from_target_ref
            .iter()
            .filter_map(|s| (!from_target_sha.contains(s)).then_some(*s)),
    )?;
    let mut workspace_selectors = HashSet::new();
    for stack in &output_stacks {
        workspace_selectors.extend(stack.nodes.keys());
    }
    let integration = compute_similarity_by_commit_ids(
        editor.repo(),
        &upstream_commits,
        &commit_ids(editor, workspace_selectors)?,
        true,
    )?;

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

            let node = editor.lookup_step(*node)?;

            if let Step::Pick(Pick { id, .. }) = node
                && integration.matches_by_workspace_commit.contains_key(&id)
            {
                attrs.content_integrated = true;
            }
        }

        // Propagate content_integrated up to Reference or None steps who's
        // parents are _all_ content_integrated.
        //
        // We shouldn't need to do the same for historically_integrated
        // references because they should already be covered by the topography
        // on the Editor's graph.
        let mut tips = bottoms
            .iter()
            .filter_map(|b| stack.nodes.get(b)?.content_integrated.then_some(*b))
            .collect::<Vec<_>>();
        let mut seen = tips.iter().cloned().collect::<HashSet<_>>();

        while let Some(tip) = tips.pop() {
            for (child, _) in editor.direct_children(tip)? {
                if editor
                    .direct_parents(child)?
                    .into_iter()
                    .filter_map(|(p, _)| stack.nodes.get(&p))
                    .any(|p_attr| !p_attr.content_integrated)
                {
                    continue;
                }

                let c_step = editor.lookup_step(child)?;
                let Some(c_attrs) = stack.nodes.get_mut(&child) else {
                    continue;
                };
                if matches!(c_step, Step::Pick(_)) && !c_attrs.content_integrated {
                    continue;
                }
                if seen.insert(child) {
                    tips.push(child);

                    c_attrs.content_integrated = true;
                }
            }
        }
    }

    Ok(output_stacks)
}

/// Convert a list of selectors into their current commit ids.
///
/// Use the commit ids with great care as they might go out of date or have
/// expected parentages after mutations in the editor.
///
/// Prefer using the selectors if possible.
fn commit_ids<'ws, 'meta, M: RefMetadata>(
    editor: &Editor<'ws, 'meta, M>,
    selectors: impl IntoIterator<Item = Selector>,
) -> Result<Vec<gix::ObjectId>> {
    selectors
        .into_iter()
        .filter_map(|s| {
            editor
                .lookup_step(s)
                .map(|s| match s {
                    Step::Pick(Pick { id, .. }) => Some(id),
                    _ => None,
                })
                .transpose()
        })
        .collect()
}
