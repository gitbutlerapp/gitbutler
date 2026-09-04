//! An action to move changes between commits

use anyhow::{Result, bail};
use but_core::{DiffSpec, RefMetadata, RepositoryExt};
use but_rebase::{
    commit::DateMode,
    graph_rebase::{CommitIndex, CommitSpec, Editor, RebasedEditor},
};

use crate::tree_manipulation::{ChangesSource, create_tree_without_diff};

/// The result of a move_changes_between_commits operation.
#[derive(Debug)]
pub struct MoveChangesOutcome<'meta, M: RefMetadata> {
    /// The successful rebase result
    pub rebase: RebasedEditor<'meta, M>,
    /// CommitIndex pointing to the source commit (with changes removed)
    pub source: CommitIndex,
    /// CommitIndex pointing to the destination commit (with changes added)
    pub destination: CommitIndex,
}

/// Move changes from one commit to another.
///
/// This operation removes the specified changes from the source commit and
/// applies them to the destination commit using a three-way merge.
///
/// ## Parameters
///
/// - `editor`: The rebase editor to use
/// - `source_commit_id`: The commit to remove changes from
/// - `destination_commit_id`: The commit to add changes to
/// - `changes_to_move`: The changes to move (as "subtraction" specs)
/// - `context_lines`: Number of context lines for hunk matching
///
/// ## Returns
///
/// Returns the rebase outcome along with entries pointing to both the
/// modified source and destination commits. The caller should call
/// `outcome.rebase.materialize()` to persist the changes.
pub fn move_changes_between_commits<'meta, M: RefMetadata>(
    mut editor: Editor<'meta, M>,
    source_commit: CommitIndex,
    destination_commit: CommitIndex,
    changes_to_move: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> Result<MoveChangesOutcome<'meta, M>> {
    let source = source_commit;
    let source_commit = editor.commit_of(source)?;
    let (destination, destination_commit) =
        (destination_commit, editor.commit_of(destination_commit)?);

    // Early return if source and destination are the same
    if source_commit.id == destination_commit.id {
        // Select the commit to get a valid entry, then just rebase (no-op)
        let outcome = editor.rebase()?;
        return Ok(MoveChangesOutcome {
            rebase: outcome,
            source,
            destination,
        });
    }

    // Step 1: Get the source commit and its tree
    let source_tree_id = {
        let source_commit = source_commit.clone().attach(editor.repo());
        if source_commit.is_conflicted() {
            bail!("Source commit must not be conflicted")
        }
        source_commit.tree
    };

    let (source_tree_without_changes_id, dropped_diffs) = create_tree_without_diff(
        editor.repo(),
        ChangesSource::Commit {
            id: source_commit.id,
        },
        changes_to_move,
        context_lines,
    )?;

    if !dropped_diffs.is_empty() {
        bail!("Failed to extract described changes from source commit");
    }

    let new_source_commit_id = {
        let mut new_source_commit = source_commit.clone();
        new_source_commit.tree = source_tree_without_changes_id;
        editor.new_commit(new_source_commit, DateMode::CommitterUpdateAuthorKeep)?
    };

    editor.replace_commit(source, CommitSpec::new(new_source_commit_id))?;

    // Rebase and get potentially rebased destination commit
    let mut editor = editor.rebase()?.into_editor();
    let rebased_destination_commit = editor.commit_of(destination)?;
    let destination_tree_id = {
        let rebased_destination_commit = rebased_destination_commit.clone().attach(editor.repo());
        if rebased_destination_commit.is_conflicted() {
            bail!("Destination commit must not be conflicted")
        }
        rebased_destination_commit.tree
    };

    let destination_tree_with_changes = {
        let repo = editor.repo();
        let (fail_fast_options, conflict_kind) = repo.merge_options_fail_fast()?;
        let mut merge_result = repo.merge_trees(
            source_tree_without_changes_id,
            source_tree_id,
            destination_tree_id,
            Default::default(),
            fail_fast_options,
        )?;

        if merge_result.has_unresolved_conflicts(conflict_kind) {
            bail!("Failed to apply changes to destination commit - merge conflict");
        }

        merge_result.tree.write()?.detach()
    };

    let new_destination_commit_id = {
        let mut commit = rebased_destination_commit;
        commit.tree = destination_tree_with_changes;
        editor.new_commit(commit, DateMode::CommitterUpdateAuthorKeep)?
    };

    editor.replace_commit(destination, CommitSpec::new(new_destination_commit_id))?;

    let outcome = editor.rebase()?;

    Ok(MoveChangesOutcome {
        rebase: outcome,
        source,
        destination,
    })
}
