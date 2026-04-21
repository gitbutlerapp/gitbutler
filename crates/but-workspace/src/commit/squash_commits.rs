//! An action to squash multiple commits into a target commit.

use anyhow::{Result, bail};
use but_core::RefMetadata;
use but_rebase::{
    commit::DateMode,
    graph_rebase::{
        Editor, Selector, Step, SuccessfulRebase, ToCommitSelector, mutate::InsertSide,
    },
};
use std::collections::BTreeMap;

/// The result of a squash_commits operation.
#[derive(Debug)]
pub struct SquashCommitsOutcome<'ws, 'meta, M: RefMetadata> {
    /// The successful rebase result.
    pub rebase: SuccessfulRebase<'ws, 'meta, M>,
    /// Selector pointing to the squashed replacement commit.
    pub commit_selector: Selector,
    /// The final squashed commit ID after remapping through all rewrites.
    pub new_commit: gix::ObjectId,
}

/// Append `message` to `combined`, inserting enough newlines so there are at
/// least two `\n` bytes between existing and appended non-empty blocks.
///
/// Empty `message` values are ignored.
fn push_message_with_spacing(combined: &mut Vec<u8>, message: &[u8]) {
    if message.is_empty() {
        return;
    }

    if !combined.is_empty() {
        let trailing_newlines = combined
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\n')
            .count();
        if trailing_newlines < 2 {
            for _ in trailing_newlines..2 {
                combined.push(b'\n');
            }
        }
    }

    combined.extend_from_slice(message);
}

fn resolve_mapped_commit_id(
    commit_id: gix::ObjectId,
    rewritten_commits: &BTreeMap<gix::ObjectId, gix::ObjectId>,
) -> gix::ObjectId {
    let mut current = commit_id;
    while let Some(next) = rewritten_commits.get(&current).copied() {
        if next == current {
            break;
        }
        current = next;
    }
    current
}

/// Reorder commits around `target_commit` so all selected commits become
/// adjacent around the target in parentage order.
///
/// Returns the rewritten editor together with the original below/above anchor
/// commit IDs used for remapping after the first rebase.
fn reorder_commits_around_target<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    ordered_all_commits: &[Selector],
    target_commit: Selector,
) -> Result<(Editor<'ws, 'meta, M>, Selector, Selector)> {
    let target_pos = ordered_all_commits
        .iter()
        .position(|id| *id == target_commit)
        .expect("target commit must be in ordered commit list");

    let (below_commits, target_and_above_commits) = ordered_all_commits.split_at(target_pos);

    let mut below_anchor = target_commit;
    for source_id in below_commits.iter().rev().copied() {
        editor = crate::commit::move_commit_no_rebase(
            editor,
            source_id,
            below_anchor,
            InsertSide::Below,
        )?;
        below_anchor = source_id;
    }

    let mut above_anchor = target_commit;
    for source_id in target_and_above_commits.iter().skip(1).copied() {
        editor = crate::commit::move_commit_no_rebase(
            editor,
            source_id,
            above_anchor,
            InsertSide::Above,
        )?;
        above_anchor = source_id;
    }

    Ok((editor, below_anchor, above_anchor))
}

/// Build the squashed commit from the mapped top/bottom commits and replace the
/// bottom selector with the newly created commit.
///
/// Returns the updated editor, the selector that now points to the squashed
/// commit, and the initially created commit ID.
fn construct_new_squashed_commit<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    top_most_commit_id: Selector,
    bottom_most_commit_id: Selector,
    combined_message: Vec<u8>,
) -> Result<(Editor<'ws, 'meta, M>, Selector, gix::ObjectId)> {
    let (_, top_most_commit) = editor.find_selectable_commit(top_most_commit_id)?;
    let (bottom_most_selector, bottom_most_commit) =
        editor.find_selectable_commit(bottom_most_commit_id)?;

    let new_commit = {
        let mut squashed_commit = bottom_most_commit.clone();
        squashed_commit.tree = top_most_commit.tree;
        squashed_commit.message = combined_message.into();
        editor.new_commit(squashed_commit, DateMode::CommitterUpdateAuthorKeep)?
    };

    editor.replace(bottom_most_selector, Step::new_pick(new_commit))?;

    Ok((editor, bottom_most_selector, new_commit))
}

/// Squash `subject_commit_ids` into `target_commit`.
///
/// `subject_commit_ids` may be provided in any order. They are ordered by
/// parentage internally together with `target_commit` before reordering and
/// squashing.
///
/// The resulting squashed commit keeps the tree of the top-most selected commit
/// after reordering, and its message is composed as follows:
/// - target commit message first
/// - then source commit messages in child-to-parent order
/// - with at least two newlines between non-empty message blocks
///
pub fn squash_commits<'ws, 'meta, M: RefMetadata, S: ToCommitSelector, T: ToCommitSelector>(
    editor: Editor<'ws, 'meta, M>,
    subject_commit_ids: Vec<S>,
    target_commit: T,
) -> Result<SquashCommitsOutcome<'ws, 'meta, M>> {
    if subject_commit_ids.is_empty() {
        bail!("Need at least 2 commits to squash")
    }

    let (target_commit_selector, target_commit_obj) =
        editor.find_selectable_commit(target_commit)?;

    let mut all_commits = Vec::with_capacity(subject_commit_ids.len() + 1);
    all_commits.push(target_commit_selector);
    for subject_commit in subject_commit_ids {
        let (subject_commit_selector, _) = editor.find_selectable_commit(subject_commit)?;
        if subject_commit_selector == target_commit_selector {
            bail!("Cannot squash a commit into itself")
        }
        all_commits.push(subject_commit_selector);
    }

    let ordered_selectors = editor.order_commit_selectors_by_parentage(all_commits)?;

    let mut combined_message = Vec::new();
    push_message_with_spacing(&mut combined_message, target_commit_obj.message.as_ref());
    for source_id in ordered_selectors
        .iter()
        .rev()
        .copied()
        .filter(|commit_selector| *commit_selector != target_commit_selector)
    {
        let (_, source_commit) = editor.find_selectable_commit(source_id)?;
        push_message_with_spacing(&mut combined_message, source_commit.message.as_ref());
    }

    let (editor, below_anchor, above_anchor) =
        reorder_commits_around_target(editor, &ordered_selectors, target_commit_selector)?;

    let rebase = editor.rebase()?;
    let editor = rebase.into_editor();

    for commit_selector in &ordered_selectors {
        let (_, commit) = editor.find_selectable_commit(*commit_selector)?;
        if commit.clone().attach(editor.repo()).is_conflicted() {
            bail!(
                "Commit {} became conflicted after reordering. Can't continue with squash.",
                commit.id
            );
        }
    }

    let top_most_commit_id = above_anchor;
    let bottom_most_commit_id = below_anchor;

    let (editor, bottom_most_selector, new_commit) = construct_new_squashed_commit(
        editor,
        top_most_commit_id,
        bottom_most_commit_id,
        combined_message,
    )?;

    let rebase = editor.rebase()?;
    let mut editor = rebase.into_editor();

    for commit_selector in ordered_selectors {
        if commit_selector == bottom_most_commit_id {
            continue;
        }
        let Ok((selector, _)) = editor.find_selectable_commit(commit_selector) else {
            continue;
        };
        editor.replace(selector, Step::None)?;
    }

    let rebase = editor.rebase()?;
    let final_rewritten_commits = rebase.history.commit_mappings();
    let final_new_commit = resolve_mapped_commit_id(new_commit, &final_rewritten_commits);

    Ok(SquashCommitsOutcome {
        rebase,
        commit_selector: bottom_most_selector,
        new_commit: final_new_commit,
    })
}

#[cfg(test)]
mod tests {
    use super::push_message_with_spacing;

    #[test]
    fn push_message_with_spacing_adds_first_message_without_padding() {
        let mut combined = Vec::new();
        push_message_with_spacing(&mut combined, b"target");
        assert_eq!(combined, b"target");
    }

    #[test]
    fn push_message_with_spacing_ignores_empty_message() {
        let mut combined = b"target".to_vec();
        push_message_with_spacing(&mut combined, b"");
        assert_eq!(combined, b"target");
    }

    #[test]
    fn push_message_with_spacing_inserts_two_newlines_when_none_present() {
        let mut combined = b"target".to_vec();
        push_message_with_spacing(&mut combined, b"source");
        assert_eq!(combined, b"target\n\nsource");
    }

    #[test]
    fn push_message_with_spacing_inserts_one_newline_when_one_present() {
        let mut combined = b"target\n".to_vec();
        push_message_with_spacing(&mut combined, b"source");
        assert_eq!(combined, b"target\n\nsource");
    }

    #[test]
    fn push_message_with_spacing_keeps_existing_two_newlines() {
        let mut combined = b"target\n\n".to_vec();
        push_message_with_spacing(&mut combined, b"source");
        assert_eq!(combined, b"target\n\nsource");
    }

    #[test]
    fn push_message_with_spacing_keeps_existing_three_newlines() {
        let mut combined = b"target\n\n\n".to_vec();
        push_message_with_spacing(&mut combined, b"source");
        assert_eq!(combined, b"target\n\n\nsource");
    }
}
