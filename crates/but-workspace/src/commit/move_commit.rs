//! Move a commit within or across branches and stacks.

use but_core::RefMetadata;
use but_rebase::graph_rebase::{
    Editor, LookupStep, SuccessfulRebase, ToCommitSelector, ToSelector,
    mutate::{InsertSide, SegmentDelimiter, SelectorSet, SomeSelectors},
};

/// Move a commit.
///
/// `editor` is assumed to be aligned with the graph being mutated.
///
/// `subject_commit` - The commit to be moved.
///
/// `anchor` - A git graph node selector to move the subject commit relative to.
///
/// `side` - The side relative to the anchor at which to insert the subject commit.
///
/// The subject commit will be detached from the source segment, and inserted relative
/// to a given anchor (branch or commit).
pub fn move_commit<'ws, 'meta, M: RefMetadata>(
    editor: Editor<'ws, 'meta, M>,
    subject_commit: impl ToCommitSelector,
    anchor: impl ToSelector,
    side: InsertSide,
) -> anyhow::Result<SuccessfulRebase<'ws, 'meta, M>> {
    let editor = move_commit_no_rebase(editor, subject_commit, anchor, side)?;
    editor.rebase()
}

/// Move a commit without rebasing.
///
/// `editor` is assumed to be aligned with the graph being mutated.
///
/// `subject_commit` - The commit to be moved.
///
/// `anchor` - A git graph node selector to move the subject commit relative to.
///
/// `side` - The side relative to the anchor at which to insert the subject commit.
///
/// The subject commit will be detached from the source segment, and inserted relative
/// to a given anchor (branch or commit).
///
/// This function mutates the editor graph but does not execute a rebase.
pub fn move_commit_no_rebase<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    subject_commit: impl ToCommitSelector,
    anchor: impl ToSelector,
    side: InsertSide,
) -> anyhow::Result<Editor<'ws, 'meta, M>> {
    let (subject_commit_selector, _) = editor.find_selectable_commit(subject_commit)?;

    let commit_delimiter = SegmentDelimiter {
        child: subject_commit_selector,
        parent: subject_commit_selector,
    };

    // Step 1: Determine the parents to disconnect.
    let parent_to_disconnect = determine_parent_selector(&editor, subject_commit_selector)?;

    // Step 2: Disconnect
    editor.disconnect_segment_from(
        commit_delimiter.clone(),
        SelectorSet::All,
        parent_to_disconnect,
        false,
    )?;

    // Step 3: Insert
    editor.insert_segment(anchor, commit_delimiter, side)?;
    Ok(editor)
}

/// Determine which parent to disconnect from the subject commit.
///
/// Preference rules:
/// - Prefer a `Pick` parent first. This matches first-parent linear history
///   semantics, which is the primary ancestry edge we want to detach when
///   moving a commit within or across stacks.
/// - If there is no commit parent edge, fall back to a `Reference` parent.
///
/// If no explicit parent candidate is available (e.g. truncated history or
/// root-like scenarios), we use `SelectorSet::All` as a safe fallback,
/// matching prior behavior for these edge cases.
fn determine_parent_selector<'ws, 'meta, M: RefMetadata>(
    editor: &Editor<'ws, 'meta, M>,
    subject_commit_selector: but_rebase::graph_rebase::Selector,
) -> Result<SelectorSet, anyhow::Error> {
    let mut parents = editor.direct_parents(subject_commit_selector)?;
    parents.sort_by_key(|(_, order)| *order);

    // Prefer parent commit first (linear segment), then reference fallback.
    let preferred = parents
        .iter()
        .find(|(selector, _)| {
            matches!(
                editor.lookup_step(*selector),
                Ok(but_rebase::graph_rebase::Step::Pick(_))
            )
        })
        .or_else(|| {
            parents.iter().find(|(selector, _)| {
                matches!(
                    editor.lookup_step(*selector),
                    Ok(but_rebase::graph_rebase::Step::Reference { .. })
                )
            })
        })
        .map(|(selector, _)| *selector);

    let parent_to_disconnect = match preferred {
        Some(selector) => {
            let selectors = SomeSelectors::new(vec![selector])?;
            SelectorSet::Some(selectors)
        }
        // No explicit parent available (e.g. root commit/truncated history).
        None => SelectorSet::All,
    };
    Ok(parent_to_disconnect)
}
