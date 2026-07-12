//! Integrating upstream changes

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};

use but_core::{RefMetadata, branch::unique_canned_refname, ref_metadata::ProjectMeta};
use but_graph::workspace::commit::is_managed_workspace_by_message;
use but_rebase::graph_rebase::{
    CommitSpec, Editor, EditorIndex, RebasedEditor,
    anchor::{Anchor, Cut, Range},
    mutate::{InsertSide, Reconnect},
};

use crate::resolve_tracking_branch_ref_name;
use bstr::ByteSlice;
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
    pub selector: Anchor,
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
pub struct IntegrateUpstreamOutcome<'meta, M: RefMetadata> {
    /// The updated workspace metadata.
    pub ws_meta: Option<but_core::ref_metadata::Workspace>,
    /// The updated project metadata.
    pub project_meta: ProjectMeta,
    /// The rebased outcome.
    pub rebase: RebasedEditor<'meta, M>,
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
    nodes: HashMap<EditorIndex, AnnotatedNode>,
    heads: HashSet<EditorIndex>,
    bottoms: HashSet<EditorIndex>,
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
/// - We create a merge commit either the top `CommitSpec` or `None` step, or beneath
///   the top `Reference` step.
///
/// ### Performing rebases
/// - We identify edges between commits that are not `historically_integrated`
///   and those that are. These edges get replaced with edges to `target.ref`
/// - We replace all steps marked as `content_integrated` that are not
///   `historically_integrated` with `None` steps.
///
/// This variant uses no review hints and never swaps an emptied managed workspace for a canned
/// branch; see [`integrate_upstream_with_hints()`] for both.
pub fn integrate_upstream<'meta, M: RefMetadata>(
    workspace: &but_graph::Workspace,
    meta: &'meta mut M,
    project_meta: ProjectMeta,
    repo: &gix::Repository,
    updates: Vec<BottomUpdate>,
) -> Result<IntegrateUpstreamOutcome<'meta, M>> {
    integrate_upstream_with_hints(workspace, meta, project_meta, repo, updates, &[], false)
}

/// Like [`integrate_upstream()`], but accepts merged-review-derived integration
/// anchors to classify additional integrated history.
///
/// With `single_branch_mode`, a managed workspace whose applied stacks were all integrated is
/// replaced by a checked-out canned branch at the target tip. Otherwise the emptied managed
/// workspace stays checked out, reparented onto the target.
pub fn integrate_upstream_with_hints<'meta, M: RefMetadata>(
    workspace: &but_graph::Workspace,
    meta: &'meta mut M,
    project_meta: ProjectMeta,
    repo: &gix::Repository,
    updates: Vec<BottomUpdate>,
    review_hints: &[ReviewIntegrationHint],
    single_branch_mode: bool,
) -> Result<IntegrateUpstreamOutcome<'meta, M>> {
    if matches!(workspace.kind(), but_graph::workspace::WorkspaceKind::AdHoc)
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

    let head_commit = workspace
        .entrypoint_commit_id()?
        .context("Cannot update workspace without head commit")?;
    let head_commit = repo.find_commit(head_commit)?;
    let head_commit_id = head_commit.id;
    let head_is_workspace_commit = is_managed_workspace_by_message(head_commit.message_raw()?);
    let workspace_ref_name = workspace.ref_name().map(ToOwned::to_owned);
    let direct_checkout_head_ref_name = if head_is_workspace_commit {
        None
    } else {
        repo.head_name()?
    };

    // The editor contains every commit in the graph; the target ref's step
    // is reachable from HEAD and so is mutable by default.
    let mut editor = Editor::for_workspace(workspace, meta, repo)?;

    let updates_with_entries = updates
        .iter()
        .map(|update| Ok((editor.resolve_anchor(update.selector.clone())?, update.kind)))
        .collect::<Result<Vec<_>, anyhow::Error>>()?;

    let direct_checkout_head_ref_entry: Option<EditorIndex> = direct_checkout_head_ref_name
        .as_ref()
        .map(|head_ref_name| {
            editor
                .select_reference(head_ref_name.as_ref())
                .map(Into::into)
        })
        .transpose()?;
    let mut direct_checkout_head_shares_tip_with_local_ref = false;
    if let Some(head_ref_entry) = direct_checkout_head_ref_entry {
        for entry in editor.references_of(head_commit_id)? {
            if entry != head_ref_entry
                && let EditorIndex::Ref(reference) = entry
                && !editor.is_removed(reference)
                && editor.name_of(reference)?.category() == Some(gix::refs::Category::LocalBranch)
            {
                direct_checkout_head_shares_tip_with_local_ref = true;
                break;
            }
        }
    }

    // Select an empty checked-out branch by reference so same-tip local refs retain their distinct
    // identities. A direct reference update has the same requirement; ordinary non-empty direct
    // checkouts stay on the existing commit-based path.
    let direct_checkout_ref_entry = if direct_checkout_head_shares_tip_with_local_ref {
        direct_checkout_head_ref_entry
    } else {
        direct_checkout_head_ref_name
            .as_ref()
            .and_then(|head_ref_name| {
                updates
                    .iter()
                    .zip(&updates_with_entries)
                    .find_map(|(update, (entry, _))| match &update.selector {
                        Anchor::Reference(ref_name) if ref_name == head_ref_name => Some(*entry),
                        _ => None,
                    })
            })
    };

    let target_ref_entry = editor.resolve_anchor(&target_ref.ref_name)?;
    let target_sha_entry = editor.resolve_anchor(target_sha)?;
    let target_ref_commit_entry = editor.resolve_anchor(target_ref_commit.detach())?;

    let from_target_ref = editor.position_reachable(target_ref_entry)?;
    let mut from_target_sha = editor.position_reachable(target_sha_entry)?;
    from_target_sha.extend(editor.references_of(target_sha_entry)?);

    let mut stacks = collect_stacks(
        head_commit,
        head_is_workspace_commit,
        direct_checkout_ref_entry,
        &editor,
        TargetReach {
            from_sha: from_target_sha,
            from_ref: from_target_ref,
        },
        TargetRef {
            sha: target_sha,
            ref_name: target_ref.ref_name.as_ref(),
            ref_commit: target_ref_commit.detach(),
        },
        review_hints,
    )?;

    // Validate described updates and find commits to rebase
    for stack in &mut stacks {
        let relevant_updates = updates_with_entries
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
                    .position_children(tip)?
                    .iter()
                    .filter(|c| stack.nodes.contains_key(*c))
                {
                    if seen.insert(*c) {
                        tips.push(*c);
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
    let workspace_commit_entry = head_is_workspace_commit
        .then(|| editor.select_commit(head_commit_id))
        .transpose()?;
    let mut fully_integrated_workspace_parents = HashSet::new();
    let mut direct_checkout_replacement_ref: Option<(EditorIndex, gix::refs::FullName)> = None;
    let mut selected_stack_entries = HashSet::new();
    for stack in &stacks {
        let is_selected = stack.nodes.values().any(|attrs| attrs.to_rebase)
            || stack.to_merge
            || updates_with_entries
                .iter()
                .any(|(entry, _)| stack.bottoms.contains(entry));
        let is_fully_integrated = stack
            .nodes
            .values()
            .all(|attrs| attrs.is_integrated() || attrs.reference_integrated.is_some());
        if !is_selected {
            continue;
        }
        selected_stack_entries.extend(stack.nodes.keys().copied());

        if is_fully_integrated {
            // If we're not in the managed workspace, we haven't determined a
            // ref replacement yet and we were checked out on a local branch.
            if !head_is_workspace_commit
                && direct_checkout_replacement_ref.is_none()
                && let Some(head_ref_name) = direct_checkout_head_ref_name.as_ref()
                && head_ref_name.as_ref().category() == Some(gix::refs::Category::LocalBranch)
            {
                direct_checkout_replacement_ref = Some(replace_checkout_ref_with_fallback(
                    &mut editor,
                    repo,
                    head_ref_name.as_ref(),
                    target_ref_commit_entry,
                )?);
            }
            // TODO: Look into what happens when the head is an irrelevant
            // reference like the target_sha or a remote reference. In these
            // cases, we should look to see if it has a relevant reference
            // parent.
            for head in &stack.heads {
                let EditorIndex::Ref(reference) = *head else {
                    continue;
                };
                if editor.is_removed(reference) {
                    continue;
                }
                let refname = editor.name_of(reference)?;
                if refname.as_ref() == target_ref.ref_name.as_ref() {
                    continue;
                }
                fully_integrated_workspace_parents.insert(*head);
            }
        }

        // Remove integrated refs from the workspace and from git.
        // TODO: allow to keep some references.
        for (entry, attrs) in &stack.nodes {
            if let Some(ref_name) = attrs.reference_integrated.as_ref() {
                if let Some(ws_meta) = ws_meta.as_mut() {
                    ws_meta.remove_segment(ref_name.as_ref());
                }
                if should_delete_integrated_local_branch(ref_name.as_ref()) {
                    if direct_checkout_replacement_ref
                        .as_ref()
                        .is_some_and(|(replacement_entry, _)| replacement_entry == entry)
                    {
                        continue;
                    }
                    remove_entry(&mut editor, *entry)?;
                }
            }
        }
    }

    // A DECLARED branch whose REFERENCE has ended up inside the target's own history has landed:
    // the merge holds nothing of its own for it anymore. Such a reference never joins a stack —
    // collection stops at the target's territory, which is exactly where it now sits — so the
    // per-stack cleanup above can never reach it, and `but pull` kept telling the user to run
    // `but pull`. The target's own local branch is excluded: it is not a feature branch that
    // landed, it is the thing others land into.
    // Recomputed here, not reused from above: the rewiring since then changed what the target
    // reaches.
    let target_territory = editor.position_reachable(target_ref_entry)?;
    // Only references the CALLER NAMED. A stack is pruned when it was selected, never merely
    // because it is integrated (`non_bottom_update_selector_does_not_prune_fully_integrated_stack`)
    // — this sweep is the same rule for a branch that never reached a stack, so it needs the same
    // consent. Matching by name: the declaration lists its branches by name, not by position.
    let named_by_caller: HashSet<&gix::refs::FullNameRef> = updates
        .iter()
        .filter_map(|u| match &u.selector {
            Anchor::Reference(name) => Some(name.as_ref()),
            // Commit- and entry-addressed updates carry no branch name to match on.
            Anchor::Commit(_) | Anchor::Held(_) => None,
        })
        .collect();
    let landed = ws_meta
        .as_ref()
        .filter(|_| !named_by_caller.is_empty())
        .map(|m| {
            m.stacks
                .iter()
                .filter(|s| s.is_in_workspace())
                .flat_map(|s| &s.branches)
                .map(|b| b.ref_name.clone())
                .filter(|rn| rn.as_ref() != target_ref.ref_name.as_ref())
                .filter(|rn| !is_target_local_branch(rn.as_ref(), target_ref.ref_name.as_ref()))
                .filter(|rn| named_by_caller.contains(rn.as_ref()))
                .filter_map(|rn| {
                    let sel = editor.resolve_anchor(rn.clone()).ok()?;
                    target_territory.contains(&sel).then_some((rn, sel))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for (ref_name, entry) in landed {
        if let Some(ws_meta) = ws_meta.as_mut() {
            ws_meta.remove_segment(ref_name.as_ref());
        }
        if should_delete_integrated_local_branch(ref_name.as_ref()) {
            remove_entry(&mut editor, entry)?;
        }
    }

    // Disconnect all stack heads from the workspace commit, if any.
    if let Some(workspace_commit_entry) = workspace_commit_entry {
        for entry in &fully_integrated_workspace_parents {
            editor.detach(workspace_commit_entry, *entry)?;
        }
        let direct_parents = editor.direct_parents(workspace_commit_entry)?;
        match direct_parents.as_slice() {
            [(parent_entry, _)]
                if !selected_stack_entries.contains(parent_entry)
                    && entry_commit_id(&editor, *parent_entry)? == Some(target_sha)
                    && target_sha != target_ref_commit.detach() =>
            {
                // Only parent is the old target sha, and that's not the latest tip of the target
                // ref. This is a workspace with no stacks, or an unnamed empty lane at the base
                // left behind after disconnecting fully integrated stacks — but never a selected
                // stack (a selected empty branch is rebased onto the target instead, and the
                // workspace commit must keep following it). We need to reparent it onto the
                // latest target ref; leaving it would materialize the stale target's tree over
                // the worktree.
                editor.reparent(workspace_commit_entry, *parent_entry, target_ref_entry)?;
            }
            [] if !fully_integrated_workspace_parents.is_empty()
                && stacks.len() > 1
                && single_branch_mode =>
            {
                // In single-branch mode a managed workspace must not become empty. Replace its
                // checkout with a uniquely named canned branch at the latest target tip, just
                // like a fully integrated direct checkout.
                let workspace_ref_name = workspace_ref_name
                    .as_ref()
                    .map(|name| name.as_ref())
                    .context("Managed workspace has no reference")?;
                replace_checkout_ref_with_fallback(
                    &mut editor,
                    repo,
                    workspace_ref_name,
                    target_ref_commit_entry,
                )?;
            }
            [] if !fully_integrated_workspace_parents.is_empty() => {
                // Otherwise the existing empty managed workspace is retained.
                editor.insert_parent(workspace_commit_entry, target_ref_entry, 0)?;
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
            let insert_side = match *head {
                EditorIndex::Ref(_) if !editor.is_removed(*head) => InsertSide::Below,
                _ => InsertSide::Above,
            };

            let merge_commit =
                editor.new_merge_commit(format!("Merge {} into merge", target_ref.ref_name))?;
            let merge_commit =
                editor.insert_commit(*head, CommitSpec::untracked(merge_commit), insert_side)?;
            editor.insert_parent(merge_commit, target_ref_entry, 1)?;
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
                    remove_entry(&mut editor, *node)?;
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
                let child_is_direct_checkout_ref = match child {
                    EditorIndex::Ref(reference) if !head_is_workspace_commit => editor
                        .name_of(reference)
                        .ok()
                        .is_some_and(|name| direct_checkout_head_ref_name.as_ref() == Some(&name)),
                    _ => false,
                };
                if child_is_direct_checkout_ref {
                    // A direct local ref cannot be parented through the target reference
                    // when both refs already participate in the same reachable graph.
                    // Anchor it at the immutable target-tip commit instead.
                    editor.disconnect(Range::single(child), Cut::All, Cut::All, Reconnect::Heal)?;
                    preserve_original_parents(&mut editor, target_ref_commit_entry)?;
                    editor.insert_parent(child, target_ref_commit_entry, 0)?;
                } else {
                    editor.reparent(child, parent, target_ref_entry)?;
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

/// The integration target's coordinates: its recorded sha, its ref, and the ref's current
/// commit — bundled so the two same-typed object ids can't be transposed across the helpers
/// that compare one against the other.
#[derive(Clone, Copy)]
struct TargetRef<'a> {
    sha: gix::ObjectId,
    ref_name: &'a gix::refs::FullNameRef,
    ref_commit: gix::ObjectId,
}

/// The graph entries reachable from the target's sha and from its ref — the stack walk's
/// two stop-sets, bundled so the pair can't be swapped at the call.
struct TargetReach {
    from_sha: HashSet<EditorIndex>,
    from_ref: HashSet<EditorIndex>,
}

fn collect_stacks<'meta, M: RefMetadata>(
    head_commit: gix::Commit<'_>,
    head_is_workspace_commit: bool,
    direct_checkout_ref_entry: Option<EditorIndex>,
    editor: &Editor<'meta, M>,
    reach: TargetReach,
    target: TargetRef<'_>,
    review_hints: &[ReviewIntegrationHint],
) -> Result<Vec<Stack>> {
    let direct_checkout_head_commit_id = head_commit.id;
    let mut stacks = if head_is_workspace_commit {
        editor
            .position_parents(head_commit.id)?
            .into_iter()
            .map(|c| Stack {
                to_merge: false,
                nodes: HashMap::from([(c, AnnotatedNode::new())]),
                heads: HashSet::from([c]),
                bottoms: HashSet::new(),
            })
            .collect()
    } else {
        let c: EditorIndex = match direct_checkout_ref_entry {
            Some(entry) => entry,
            None => editor.select_commit(head_commit.id)?.into(),
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
            for parent in editor.position_parents(tip)? {
                if reach.from_sha.contains(&parent) {
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

    let upstream_entries = reach
        .from_ref
        .iter()
        .filter_map(|s| (!reach.from_sha.contains(s)).then_some(*s))
        .collect::<Vec<_>>();
    let upstream_entries = if upstream_entries.is_empty() {
        reach.from_ref.iter().copied().collect()
    } else {
        upstream_entries
    };
    let upstream_commits = commit_ids(editor, upstream_entries)?;
    let mut workspace_entries = HashSet::new();
    for stack in &output_stacks {
        workspace_entries.extend(stack.nodes.keys());
    }
    let upstream_lut = compute_upstream_commits_lut(editor.repo(), &upstream_commits)?;
    let matches_by_workspace_commit = identify_matching_content(
        editor.repo(),
        &upstream_lut,
        &commit_ids(editor, workspace_entries)?,
    )?;

    for stack in &mut output_stacks {
        let Stack { nodes, bottoms, .. } = stack;

        for node in nodes.keys() {
            if editor
                .position_parents(*node)?
                .iter()
                .all(|p| !nodes.contains_key(p))
            {
                bottoms.insert(*node);
            }
        }

        for (node, attrs) in nodes.iter_mut() {
            let is_local_reference = matches!(
                *node,
                EditorIndex::Ref(reference)
                    if !editor.is_removed(reference)
                        && editor.name_of(reference)?.category()
                            == Some(gix::refs::Category::LocalBranch)
            );
            // Reachability also marks REFERENCES: a ref chain reachable from the target lies
            // on integrated history (its anchor commit is integrated), so this is anchor-based
            // integration expressed through the graph walk. A local ref checked out directly
            // is exempt: its position alone is no evidence its branch was integrated.
            if reach.from_ref.contains(node)
                && !(!head_is_workspace_commit && is_local_reference && stack.heads.contains(node))
            {
                attrs.historically_integrated = true;
            }

            if let EditorIndex::Commit(commit) = *node
                && !editor.is_removed(commit)
                && let Ok(id) = editor.id_of(commit)
                && matches_by_workspace_commit.contains_key(&id)
            {
                attrs.content_integrated = true;
            }
        }

        apply_review_integration_hints(editor, stack, review_hints)?;

        let reference_nodes = stack
            .nodes
            .keys()
            .filter_map(|n| match *n {
                EditorIndex::Ref(reference) if !editor.is_removed(reference) => {
                    Some(editor.name_of(reference).map(|refname| (*n, refname)))
                }
                _ => None,
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
                let parents = editor.position_parents(tip)?;
                if parents.len() != 1 {
                    linear = false;
                }
                for parent in parents {
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
                    entry_commit_id(editor, base)?,
                    entry_commit_id(editor, *r_sel)?,
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
                target,
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
fn empty_local_reference_remote_tip_integrated<'meta, M: RefMetadata>(
    editor: &Editor<'meta, M>,
    entry: EditorIndex,
    ref_name: &gix::refs::FullNameRef,
    reference_nodes: &HashMap<EditorIndex, gix::refs::FullName>,
    target: TargetRef<'_>,
) -> Result<bool> {
    if ref_name.category() != Some(gix::refs::Category::LocalBranch) {
        return Ok(false);
    }
    // An empty branch sitting on another local branch is that stack's forward-going tip and
    // survives the cleanup — unless the parent branch literally rests at the target position.
    if editor.position_parents(entry)?.iter().any(|parent| {
        reference_nodes.get(parent).is_some_and(|parent_ref| {
            parent_ref.category() == Some(gix::refs::Category::LocalBranch)
                && !reference_points_to_target(editor.repo(), parent_ref.as_ref(), target)
        })
    }) {
        return Ok(false);
    }

    let Ok(remote_ref_name) = resolve_tracking_branch_ref_name(ref_name, editor.repo()) else {
        return Ok(false);
    };
    if remote_ref_name.as_bstr() == target.ref_name.as_bstr() {
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
    if remote_tip_id == target.sha && remote_tip_id != target.ref_commit {
        return Ok(false);
    }

    Ok(editor
        .repo()
        .merge_base(remote_tip_id, target.ref_commit)
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
    target: TargetRef<'_>,
) -> bool {
    repo.try_find_reference(ref_name)
        .ok()
        .flatten()
        .and_then(|mut reference| reference.peel_to_id().ok())
        .map(|id| {
            let id = id.detach();
            id == target.sha || id == target.ref_commit
        })
        .unwrap_or(false)
}

/// Whether `ref_name` is the local counterpart of the target — `main` to `origin/main`.
fn is_target_local_branch(
    ref_name: &gix::refs::FullNameRef,
    target_ref: &gix::refs::FullNameRef,
) -> bool {
    let Some((gix::refs::Category::LocalBranch, short)) = ref_name.category_and_short_name() else {
        return false;
    };
    target_ref
        .shorten()
        .rsplit(|b| *b == b'/')
        .next()
        .is_some_and(|target_short| target_short == short)
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
    editor: &Editor<'_, M>,
    stack: &mut Stack,
    review_hints: &[ReviewIntegrationHint],
) -> Result<()> {
    let mut entries_by_commit_id = HashMap::<gix::ObjectId, EditorIndex>::new();
    for entry in stack.nodes.keys().copied() {
        if let EditorIndex::Commit(commit) = entry
            && !editor.is_removed(commit)
        {
            entries_by_commit_id.insert(editor.id_of(commit)?, entry);
        }
    }

    let matching_heads = review_hints
        .iter()
        .filter_map(|hint| {
            entries_by_commit_id
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
    editor: &Editor<'_, M>,
    stack: &Stack,
    matching_heads: &[EditorIndex],
) -> Result<Vec<EditorIndex>> {
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

/// Convert a list of entries into their current commit ids.
///
/// Use the commit ids with great care as they might go out of date or have
/// expected parentages after mutations in the editor.
///
/// Prefer using the entries if possible.
fn commit_ids<'meta, M: RefMetadata>(
    editor: &Editor<'meta, M>,
    entries: impl IntoIterator<Item = EditorIndex>,
) -> Result<Vec<gix::ObjectId>> {
    entries
        .into_iter()
        .filter_map(|s| match s {
            EditorIndex::Commit(commit) if !editor.is_removed(commit) => Some(editor.id_of(commit)),
            _ => None,
        })
        .collect()
}

fn entry_commit_id<M: RefMetadata>(
    editor: &Editor<'_, M>,
    entry: EditorIndex,
) -> Result<Option<gix::ObjectId>> {
    if editor.is_removed(entry) {
        return Ok(None);
    }
    Ok(match entry {
        EditorIndex::Commit(commit) => Some(editor.id_of(commit)?),
        EditorIndex::Ref(reference) => {
            let refname = editor.name_of(reference)?;
            Some(
                editor
                    .repo()
                    .find_reference(refname.as_ref())?
                    .id()
                    .detach(),
            )
        }
    })
}

/// Replace a fully integrated checkout reference with a new canned local branch at the latest
/// target tip.
///
/// Deleting the checked-out branch or empty managed workspace reference would leave `HEAD`
/// pointing at a missing ref. Instead, reuse the checkout reference step for a fresh branch name
/// and point it at the latest target commit.
///
/// The old checkout reference can be on the target ancestry path. Before repointing the step to
/// the target tip, `disconnect()` rewires its children around the old reference to
/// preserve the existing graph and avoid introducing a cycle.
fn replace_checkout_ref_with_fallback<M: RefMetadata>(
    editor: &mut Editor<'_, M>,
    repo: &gix::Repository,
    head_ref_name: &gix::refs::FullNameRef,
    target_tip_entry: EditorIndex,
) -> Result<(EditorIndex, gix::refs::FullName)> {
    let head_ref_entry = editor.select_reference(head_ref_name)?;
    let fallback_ref_name = unique_canned_refname(repo)?;

    editor.rename_reference(head_ref_entry, fallback_ref_name.clone())?;

    editor.disconnect(
        Range::single(head_ref_entry),
        Cut::All,
        Cut::All,
        Reconnect::Heal,
    )?;
    preserve_original_parents(editor, target_tip_entry)?;
    editor.insert_parent(head_ref_entry, target_tip_entry, 0)?;

    Ok((head_ref_entry.into(), fallback_ref_name))
}

/// Remove whatever `entry` addresses — a commit drops in place, a reference deletes
/// with dependents healing past it; a removed entry is a no-op.
fn remove_entry<M: RefMetadata>(editor: &mut Editor<'_, M>, entry: EditorIndex) -> Result<()> {
    if editor.is_removed(entry) {
        return Ok(());
    }
    match entry {
        EditorIndex::Commit(commit) => editor.drop_commit(commit),
        EditorIndex::Ref(reference) => editor.remove_reference(reference),
    }
}

fn preserve_original_parents<M: RefMetadata>(
    editor: &mut Editor<'_, M>,
    entry: EditorIndex,
) -> Result<()> {
    let EditorIndex::Commit(commit_ix) = entry else {
        bail!("Expected target tip to address a commit");
    };
    let mut spec = editor.spec_of(commit_ix)?;
    let commit = editor.find_commit(spec.id)?;
    // TODO: Teach but-rebase to treat immutable reference parents as object
    // anchors. Until then, preserve the target tip's original parents here so
    // graph-rebase materializes the fallback branch at the exact target ref
    // object instead of replaying merge-based target history into an equivalent
    // local rewrite.
    spec.preserved_parents = Some(commit.inner.parents.iter().copied().collect());
    editor.replace_commit(commit_ix, spec)?;
    Ok(())
}

/// Fast-forward the local branch that tracks a remote `target_ref`, preferring the same name.
///
/// Local target refs, missing tracking branches, checked-out branches, and non-fast-forward updates
/// are left unchanged.
pub fn fast_forward_local_tracking_branch(
    repo: &gix::Repository,
    target_ref: &gix::refs::FullNameRef,
    target_id: gix::ObjectId,
) -> Result<()> {
    let Some(local_ref_name) = local_tracking_branch_to_fast_forward(repo, target_ref, target_id)?
    else {
        return Ok(());
    };
    let local_id = repo.find_reference(&local_ref_name)?.id().detach();

    repo.reference(
        local_ref_name,
        target_id,
        gix::refs::transaction::PreviousValue::ExistingMustMatch(gix::refs::Target::Object(
            local_id,
        )),
        "integrate upstream: fast-forward local target",
    )?;
    Ok(())
}

/// Return the local tracking branch that can safely be fast-forwarded to `target_id`.
pub fn local_tracking_branch_to_fast_forward(
    repo: &gix::Repository,
    target_ref: &gix::refs::FullNameRef,
    target_id: gix::ObjectId,
) -> Result<Option<gix::refs::FullName>> {
    let local_ref_name = match target_ref.category() {
        Some(gix::refs::Category::RemoteBranch) => {
            let target_short_name =
                but_core::extract_remote_name_and_short_name(target_ref, &repo.remote_names())
                    .map(|(_, short_name)| short_name);
            let tracks_target = |name: &gix::refs::FullNameRef| -> Result<bool> {
                Ok(repo
                    .branch_remote_tracking_ref_name(name, gix::remote::Direction::Fetch)
                    .transpose()?
                    .is_some_and(|name| name.as_bstr() == target_ref.as_bstr()))
            };
            let preferred = if let Some(short_name) = target_short_name.as_ref() {
                let preferred =
                    gix::refs::Category::LocalBranch.to_full_name(short_name.as_bstr())?;
                if let Some(reference) = repo.try_find_reference(&preferred)?
                    && tracks_target(reference.name())?
                {
                    Some(preferred)
                } else {
                    None
                }
            } else {
                None
            };
            if preferred.is_some() {
                preferred
            } else {
                let mut fallback = None;
                for reference in repo.references()?.prefixed("refs/heads/")? {
                    let Ok(reference) = reference else {
                        continue;
                    };
                    if !tracks_target(reference.name())? {
                        continue;
                    }
                    fallback.get_or_insert_with(|| reference.name().to_owned());
                }
                fallback
            }
        }
        _ => None,
    };
    let Some(local_ref_name) = local_ref_name else {
        return Ok(None);
    };
    let mut local_ref = repo.find_reference(&local_ref_name)?;
    let local_id = local_ref.peel_to_id()?.detach();
    if local_id == target_id
        || !repo
            .merge_base(local_id, target_id)
            .is_ok_and(|base| base.detach() == local_id)
    {
        return Ok(None);
    }
    if but_core::branch::SafeDelete::new(repo)?
        .worktree_dirs_with_ref(&local_ref)
        .is_some()
    {
        return Ok(None);
    }

    Ok(Some(local_ref_name))
}
