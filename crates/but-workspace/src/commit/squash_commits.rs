//! An action to squash multiple commits into a target commit.

use anyhow::{Context as _, Result};
use but_core::RefMetadata;
use but_error::bail_precondition;
use but_rebase::{
    commit::DateMode,
    graph_rebase::{
        Editor, LookupStep as _, Selector, SuccessfulRebase, ToCommitSelector, mutate::InsertSide,
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

/// Why [`squash_commits()`] refused: a subject conflicts once it is moved next to the target,
/// because commits between them change the same lines.
///
/// It sits below the [`but_error::Context`] with the message, so callers can tell this refusal
/// from other preconditions by downcasting to it.
#[derive(Debug)]
pub struct SquashConflicts;

impl std::fmt::Display for SquashConflicts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("squashed commits would conflict")
    }
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
        bail_precondition!("Need at least 2 commits to squash")
    }

    let (target_commit_selector, target_commit_obj) =
        editor.find_selectable_commit(target_commit)?;

    let mut subject_selectors = Vec::with_capacity(subjects.len());
    for subject_commit in subjects {
        let (subject_commit_selector, _) = editor.find_selectable_commit(subject_commit)?;
        if subject_commit_selector == target_commit_selector {
            bail_precondition!("Cannot squash a commit into itself")
        }
        if !seen_subjects.insert(subject_commit_selector) {
            continue;
        }
        subject_selectors.push(subject_commit_selector);
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

    let lineage = editor.order_commit_selectors_by_parentage(
        subject_selectors
            .iter()
            .copied()
            .chain([target_commit_selector]),
    )?;
    let mut sides = lineage.split(|selector| *selector == target_commit_selector);
    let below = sides.next().unwrap_or_default();
    let above = sides.next().unwrap_or_default();

    let mut editor = editor;
    for subject in below {
        editor.move_pick(*subject, target_commit_selector, InsertSide::Below)?;
    }
    for subject in above.iter().rev() {
        editor.move_pick(*subject, target_commit_selector, InsertSide::Above)?;
    }
    let mut editor = editor.rebase()?.into_editor();

    for subject in &subject_selectors {
        let (_, commit) = editor.find_selectable_commit(*subject)?;
        if commit.attach(editor.repo()).is_conflicted() {
            return Err(
                anyhow::anyhow!(SquashConflicts).context(but_error::Context::new_static(
                    but_error::Code::PreconditionFailed,
                    "Cannot squash commits that would result in merge conflicts",
                )),
            );
        }
    }
    let child_most = *lineage
        .last()
        .context("BUG: the lineage holds the target")?;
    let squashed_tree = editor.find_selectable_commit(child_most)?.1.tree;

    editor.fold(subject_selectors, target_commit_selector)?;

    let (_, mut squashed_commit) = editor.find_selectable_commit(target_commit_selector)?;
    squashed_commit.inner.parents = editor
        .commit_parents(target_commit_selector)?
        .into_iter()
        .map(|parent| editor.lookup_pick(parent))
        .collect::<Result<Vec<_>>>()?
        .into();
    squashed_commit.tree = squashed_tree;
    squashed_commit.message = combined_message.into();
    let new_commit = editor.new_commit(squashed_commit, DateMode::CommitterUpdateAuthorKeep)?;
    editor.amend_pick(target_commit_selector, new_commit)?;

    Ok(SquashCommitsOutcome {
        rebase: editor.rebase()?,
        commit_selector: target_commit_selector,
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
