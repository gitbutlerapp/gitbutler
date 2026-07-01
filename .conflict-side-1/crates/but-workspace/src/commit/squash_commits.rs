//! An action to squash multiple commits into a target commit.

use anyhow::{Result, bail};
use but_core::{RefMetadata, RepositoryExt};
use but_rebase::{
    commit::DateMode,
    graph_rebase::{
        Editor, LookupStep as _, Selector, Step, SuccessfulRebase, ToCommitSelector,
        merge_commit_changes::MergeCommitChangesOutcome,
        mutate::{SegmentDelimiter, SelectorSet},
    },
};

/// The result of a squash_commits operation.
#[derive(Debug)]
pub struct SquashCommitsOutcome<'ws, 'meta, M: RefMetadata> {
    /// The successful rebase result.
    pub rebase: SuccessfulRebase<'ws, 'meta, M>,
    /// Selector pointing to the squashed replacement commit.
    pub commit_selector: Selector,
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

/// Build the squashed commit and replace the target selector with the newly
/// created commit.
///
/// Returns the updated editor and the selector that now points to the squashed
/// commit.
fn construct_new_squashed_commit<'ws, 'meta, M: RefMetadata>(
    mut editor: Editor<'ws, 'meta, M>,
    squashed_tree: MergeCommitChangesOutcome,
    target_commit_id: Selector,
    combined_message: Vec<u8>,
) -> Result<(Editor<'ws, 'meta, M>, Selector)> {
    let (target_selector, target_commit) = editor.find_selectable_commit(target_commit_id)?;
    let target_parent_ids = parent_commit_ids(&editor, target_selector)?;

    let new_commit = {
        let mut squashed_commit = target_commit.clone();
        squashed_commit.inner.parents = target_parent_ids.into();
        squashed_commit.tree = squashed_tree.tree_id;
        squashed_commit.message = combined_message.into();
        editor.new_commit(squashed_commit, DateMode::CommitterUpdateAuthorKeep)?
    };

    editor.replace(target_selector, Step::new_pick(new_commit))?;

    Ok((editor, target_selector))
}

fn parent_commit_ids<M: RefMetadata>(
    editor: &Editor<'_, '_, M>,
    selector: Selector,
) -> Result<Vec<gix::ObjectId>> {
    let mut parents = editor.direct_parents(selector)?;
    parents.sort_by_key(|(_, order)| *order);

    parents
        .into_iter()
        .map(|(parent_selector, _)| match editor.lookup_step(parent_selector)? {
            Step::Pick(_) => {
                let (_, commit) = editor.find_selectable_commit(parent_selector)?;
                Ok(commit.id)
            }
            Step::Reference { .. } => {
                let (_, commit) = editor.find_reference_target(parent_selector)?;
                Ok(commit.id)
            }
            Step::None => bail!(
                "BUG: expected parent selector {parent_selector:?} to point to a pick or reference"
            ),
        })
        .collect()
}

/// How to combine messages of commits being squashed.
#[derive(Debug, serde::Serialize, serde::Deserialize, Copy, Clone)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
pub enum MessageCombinationStrategy {
    /// Keep both messages.
    KeepBoth,
    /// Only keep the messages of subject commits.
    ///
    /// Target message will be discarded.
    KeepSubject,
    /// Only keep the message of the target.
    ///
    /// Subject message will be discarded.
    KeepTarget,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(MessageCombinationStrategy);

/// Squash `subjects` into `target_commit`.
///
/// The `target_commit` must not also appear in `subjects`.
/// This operation assumes the provided editor is already normalized and up to
/// date. Callers chaining previous editor mutations should first run
/// `editor.rebase()?.into_editor()` before squashing.
///
/// After squashing, the resulting squashed commit has:
/// - The tree produced from the target commit's full tree plus the subject
///   commits' own change ranges.
/// - A message determined by `how_to_combine_messages`:
///   - `KeepTarget`: target message only.
///   - `KeepSubject`: subject messages only.
///   - `KeepBoth`: target message followed by subject messages.
///
/// Subject messages are appended in the order they are provided, with at least
/// one blank line between non-empty message blocks.
///
pub fn squash_commits<'ws, 'meta, M: RefMetadata, S: ToCommitSelector, T: ToCommitSelector>(
    editor: Editor<'ws, 'meta, M>,
    subjects: Vec<S>,
    target_commit: T,
    how_to_combine_messages: MessageCombinationStrategy,
) -> Result<SquashCommitsOutcome<'ws, 'meta, M>> {
    let mut seen_subjects = std::collections::HashSet::with_capacity(subjects.len());

    if subjects.is_empty() {
        bail!("Need at least 2 commits to squash")
    }

    let (target_commit_selector, target_commit_obj) =
        editor.find_selectable_commit(target_commit)?;

    let mut subject_selectors = Vec::with_capacity(subjects.len());
    for subject_commit in subjects {
        let (subject_commit_selector, _) = editor.find_selectable_commit(subject_commit)?;
        if subject_commit_selector == target_commit_selector {
            bail!("Cannot squash a commit into itself")
        }
        if !seen_subjects.insert(subject_commit_selector) {
            continue;
        }
        subject_selectors.push(subject_commit_selector);
    }

    let subject_commit_ids = subject_selectors
        .iter()
        .map(|commit_selector| {
            let (_, commit) = editor.find_selectable_commit(*commit_selector)?;
            Ok(commit.id)
        })
        .collect::<Result<Vec<_>>>()?;
    let squashed_tree = editor.merge_commit_changes_to_tree(
        target_commit_obj.id,
        subject_commit_ids,
        editor.repo().merge_options_force_ours()?,
    )?;
    if squashed_tree.conflict.is_some() {
        bail!("Cannot squash commits that would result in merge conflicts");
    }

    let mut combined_message = Vec::new();
    match how_to_combine_messages {
        MessageCombinationStrategy::KeepSubject => {
            for source_id in subject_selectors.iter().copied() {
                let (_, source_commit) = editor.find_selectable_commit(source_id)?;
                push_message_with_spacing(&mut combined_message, source_commit.message.as_ref());
            }
        }
        MessageCombinationStrategy::KeepTarget => {
            push_message_with_spacing(&mut combined_message, target_commit_obj.message.as_ref());
        }
        MessageCombinationStrategy::KeepBoth => {
            push_message_with_spacing(&mut combined_message, target_commit_obj.message.as_ref());
            for source_id in subject_selectors.iter().copied() {
                let (_, source_commit) = editor.find_selectable_commit(source_id)?;
                push_message_with_spacing(&mut combined_message, source_commit.message.as_ref());
            }
        }
    }

    let mut editor = editor;
    for commit_selector in subject_selectors {
        let delimiter = SegmentDelimiter {
            child: commit_selector,
            parent: commit_selector,
        };
        editor.disconnect_segment_from(delimiter, SelectorSet::All, SelectorSet::All, false)?;
        let (selector, _) = editor.find_selectable_commit(commit_selector)?;
        editor.replace(selector, Step::None)?;
    }

    let (editor, new_target_selector) = construct_new_squashed_commit(
        editor,
        squashed_tree,
        target_commit_selector,
        combined_message,
    )?;

    Ok(SquashCommitsOutcome {
        rebase: editor.rebase()?,
        commit_selector: new_target_selector,
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
