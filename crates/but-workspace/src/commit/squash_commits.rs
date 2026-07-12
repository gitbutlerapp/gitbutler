//! An action to squash multiple commits into a target commit.

use anyhow::{Result, bail};
use but_core::{RefMetadata, RepositoryExt};
use but_rebase::graph_rebase::{
    CommitIndex, CommitSpec, Editor, EditorIndex, RebasedEditor,
    merge_commit_changes::MergeCommitChangesOutcome,
};

/// The result of a squash_commits operation.
#[derive(Debug)]
pub struct SquashCommitsOutcome<'meta, M: RefMetadata> {
    /// The successful rebase result.
    pub rebase: RebasedEditor<'meta, M>,
    /// The squashed replacement commit.
    pub commit: CommitIndex,
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

/// Build the squashed commit and replace the target entry with the newly
/// created commit.
///
/// Returns the updated editor and the handle that now points to the squashed
/// commit.
fn construct_new_squashed_commit<'meta, M: RefMetadata>(
    mut editor: Editor<'meta, M>,
    squashed_tree: MergeCommitChangesOutcome,
    target_commit_id: CommitIndex,
    combined_message: Vec<u8>,
) -> Result<(Editor<'meta, M>, CommitIndex)> {
    let target_entry = target_commit_id;
    let target_commit = editor.commit_of(target_entry)?;
    let target_parent_ids = parent_commit_ids(&editor, target_entry)?;

    let new_commit = editor.new_squashed_commit(
        target_commit.clone(),
        target_parent_ids,
        squashed_tree,
        combined_message,
    )?;
    editor.replace_commit(target_entry, CommitSpec::new(new_commit))?;

    Ok((editor, target_entry))
}

fn parent_commit_ids<M: RefMetadata>(
    editor: &Editor<'_, M>,
    entry: CommitIndex,
) -> Result<Vec<gix::ObjectId>> {
    let mut parents = editor.direct_parents(entry)?;
    parents.sort_by_key(|(_, order)| *order);

    parents
        .into_iter()
        .map(|(parent_entry, _)| {
            if editor.is_removed(parent_entry) {
                bail!(
                    "BUG: expected parent entry {parent_entry:?} to be a live commit or reference"
                )
            }
            match parent_entry {
                EditorIndex::Commit(commit) => {
                    let commit = editor.commit_of(commit)?;
                    Ok(commit.id)
                }
                EditorIndex::Ref(reference) => {
                    let (_, commit) = editor.target_of(reference)?;
                    Ok(commit.id)
                }
            }
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
pub fn squash_commits<'meta, M: RefMetadata>(
    editor: Editor<'meta, M>,
    subjects: Vec<gix::ObjectId>,
    target_commit: gix::ObjectId,
    how_to_combine_messages: MessageCombinationStrategy,
) -> Result<SquashCommitsOutcome<'meta, M>> {
    let mut seen_subjects = std::collections::HashSet::with_capacity(subjects.len());

    if subjects.is_empty() {
        bail!("Need at least 2 commits to squash")
    }

    let target_commit_entry = editor.select_commit(target_commit)?;
    let target_commit_obj = editor.commit_of(target_commit_entry)?;

    let mut subject_entries = Vec::with_capacity(subjects.len());
    for subject_commit in subjects {
        let subject_commit_entry = editor.select_commit(subject_commit)?;
        if subject_commit_entry == target_commit_entry {
            bail!("Cannot squash a commit into itself")
        }
        if !seen_subjects.insert(subject_commit_entry) {
            continue;
        }
        subject_entries.push(subject_commit_entry);
    }

    let subject_commit_ids = subject_entries
        .iter()
        .map(|commit| {
            let commit = editor.commit_of(*commit)?;
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
            for source_id in subject_entries.iter().copied() {
                let source_commit = editor.commit_of(source_id)?;
                push_message_with_spacing(&mut combined_message, source_commit.message.as_ref());
            }
        }
        MessageCombinationStrategy::KeepTarget => {
            push_message_with_spacing(&mut combined_message, target_commit_obj.message.as_ref());
        }
        MessageCombinationStrategy::KeepBoth => {
            push_message_with_spacing(&mut combined_message, target_commit_obj.message.as_ref());
            for source_id in subject_entries.iter().copied() {
                let source_commit = editor.commit_of(source_id)?;
                push_message_with_spacing(&mut combined_message, source_commit.message.as_ref());
            }
        }
    }

    let mut editor = editor;
    for commit in subject_entries {
        // A linked worktree checked out on a squashed commit moves onto the combined result.
        for reference in super::linked_worktree_refs_on(&editor, commit)? {
            editor.insert_parent(reference, target_commit_entry, 0)?;
        }
        editor.remove_commit(commit)?;
    }

    // Removing a subject that sits below the target rewrites every commit above it, including
    // the target. Rebase before building the squashed commit so it is created on top of the
    // rebased parents. Otherwise the squashed commit would be replayed as a diff against its
    // old parent, and the subject's changes would disappear along with the subject commit.
    let editor = editor.rebase()?.into_editor();

    let (editor, new_target_entry) = construct_new_squashed_commit(
        editor,
        squashed_tree,
        target_commit_entry,
        combined_message,
    )?;

    Ok(SquashCommitsOutcome {
        rebase: editor.rebase()?,
        commit: new_target_entry,
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
