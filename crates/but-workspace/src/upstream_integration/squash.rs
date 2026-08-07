//! Detection of squash-merged stacks during upstream integration.

use anyhow::Result;
use but_core::RefMetadata;
use but_core::changeset::{SimilarityByCommitIds, SquashCandidate, squash_merge_boundary};
use but_rebase::graph_rebase::{Editor, LookupStep, Pick, Step};

use super::{Stack, selector_commit_id};

/// Detect a squash-merged stack: the cumulative changeset of its commits matching a single
/// upstream commit. Per-commit similarity can never find these for multi-commit stacks, which
/// would otherwise be rebased onto content they already contain and end up as empty (and
/// possibly conflicted) commits instead of being dropped.
///
/// This is the same [`squash_merge_boundary`](but_core::changeset::squash_merge_boundary)
/// policy that `RefInfo::compute_similarity` uses, applied to the editor's step graph: on a
/// match, the boundary and everything beneath it is marked `content_integrated`.
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
    // Walk the chain top-to-bottom, collecting each commit's trial candidate and selector
    // (segments are delimited by `Reference` steps), and the base beneath the stack.
    let mut selectors = Vec::new();
    let mut candidates = Vec::new();
    let mut segment = 0usize;
    let base = loop {
        match editor.lookup_step(cursor)? {
            Step::Pick(Pick { id, .. }) => {
                candidates.push(SquashCandidate {
                    id,
                    integrated: stack
                        .nodes
                        .get(&cursor)
                        .is_some_and(|attrs| attrs.is_integrated()),
                    segment,
                });
                selectors.push(cursor);
            }
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

    if let Some((boundary, _)) =
        squash_merge_boundary(repo, &integration.upstream_lut, base, &candidates)?
    {
        // Mark the boundary and everything beneath it (across lower segments) integrated.
        for selector in selectors.iter().skip(boundary) {
            if let Some(attrs) = stack.nodes.get_mut(selector) {
                attrs.content_integrated = true;
            }
        }
    }
    Ok(())
}
