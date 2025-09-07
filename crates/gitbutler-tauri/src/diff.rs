use but_api::commands::diff::{self, CommitDetails};
use but_api::error::Error;
use but_api::hex_hash::HexHash;
use but_core::ui::{TreeChange, TreeChanges};
use but_hunk_assignment::{AssignmentRejection, HunkAssignmentRequest, WorktreeChanges};
use but_workspace::StackId;
use gitbutler_project::ProjectId;
use gitbutler_reference::Refname;
/// Provide a unified diff for `change`, but fail if `change` is a [type-change](but_core::ModeFlags::TypeChange)
/// or if it involves a change to a [submodule](gix::object::Kind::Commit).
#[tauri::command(async)]
pub fn tree_change_diffs(
    project_id: ProjectId,
    change: TreeChange,
) -> anyhow::Result<but_core::UnifiedDiff, Error> {
    diff::tree_change_diffs(project_id, change)
}

#[tauri::command(async)]
pub fn commit_details(
    project_id: ProjectId,
    commit_id: HexHash,
) -> anyhow::Result<CommitDetails, Error> {
    diff::commit_details(project_id, commit_id)
}

/// Gets the changes for a given branch.
/// If the branch is part of a stack and if the stack_id is provided, this will include only the changes
/// up to the next branch in the stack.
/// Otherwise, if stack_id is not provided, this will include all changes as compared to the target branch
/// Note that `stack_id` is deprecated in favor of `branch_name`
/// *(which should be a full ref-name as well and make `remote` unnecessary)*
#[tauri::command(async)]
pub fn changes_in_branch(
    project_id: ProjectId,
    // TODO: remove this, go by name. Ideally, the UI would pass us two commits.
    _stack_id: Option<StackId>,
    branch: Refname,
) -> anyhow::Result<TreeChanges, Error> {
    diff::changes_in_branch(project_id, _stack_id, branch)
}

/// This UI-version of [`but_core::diff::worktree_changes()`] simplifies the `git status` information for display in
/// the user interface as it is right now. From here, it's always possible to add more information as the need arises.
///
/// ### Notable Transformations
/// * There is no notion of an index (`.git/index`) - all changes seem to have happened in the worktree.
/// * Modifications that were made to the index will be ignored *only if* there is a worktree modification to the same file.
/// * conflicts are ignored
///
/// All ignored status changes are also provided so they can be displayed separately.
#[tauri::command(async)]
pub fn changes_in_worktree(project_id: ProjectId) -> anyhow::Result<WorktreeChanges, Error> {
    diff::changes_in_worktree(project_id)
}

#[tauri::command(async)]
pub fn assign_hunk(
    project_id: ProjectId,
    assignments: Vec<HunkAssignmentRequest>,
) -> anyhow::Result<Vec<AssignmentRejection>, Error> {
    diff::assign_hunk(project_id, assignments)
}
