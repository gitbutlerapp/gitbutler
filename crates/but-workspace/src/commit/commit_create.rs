//! An action to create a new commit relative to a commit or reference.

use anyhow::Result;
use but_core::{DiffSpec, RefMetadata};
use but_rebase::graph_rebase::{
    CommitIndex, CommitSpec, Editor, EditorIndex, RebasedEditor, anchor::Anchor, mutate::InsertSide,
};

use crate::commit_engine::{Destination, create_commit};

use super::{ChangeSource, cancel_consumed_changes};

/// The result of creating and inserting a new commit in the graph rebase editor.
#[derive(Debug)]
pub struct CommitCreateOutcome<'meta, M: RefMetadata> {
    /// A successful rebase result for continuing operations. This will be
    /// always provided regardless of whether a commit was actually
    /// created.
    pub rebase: RebasedEditor<'meta, M>,
    /// The newly created commit, if one was created.
    ///
    /// A commit may not be created if all the diff_specs are rejected. See
    /// [`create_commit`] for more details.
    pub commit: Option<CommitIndex>,
    /// Rejected diff specs from commit creation. See [`create_commit`] for
    /// more details.
    pub rejected_specs: Vec<(but_core::tree::create_tree::RejectionReason, DiffSpec)>,
}

/// Create a commit from `changes` and insert it relative to `relative_to` on `side`.
///
/// Similar to other `editor` based functions, this consumes an editor and
/// gives it back as a [`RebasedEditor`] which can be used to chain more
/// operations or just materialize the result.
///
/// `changes` defines which changes should be committed, and `source` which
/// checkout they are read from - see [`create_commit`] for more details, and
/// [`commit_amend`](super::commit_amend()) for what a [`ChangeSource::Worktree`]
/// requires.
///
/// `relative_to` and `side` determine the position to insert the commit.
/// See [`InsertSide`] to learn more about insertion semantics.
///
/// `message` will be the message used for the newly created commit.
///
/// `context_lines` define how many diff context lines are being used for
/// this particular function call. The provided `context_lines` MUST align
/// with the `context_lines` value used to generate the `DiffSpec`s passed
/// in the `changes` parameter.
pub fn commit_create<'meta, M: RefMetadata>(
    mut editor: Editor<'meta, M>,
    changes: Vec<DiffSpec>,
    relative_to: Anchor,
    side: InsertSide,
    message: &str,
    context_lines: u32,
    source: ChangeSource<'_>,
) -> Result<CommitCreateOutcome<'meta, M>> {
    let relative_to_entry = editor.resolve_anchor(relative_to)?;
    let parent_commit_id = parent_commit_id_for_new_commit(&editor, relative_to_entry, side)?;

    // Clone before `create_commit` consumes the vec — needed afterwards
    // to determine which changes were consumed (not rejected).
    let all_changes = changes.clone();
    let create_out = create_commit(
        source.repo(&editor),
        Destination::NewCommit {
            parent_commit_id,
            stack_segment: None,
            message: message.to_owned(),
        },
        changes,
        context_lines,
    )?;

    let Some(new_commit_id) = create_out.new_commit else {
        return Ok(CommitCreateOutcome {
            rebase: editor.rebase()?,
            commit: None,
            rejected_specs: create_out.rejected_specs,
        });
    };

    // Runs before `insert` so an unknown worktree fails with zero graph mutation.
    cancel_consumed_changes(
        &mut editor,
        &source,
        all_changes,
        &create_out.rejected_specs,
        context_lines,
    )?;

    let commit = editor.insert_commit(
        relative_to_entry,
        CommitSpec::untracked(new_commit_id),
        side,
    )?;

    // RECORD THE INTENT HERE. Committing onto a branch puts it in the workspace, so the
    // declaration has to say so — every other write path (apply, move_branch, create_reference)
    // already declares what it brings in. This one relied on the vb-toml write-back noticing
    // afterwards, which meant a DERIVED view was authoring the declaration.
    let landed_on = match relative_to_entry {
        EditorIndex::Ref(reference) if !editor.is_removed(reference) => {
            Some(editor.name_of(reference)?)
        }
        _ => None,
    };
    let mut rebase = editor.rebase()?;
    if let Some(refname) = landed_on {
        declare_branch_if_absent(&mut rebase, refname)?;
    }

    Ok(CommitCreateOutcome {
        rebase,
        commit: Some(commit),
        rejected_specs: create_out.rejected_specs,
    })
}

fn parent_commit_id_for_new_commit<'meta, M: RefMetadata>(
    editor: &Editor<'meta, M>,
    target: EditorIndex,
    side: InsertSide,
) -> Result<Option<gix::ObjectId>> {
    if editor.is_removed(target) {
        return Ok(None);
    }
    Ok(match (target, side) {
        (EditorIndex::Commit(commit), InsertSide::Above) => Some(editor.id_of(commit)?),
        (EditorIndex::Commit(commit), InsertSide::Below) => {
            let commit = editor.find_commit(editor.id_of(commit)?)?;
            commit.parents.first().copied()
        }
        (EditorIndex::Ref(reference), _) => {
            let refname = editor.name_of(reference)?;
            Some(
                editor
                    .target_of(editor.select_reference(refname.as_ref())?)?
                    .1
                    .id,
            )
        }
    })
}

/// Ensure `ref_name` is declared in the workspace metadata, adding it as its own stack when it is
/// not there yet. A no-op when the branch is already declared, wherever it sits.
fn declare_branch_if_absent<'meta, M: RefMetadata>(
    rebase: &mut RebasedEditor<'meta, M>,
    ref_name: gix::refs::FullName,
) -> Result<()> {
    let (_repo, meta) = rebase.repo_and_meta_mut();
    let mut ws_md = meta.workspace(but_core::WORKSPACE_REF_NAME.try_into()?)?;
    if ws_md.contains_ref(
        ref_name.as_ref(),
        but_core::ref_metadata::StackKind::AppliedAndUnapplied,
    ) {
        return Ok(());
    }
    ws_md.add_or_insert_new_stack_if_not_present(
        ref_name.as_ref(),
        None,
        but_core::ref_metadata::WorkspaceCommitRelation::Merged,
        |_| but_core::ref_metadata::StackId::generate(),
    );
    meta.set_workspace(&ws_md)?;
    Ok(())
}
