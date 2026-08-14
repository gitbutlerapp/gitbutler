//! Integrating upstream changes

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};
use bstr::ByteSlice;

use but_core::{RefMetadata, branch::unique_canned_refname, ref_metadata::ProjectMeta};
use but_graph::workspace::commit::is_managed_workspace_by_message;
use but_rebase::{
    commit::DateMode,
    graph_rebase::{
        Editor, LookupStep, Pick, Selector, Step, SuccessfulRebase, ToSelector,
        mutate::{InsertSide, RelativeTo, SegmentDelimiter, SelectorSet},
    },
};

use crate::graph_manipulation::traverse_nodes;
use crate::resolve_tracking_branch_ref_name;
use but_core::changeset::{compute_upstream_commits_lut, identify_matching_content, squash_in_lut};

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

/// A merged-review-derived integration anchor.
///
/// The commit points to the review head that was merged upstream. When that commit is still
/// present in a local stack, everything reachable beneath it is considered integrated. The source
/// branch associates the review with a reference-only empty stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewIntegrationHint {
    /// The merged review head commit that should act as an integration anchor.
    pub head_commit_at_merge: gix::ObjectId,
    /// The forge source branch associated with the merged review.
    pub source_branch: String,
}

/// The outcome of integrating upstream
pub struct IntegrateUpstreamOutcome<'ws, 'meta, M: RefMetadata> {
    /// The updated workspace metadata.
    pub ws_meta: Option<but_core::ref_metadata::Workspace>,
    /// The updated project metadata.
    pub project_meta: ProjectMeta,
    /// The rebased outcome.
    pub rebase: SuccessfulRebase<'ws, 'meta, M>,
}

#[derive(Clone, Debug)]
struct AnnotatedNode {
    to_rebase: bool,
    historically_integrated: bool,
    content_integrated: bool,
    review_integrated: bool,
    /// Only set to Some on references. Set to Some(<reference getting
    /// integrated>) if all the nodes exclusive to the current reference are
    /// marked as content or historically integrated or if the reference itself
    /// is historically integrated.
    ///
    /// Can be a remote reference, so care out to be exercised to ensure we
    /// don't try deleting remote references unexpectedly.
    reference_integrated: Option<gix::refs::FullName>,
}

impl AnnotatedNode {
    fn new() -> Self {
        Self {
            to_rebase: false,
            historically_integrated: false,
            content_integrated: false,
            review_integrated: false,
            reference_integrated: None,
        }
    }

    fn is_integrated(&self) -> bool {
        self.historically_integrated || self.content_integrated || self.review_integrated
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
    project_meta: ProjectMeta,
    repo: &gix::Repository,
    db: &'meta mut but_db::DbHandle,
    updates: Vec<BottomUpdate>,
) -> Result<IntegrateUpstreamOutcome<'ws, 'meta, M>> {
    integrate_upstream_with_hints(workspace, meta, project_meta, repo, db, updates, &[])
}

/// Like [`integrate_upstream()`], but accepts merged-review-derived integration
/// anchors to classify additional integrated history.
pub fn integrate_upstream_with_hints<'ws, 'meta, M: RefMetadata>(
    workspace: &'ws mut but_graph::Workspace,
    meta: &'meta mut M,
    project_meta: ProjectMeta,
    repo: &gix::Repository,
    db: &'meta mut but_db::DbHandle,
    updates: Vec<BottomUpdate>,
    review_hints: &[ReviewIntegrationHint],
) -> Result<IntegrateUpstreamOutcome<'ws, 'meta, M>> {
    if matches!(workspace.kind, but_graph::workspace::WorkspaceKind::AdHoc)
        && workspace.ref_name().is_none()
    {
        bail!("Operation not possible while HEAD is detached");
    }

    let mut ws_meta = workspace.metadata.clone();
    let target_sha = project_meta
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
    let head_commit_id = head_commit.id;
    let head_is_workspace_commit = is_managed_workspace_by_message(head_commit.message_raw()?);
    let direct_checkout_head_ref_name = if head_is_workspace_commit {
        None
    } else {
        repo.head_name()?
    };

    // The editor contains every segment in the graph; the target ref's segment
    // is reachable from HEAD and so is mutable by default.
    let mut editor = Editor::create(workspace, meta, repo, db)?;

    let updates_with_selectors = updates
        .iter()
        .map(|update| Ok((update.selector.to_selector(&editor)?, update.kind)))
        .collect::<Result<Vec<_>, anyhow::Error>>()?;

    let direct_checkout_head_ref_selector = direct_checkout_head_ref_name
        .as_ref()
        .map(|head_ref_name| head_ref_name.to_selector(&editor))
        .transpose()?;
    let mut direct_checkout_head_shares_tip_with_local_ref = false;
    if let Some(head_ref_selector) = direct_checkout_head_ref_selector {
        for selector in editor.step_references(head_commit_id)? {
            if selector != head_ref_selector
                && matches!(
                    editor.lookup_step(selector)?,
                    Step::Reference { refname, .. }
                        if refname.category() == Some(gix::refs::Category::LocalBranch)
                )
            {
                direct_checkout_head_shares_tip_with_local_ref = true;
                break;
            }
        }
    }

    // Select an empty checked-out branch by reference so same-tip local refs retain their distinct
    // identities. A direct reference update has the same requirement; ordinary non-empty direct
    // checkouts stay on the existing commit-based path.
    let direct_checkout_ref_selector = if direct_checkout_head_shares_tip_with_local_ref {
        direct_checkout_head_ref_selector
    } else {
        direct_checkout_head_ref_name
            .as_ref()
            .and_then(|head_ref_name| {
                updates
                    .iter()
                    .zip(&updates_with_selectors)
                    .find_map(|(update, (selector, _))| match &update.selector {
                        RelativeTo::Reference(ref_name)
                            if ref_name.as_ref() == head_ref_name.as_ref() =>
                        {
                            Some(*selector)
                        }
                        _ => None,
                    })
            })
    };

    let target_ref_selector = target_ref.ref_name.to_selector(&editor)?;
    let target_sha_selector = target_sha.to_selector(&editor)?;
    let target_ref_commit_selector = target_ref_commit.detach().to_selector(&editor)?;

    let from_target_ref = traverse_nodes(&editor, target_ref_selector)?;
    let mut from_target_sha = traverse_nodes(&editor, target_sha_selector)?;
    from_target_sha.extend(editor.step_references(target_sha_selector)?);

    let mut stacks = collect_stacks(
        head_commit,
        head_is_workspace_commit,
        direct_checkout_ref_selector,
        &editor,
        from_target_sha,
        from_target_ref,
        target_sha,
        target_ref.ref_name.as_ref(),
        target_ref_commit.detach(),
        review_hints,
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

    // Handle integrated stacks.
    // Determine which stacks (or branches) are integrated, and remove them from the workspace
    // if any.
    let workspace_commit_selector = head_is_workspace_commit
        .then(|| editor.select_commit(head_commit_id))
        .transpose()?;
    let mut fully_integrated_workspace_parents = HashSet::new();
    let mut direct_checkout_replacement_ref: Option<(Selector, gix::refs::FullName)> = None;
    let mut selected_stack_nodes = HashSet::new();
    for stack in &stacks {
        let is_selected = stack.nodes.values().any(|attrs| attrs.to_rebase)
            || stack.to_merge
            || updates_with_selectors
                .iter()
                .any(|(selector, _)| stack.bottoms.contains(selector));
        let is_fully_integrated = stack
            .nodes
            .values()
            .all(|attrs| attrs.is_integrated() || attrs.reference_integrated.is_some());
        if !is_selected {
            continue;
        }
        selected_stack_nodes.extend(stack.nodes.keys().copied());

        if is_fully_integrated {
            // If we're not in the managed workspace, we haven't determined a
            // ref replacement yet and we were checked out on a local branch.
            if !head_is_workspace_commit
                && direct_checkout_replacement_ref.is_none()
                && let Some(head_ref_name) = direct_checkout_head_ref_name.as_ref()
                && head_ref_name.as_ref().category() == Some(gix::refs::Category::LocalBranch)
            {
                direct_checkout_replacement_ref = Some(replace_direct_checkout_ref_with_fallback(
                    &mut editor,
                    repo,
                    head_ref_name.as_ref(),
                    target_ref_commit_selector,
                )?);
            }
            // TODO: Look into what happens when the head is an irrelevant
            // reference like the target_sha or a remote reference. In these
            // cases, we should look to see if it has a relevant reference
            // parent.
            for head in &stack.heads {
                let Step::Reference { refname, .. } = editor.lookup_step(*head)? else {
                    continue;
                };
                if refname.as_ref() == target_ref.ref_name.as_ref() {
                    continue;
                }
                fully_integrated_workspace_parents.insert(*head);
            }
        }

        // Remove integrated refs from the workspace and from git.
        // TODO: allow to keep some references.
        for (selector, attrs) in &stack.nodes {
            if let Some(ref_name) = attrs.reference_integrated.as_ref() {
                if let Some(ws_meta) = ws_meta.as_mut() {
                    ws_meta.remove_segment(ref_name.as_ref());
                }
                if should_delete_integrated_local_branch(ref_name.as_ref()) {
                    if direct_checkout_replacement_ref
                        .as_ref()
                        .is_some_and(|(replacement_selector, _)| replacement_selector == selector)
                    {
                        continue;
                    }
                    editor.replace(*selector, Step::None)?;
                }
            }
        }
    }

    // Disconnect all stack heads from the workspace commit, if any.
    if let Some(workspace_commit_selector) = workspace_commit_selector {
        for selector in &fully_integrated_workspace_parents {
            editor.remove_edges(workspace_commit_selector, *selector)?;
        }
        let direct_parents = editor.direct_parents(workspace_commit_selector)?;
        match direct_parents.as_slice() {
            [(parent_selector, parent_order)]
                if !selected_stack_nodes.contains(parent_selector)
                    && selector_commit_id(&editor, *parent_selector)? == Some(target_sha)
                    && target_sha != target_ref_commit.detach() =>
            {
                // Only parent is the old target sha, and that's not the latest tip of the target
                // ref. This is a workspace with no stacks, or an unnamed empty lane at the base
                // left behind after disconnecting fully integrated stacks — but never a selected
                // stack (a selected empty branch is rebased onto the target instead, and the
                // workspace commit must keep following it). We need to reparent it onto the
                // latest target ref; leaving it would materialize the stale target's tree over
                // the worktree.
                editor.remove_edges(workspace_commit_selector, *parent_selector)?;
                editor.add_edge(
                    workspace_commit_selector,
                    target_ref_selector,
                    *parent_order,
                )?;
            }
            [] if !fully_integrated_workspace_parents.is_empty() => {
                // Orphaned workspace, reparent onto the target ref.
                editor.add_edge(workspace_commit_selector, target_ref_selector, 0)?;
            }
            _ => {}
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
                editor.new_commit(merge_commit, DateMode::CommitterKeepAuthorKeep)?;
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
                if attrs.content_integrated || attrs.review_integrated {
                    editor.replace(*node, Step::None)?;
                }

                for (parent, _) in editor.direct_parents(*node)? {
                    let Some(p_attrs) = stack.nodes.get(&parent) else {
                        edges_to_replace.insert((*node, parent));
                        continue;
                    };

                    if p_attrs.historically_integrated
                        || p_attrs.content_integrated
                        || p_attrs.review_integrated
                    {
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
                    let target_selector = match editor.lookup_step(child)? {
                        Step::Reference { refname, .. }
                            if !head_is_workspace_commit
                                && direct_checkout_head_ref_name.as_ref().is_some_and(
                                    |head_ref| head_ref.as_ref() == refname.as_ref(),
                                ) =>
                        {
                            // A direct local ref cannot be parented through the target reference
                            // node when both refs already participate in the same reachable graph.
                            // Anchor it at the immutable target-tip pick instead.
                            editor.disconnect_segment_from(
                                SegmentDelimiter {
                                    child,
                                    parent: child,
                                },
                                SelectorSet::All,
                                SelectorSet::All,
                                false,
                            )?;
                            preserve_pick_parents(&mut editor, target_ref_commit_selector)?;
                            target_ref_commit_selector
                        }
                        _ => target_ref_selector,
                    };
                    editor.add_edge(child, target_selector, *removed)?;
                }
            }
        }
    }

    let mut project_meta = project_meta;
    project_meta.target_commit_id = Some(target_ref_commit.detach());
    Ok(IntegrateUpstreamOutcome {
        ws_meta,
        project_meta,
        rebase: editor.rebase()?,
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_stacks<'ws, 'meta, M: RefMetadata>(
    head_commit: gix::Commit<'_>,
    head_is_workspace_commit: bool,
    direct_checkout_ref_selector: Option<Selector>,
    editor: &Editor<'ws, 'meta, M>,
    from_target_sha: HashSet<Selector>,
    from_target_ref: HashSet<Selector>,
    target_sha: gix::ObjectId,
    target_ref_name: &gix::refs::FullNameRef,
    target_ref_commit: gix::ObjectId,
    review_hints: &[ReviewIntegrationHint],
) -> Result<Vec<Stack>> {
    let direct_checkout_head_commit_id = head_commit.id;
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
        let c = match direct_checkout_ref_selector {
            Some(selector) => selector,
            None => editor.select_commit(head_commit.id)?,
        };
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

    let upstream_selectors = from_target_ref
        .iter()
        .filter_map(|s| (!from_target_sha.contains(s)).then_some(*s))
        .collect::<Vec<_>>();
    let upstream_selectors = if upstream_selectors.is_empty() {
        from_target_ref.iter().copied().collect()
    } else {
        upstream_selectors
    };
    let upstream_commits = commit_ids(editor, upstream_selectors)?;
    let mut workspace_selectors = HashSet::new();
    for stack in &output_stacks {
        workspace_selectors.extend(stack.nodes.keys());
    }
    let upstream_lut = compute_upstream_commits_lut(editor.repo(), &upstream_commits)?;
    let matches_by_workspace_commit = identify_matching_content(
        editor.repo(),
        &upstream_lut,
        &commit_ids(editor, workspace_selectors)?,
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
            let step = editor.lookup_step(*node)?;
            let is_local_reference = matches!(
                &step,
                Step::Reference { refname, .. }
                    if refname.category() == Some(gix::refs::Category::LocalBranch)
            );
            if from_target_ref.contains(node)
                && !(!head_is_workspace_commit && is_local_reference && stack.heads.contains(node))
            {
                attrs.historically_integrated = true;
            }

            if let Step::Pick(Pick { id, .. }) = step
                && matches_by_workspace_commit.contains_key(&id)
            {
                attrs.content_integrated = true;
            }
        }

        apply_review_integration_hints(editor, stack, review_hints)?;

        let reference_nodes = stack
            .nodes
            .keys()
            .filter_map(|n| {
                editor
                    .lookup_step(*n)
                    .map(|step| match step {
                        Step::Reference { refname, .. } => Some((*n, refname)),
                        _ => None,
                    })
                    .transpose()
            })
            .collect::<Result<HashMap<_, _>>>()?;

        // Identify whether all the commits that are exclusively referenced by a
        // given reference in the stack are all integrated upstream.
        //
        // If all the commits are integrated, or if the reference itself is
        // considered historically integrated, we set the `reference_integrated`
        // flag which flags the reference for deletion, if it's a selected
        // target to be updated.
        for (r_sel, r_name) in reference_nodes.iter() {
            let mut tips = vec![*r_sel];
            let mut seen = tips.iter().cloned().collect::<HashSet<_>>();
            let mut members = Vec::new();
            let mut passed_through_references = Vec::new();
            let mut linear = true;
            let mut base = None;

            while let Some(tip) = tips.pop() {
                let parents = editor.direct_parents(tip)?;
                if parents.len() != 1 {
                    linear = false;
                }
                for (parent, _) in parents {
                    if !stack.nodes.contains_key(&parent)
                        || reference_nodes.get(&parent).is_some_and(|parent_ref| {
                            parent_ref.category() == Some(gix::refs::Category::LocalBranch)
                        })
                    {
                        base = Some(parent);
                        continue;
                    }
                    if seen.insert(parent) {
                        if reference_nodes.contains_key(&parent) {
                            passed_through_references.push(parent);
                        } else {
                            members.push(parent);
                        }
                        tips.push(parent);
                    }
                }
            }

            let traversed_commits = !members.is_empty();
            let mut all_integrated = members.iter().all(|m| {
                stack
                    .nodes
                    .get(m)
                    .is_some_and(|attrs| attrs.is_integrated())
            });
            let is_local_reference = r_name.category() == Some(gix::refs::Category::LocalBranch);

            if is_local_reference
                && members.len() > 1
                && !all_integrated
                && linear
                && let Some(base) = base
                && let (Some(base_id), Some(top_id)) = (
                    selector_commit_id(editor, base)?,
                    selector_commit_id(editor, *r_sel)?,
                )
                && squash_in_lut(editor.repo(), &upstream_lut, base_id, top_id)?.is_some()
            {
                all_integrated = true;
                for member in &members {
                    if let Some(attrs) = stack.nodes.get_mut(member) {
                        attrs.content_integrated = true;
                    }
                }
            }

            if is_local_reference && traversed_commits && all_integrated {
                for passed_sel in &passed_through_references {
                    if let (Some(node), Some(passed_name)) = (
                        stack.nodes.get_mut(passed_sel),
                        reference_nodes.get(passed_sel),
                    ) {
                        node.reference_integrated = Some(passed_name.clone());
                    }
                }
            }

            let Some(node) = stack.nodes.get_mut(r_sel) else {
                continue;
            };
            let remote_tip_integrated = empty_local_reference_remote_tip_integrated(
                editor,
                *r_sel,
                r_name.as_ref(),
                &reference_nodes,
                &from_target_ref,
                target_sha,
                target_ref_name,
                target_ref_commit,
            )?;
            if traversed_commits {
                // The remote-tip check is only an empty-segment fallback. Once
                // we have traversed local commits, those commits decide whether
                // the segment is integrated so local work ahead of its tracking
                // branch is not discarded.
                if all_integrated {
                    node.reference_integrated = Some(r_name.clone());
                }
            } else if !head_is_workspace_commit && stack.heads.contains(r_sel) && is_local_reference
            {
                // A local ref pointing to target-reachable history is not by itself evidence that
                // the branch was integrated. For an empty local segment, require branch-specific
                // evidence from its configured remote-tracking ref or its associated merged
                // review.
                let review_integrated =
                    configured_tracking_branch_short_name(editor.repo(), r_name.as_ref())
                        .is_some_and(|pushed_branch| {
                            review_hints_match_pushed_branch(
                                review_hints,
                                &pushed_branch,
                                direct_checkout_head_commit_id,
                            )
                        });
                if remote_tip_integrated || review_integrated {
                    node.reference_integrated = Some(r_name.clone());
                }
            } else if node.is_integrated() || remote_tip_integrated {
                node.reference_integrated = Some(r_name.clone());
            }
        }
    }

    Ok(output_stacks)
}

/// Return the configured remote/pushed branch name without requiring its tracking ref to exist.
/// Forge hosts commonly delete merged source branches, while the local branch configuration stays.
fn configured_tracking_branch_short_name(
    repo: &gix::Repository,
    ref_name: &gix::refs::FullNameRef,
) -> Option<String> {
    let remote_ref_name = repo
        .branch_remote_tracking_ref_name(ref_name, gix::remote::Direction::Fetch)
        .transpose()
        .ok()
        .flatten()?;
    let (_, short_name) = but_core::extract_remote_name_and_short_name(
        remote_ref_name.as_ref(),
        &repo.remote_names(),
    )?;
    short_name.to_str().ok().map(ToOwned::to_owned)
}

/// Match the forge's source branch and review head against the checked-out branch. A uniquely
/// matching `owner:branch` fork head is accepted, while ambiguous fork heads remain conservative.
fn review_hints_match_pushed_branch(
    review_hints: &[ReviewIntegrationHint],
    pushed_branch: &str,
    branch_tip: gix::ObjectId,
) -> bool {
    if review_hints
        .iter()
        .any(|hint| hint.source_branch == pushed_branch && hint.head_commit_at_merge == branch_tip)
    {
        return true;
    }

    let mut fork_matches = review_hints.iter().filter(|hint| {
        hint.head_commit_at_merge == branch_tip
            && hint
                .source_branch
                .rsplit_once(':')
                .is_some_and(|(_, branch)| branch == pushed_branch)
    });
    fork_matches.next().is_some() && fork_matches.next().is_none()
}

/// Return `true` if an empty local branch can be treated as integrated because
/// its tracking branch tip is already contained in the target branch.
///
/// Empty branch segments have no local commits to compare content-wise, so this
/// looks at the branch's configured remote-tracking ref instead. The check is
/// deliberately conservative: it only applies to local branches, ignores the
/// target branch itself, rejects stale tracking refs that still point at the old
/// target, and preserves an empty branch if it sits on top of another local
/// branch that is not itself target-integrated.
#[allow(clippy::too_many_arguments)]
fn empty_local_reference_remote_tip_integrated<'ws, 'meta, M: RefMetadata>(
    editor: &Editor<'ws, 'meta, M>,
    selector: Selector,
    ref_name: &gix::refs::FullNameRef,
    reference_nodes: &HashMap<Selector, gix::refs::FullName>,
    from_target_ref: &HashSet<Selector>,
    target_sha: gix::ObjectId,
    target_ref_name: &gix::refs::FullNameRef,
    target_ref_commit: gix::ObjectId,
) -> Result<bool> {
    if ref_name.category() != Some(gix::refs::Category::LocalBranch) {
        return Ok(false);
    }
    if editor.direct_parents(selector)?.iter().any(|(parent, _)| {
        reference_nodes.get(parent).is_some_and(|parent_ref| {
            parent_ref.category() == Some(gix::refs::Category::LocalBranch)
                && !from_target_ref.contains(parent)
                && !reference_points_to_target(
                    editor.repo(),
                    parent_ref.as_ref(),
                    target_sha,
                    target_ref_commit,
                )
        })
    }) {
        return Ok(false);
    }

    let Ok(remote_ref_name) = resolve_tracking_branch_ref_name(ref_name, editor.repo()) else {
        return Ok(false);
    };
    if remote_ref_name.as_bstr() == target_ref_name.as_bstr() {
        return Ok(false);
    }

    let Some(remote_tip_id) = editor
        .repo()
        .try_find_reference(remote_ref_name.as_ref())?
        .and_then(|mut reference| reference.peel_to_id().ok())
        .map(|id| id.detach())
    else {
        return Ok(false);
    };
    if remote_tip_id == target_sha && remote_tip_id != target_ref_commit {
        return Ok(false);
    }

    Ok(editor
        .repo()
        .merge_base(remote_tip_id, target_ref_commit)
        .is_ok_and(|merge_base| merge_base.detach() == remote_tip_id))
}

/// Return `true` if `ref_name` currently resolves to either the old target
/// commit or the advanced target tip.
///
/// This is used when checking the parent refs of an empty branch. Parent refs
/// that point at either target boundary are safe to treat as target-frame
/// anchors, while other local parent refs keep the empty branch alive.
fn reference_points_to_target(
    repo: &gix::Repository,
    ref_name: &gix::refs::FullNameRef,
    target_sha: gix::ObjectId,
    target_ref_commit: gix::ObjectId,
) -> bool {
    repo.try_find_reference(ref_name)
        .ok()
        .flatten()
        .and_then(|mut reference| reference.peel_to_id().ok())
        .map(|id| {
            let id = id.detach();
            id == target_sha || id == target_ref_commit
        })
        .unwrap_or(false)
}

/// Whether an integrated local branch ref should be deleted during integration.
///
/// Only local branches are eligible for deletion here. As a conservative
/// heuristic, never delete local `main` or `master`, even if the graph marks
/// them as integrated. Those names often represent a user's primary local
/// branch, so keeping them is safer than treating them like disposable topic
/// branches.
fn should_delete_integrated_local_branch(ref_name: &gix::refs::FullNameRef) -> bool {
    let Some((gix::refs::Category::LocalBranch, short_name)) = ref_name.category_and_short_name()
    else {
        return false;
    };

    short_name != "main" && short_name != "master"
}

/// Mark stack commits as integrated when they are covered by merged-review hints.
///
/// Hints are review heads recorded at the time a forge review was merged. If a
/// hinted review head is still present in the local stack, the review confirms
/// that commit and all stack-local ancestors below it have landed upstream.
/// Commits above the hinted head are intentionally left local, which lets a
/// branch keep extra post-merge commits while dropping the already-merged prefix.
fn apply_review_integration_hints<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    stack: &mut Stack,
    review_hints: &[ReviewIntegrationHint],
) -> Result<()> {
    let mut selectors_by_commit_id = HashMap::<gix::ObjectId, Selector>::new();
    for selector in stack.nodes.keys().copied() {
        if let Step::Pick(Pick { id, .. }) = editor.lookup_step(selector)? {
            selectors_by_commit_id.insert(id, selector);
        }
    }

    let matching_heads = review_hints
        .iter()
        .filter_map(|hint| {
            selectors_by_commit_id
                .get(&hint.head_commit_at_merge)
                .copied()
        })
        .collect::<Vec<_>>();
    if matching_heads.is_empty() {
        return Ok(());
    }

    let highest_heads = highest_review_heads(editor, stack, &matching_heads)?;
    for head in highest_heads {
        let mut tips = vec![head];
        let mut seen = HashSet::new();

        while let Some(tip) = tips.pop() {
            if !seen.insert(tip) {
                continue;
            }
            if let Some(attrs) = stack.nodes.get_mut(&tip) {
                attrs.review_integrated = true;
            }
            for (parent, _) in editor.direct_parents(tip)? {
                if stack.nodes.contains_key(&parent) {
                    tips.push(parent);
                }
            }
        }
    }

    Ok(())
}

/// Return only the highest matching review heads within the stack graph.
///
/// Multiple review hints can match commits in the same ancestry chain. Marking
/// from the highest matched head is enough because the traversal from that head
/// already covers lower matched ancestors. Keeping only the highest heads avoids
/// repeated ancestor walks while preserving independent matched branches in a
/// multi-head stack.
fn highest_review_heads<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    stack: &Stack,
    matching_heads: &[Selector],
) -> Result<Vec<Selector>> {
    let candidates = matching_heads.iter().copied().collect::<HashSet<_>>();
    let mut discard = HashSet::new();

    for head in matching_heads {
        let mut tips = vec![*head];
        let mut seen = HashSet::from([*head]);

        while let Some(tip) = tips.pop() {
            for (parent, _) in editor.direct_parents(tip)? {
                if !stack.nodes.contains_key(&parent) {
                    continue;
                }
                if candidates.contains(&parent) {
                    discard.insert(parent);
                }
                if seen.insert(parent) {
                    tips.push(parent);
                }
            }
        }
    }

    let mut out = Vec::new();
    let mut added = HashSet::new();
    for head in matching_heads {
        if !discard.contains(head) && added.insert(*head) {
            out.push(*head);
        }
    }
    Ok(out)
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

fn selector_commit_id<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    selector: Selector,
) -> Result<Option<gix::ObjectId>> {
    Ok(match editor.lookup_step(selector)? {
        Step::Pick(Pick { id, .. }) => Some(id),
        Step::Reference { refname, .. } => Some(
            editor
                .repo()
                .find_reference(refname.as_ref())?
                .id()
                .detach(),
        ),
        Step::None => None,
    })
}

/// Replace a fully integrated direct-checkout branch with a new canned local branch at the
/// latest target tip.
///
/// In a managed workspace, a fully integrated stack can simply be detached from the workspace
/// commit and the workspace commit is reparented to the target. A direct checkout has no
/// workspace commit to keep `HEAD` alive, so deleting the checked-out branch would leave `HEAD`
/// pointing at a missing ref. Instead, reuse the checkout reference step for a fresh branch name
/// and point it at the latest target commit.
///
/// The old checkout reference can be on the target ancestry path. Before repointing the step to
/// the target tip, `disconnect_segment_from()` rewires its children around the old reference to
/// preserve the existing graph and avoid introducing a cycle.
fn replace_direct_checkout_ref_with_fallback<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    repo: &gix::Repository,
    head_ref_name: &gix::refs::FullNameRef,
    target_tip_selector: Selector,
) -> Result<(Selector, gix::refs::FullName)> {
    let head_ref_selector = head_ref_name.to_selector(editor)?;
    let fallback_ref_name = unique_canned_refname(repo)?;

    editor.replace(
        head_ref_selector,
        Step::new_reference(fallback_ref_name.clone()),
    )?;

    editor.disconnect_segment_from(
        SegmentDelimiter {
            child: head_ref_selector,
            parent: head_ref_selector,
        },
        SelectorSet::All,
        SelectorSet::All,
        false,
    )?;
    preserve_pick_parents(editor, target_tip_selector)?;
    editor.add_edge(head_ref_selector, target_tip_selector, 0)?;

    Ok((head_ref_selector, fallback_ref_name))
}

fn preserve_pick_parents<M: RefMetadata>(
    editor: &mut Editor<'_, '_, M>,
    selector: Selector,
) -> Result<()> {
    let Step::Pick(mut pick) = editor.lookup_step(selector)? else {
        bail!("Expected target tip selector to point to a pick");
    };
    let commit = editor.find_commit(pick.id)?;
    // TODO: Teach but-rebase to treat immutable reference parents as object
    // anchors. Until then, preserve the target tip's original parents here so
    // graph-rebase materializes the fallback branch at the exact target ref
    // object instead of replaying merge-based target history into an equivalent
    // local rewrite.
    pick.preserved_parents = Some(commit.inner.parents.iter().copied().collect());
    editor.replace(selector, Step::Pick(pick))?;
    Ok(())
}
