//! Detection of squash-merged stacks during upstream integration.

use anyhow::Result;
use but_core::RefMetadata;
use but_core::changeset::{SimilarityByCommitIds, tree_introduces_changes};
use but_core::commit::TreeKind;
use but_rebase::graph_rebase::{Editor, LookupStep, Pick, Step};
use gix::prelude::ObjectIdExt;

use super::{Stack, selector_commit_id};

/// Detect a squash-merged stack: the cumulative changeset of its commits matching a single
/// upstream commit. Per-commit similarity can never find these for multi-commit stacks, which
/// would otherwise be rebased onto content they already contain and end up as empty (and
/// possibly conflicted) commits instead of being dropped.
///
/// This mirrors the squash-merge trial in `RefInfo::compute_similarity`: for each segment,
/// top to bottom, find its topmost commit that isn't already integrated and introduces
/// changes of its own (the boundary), compute the changeset from the stack's base to that
/// commit, and on an upstream match mark the boundary and everything beneath it
/// `content_integrated`. A failed trial retries at the next lower segment's boundary: the
/// topmost segment may carry unmerged work stacked on a squash-merged segment below it.
/// Commits above a matched boundary stay untouched: a no-change tip commit borrows the
/// cumulative content beneath it and must not have its branch deleted by a match.
///
/// Only linear stacks are trialed; stacks with multiple heads or in-stack merges are left to
/// per-commit matching.
pub(super) fn squash_merge_trial<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    stack: &mut Stack,
    integration: &SimilarityByCommitIds,
) -> Result<()> {
    let repo = editor.repo();
    let mut cursor = match stack.heads.iter().next() {
        Some(head) if stack.heads.len() == 1 => *head,
        _ => return Ok(()),
    };
    // Walk the chain top-to-bottom, collecting commits tagged with the segment they belong to
    // (segments are delimited by `Reference` steps), and the base beneath the stack.
    let mut picks = Vec::new();
    let mut segment = 0usize;
    let base = loop {
        match editor.lookup_step(cursor)? {
            Step::Pick(Pick { id, .. }) => picks.push((cursor, id, segment)),
            Step::Reference { .. } => segment += 1,
            Step::None => {}
        }
        let parents = editor.direct_parents(cursor)?;
        match parents.as_slice() {
            [] => break None,
            [(parent, _)] if stack.nodes.contains_key(parent) => cursor = *parent,
            [(below, _)] => match selector_commit_id(editor, *below)? {
                Some(id) => break Some(id),
                None => return Ok(()),
            },
            _ => return Ok(()),
        }
    };

    let mut tried_segment = None;
    for (idx, (selector, commit_id, segment)) in picks.iter().enumerate() {
        if tried_segment == Some(*segment) {
            continue;
        }
        let is_integrated = stack
            .nodes
            .get(selector)
            .is_some_and(|attrs| attrs.is_integrated());
        if is_integrated || !commit_introduces_changes(repo, *commit_id) {
            continue;
        }
        tried_segment = Some(*segment);
        if integration
            .squash_merge_match(repo, base, *commit_id)?
            .is_none()
        {
            continue;
        }
        // Mark the boundary and everything beneath it (across lower segments) integrated.
        for (selector, ..) in picks.iter().skip(idx) {
            if let Some(attrs) = stack.nodes.get_mut(selector) {
                attrs.content_integrated = true;
            }
        }
        break;
    }
    Ok(())
}

/// Whether `commit_id` introduces changes of its own, comparing its tree (the auto-resolution
/// tree for conflicted commits) to its first parent's.
fn commit_introduces_changes(repo: &gix::Repository, commit_id: gix::ObjectId) -> bool {
    let Ok(commit) = but_core::Commit::from_id(commit_id.attach(repo)) else {
        return true;
    };
    let Ok(tree_id) = commit.tree_id_or_kind(TreeKind::AutoResolution) else {
        return true;
    };
    tree_introduces_changes(
        repo,
        tree_id.detach(),
        commit.inner.parents.first().copied(),
    )
}
