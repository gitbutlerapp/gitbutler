//!
//!
//! Hunk - File, range
//!
//! HunkAssignment - None or Some(Stack in workspace)
//!
//! reconcile_assignments - takes worktree changes (Vec<TreeChange>) + current assignments (Vec<HunkAssignment>)
//! returns updated assignments (Vec<HunkAssignment>)
//!
//! set_assignments

mod reconcile;
mod state;

use anyhow::Result;
use bstr::{BString, ByteSlice};
use but_core::{HunkHeader, UnifiedPatch, ref_metadata::StackId};
use but_ctx::Context;
use but_hunk_dependency::ui::{
    HunkDependencies, HunkLock, hunk_dependencies_for_workspace_changes_by_worktree_dir,
};
use gitbutler_stack::VirtualBranchesHandle;
use itertools::Itertools;
use reconcile::MultipleOverlapping;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HunkAssignment {
    /// A stable identifier for the hunk assignment.
    ///   - When a new hunk is first observed (from the uncommitted changes), it is assigned a new id.
    ///   - If a hunk is modified (i.e. it has gained or lost lines), the UUID remains the same.
    ///   - If two or more hunks become merged (due to edits causing the contexts to overlap), the id of the hunk with the most lines is adopted.
    pub id: Option<Uuid>,
    /// The hunk that is being assigned. Together with path_bytes, this identifies the hunk.
    /// If the file is binary, or too large to load, this will be None and in this case the path name is the only identity.
    pub hunk_header: Option<HunkHeader>,
    /// The file path of the hunk.
    pub path: String,
    /// The file path of the hunk in bytes.
    pub path_bytes: BString,
    /// The stack to which the hunk is assigned. If None, the hunk is not assigned to any stack.
    pub stack_id: Option<StackId>,
    /// The dependencies(locks) that this hunk has. This determines where the hunk can be assigned.
    /// This field is ignored when HunkAssignment is passed by the UI to create a new assignment.
    #[serde(skip)]
    pub hunk_locks: Option<Vec<HunkLock>>,
    /// The line numbers that were added in this hunk.
    pub line_nums_added: Option<Vec<usize>>,
    /// The line numbers that were removed in this hunk.
    pub line_nums_removed: Option<Vec<usize>>,
    /// The hunk diff for internal usage. This is not to be persisted or sent over the API.
    #[serde(skip)]
    pub diff: Option<BString>,
}

impl TryFrom<but_db::HunkAssignment> for HunkAssignment {
    type Error = anyhow::Error;
    fn try_from(value: but_db::HunkAssignment) -> Result<Self, Self::Error> {
        let header = value
            .hunk_header
            .as_ref()
            .and_then(|h| serde_json::from_str(h).ok());
        let stack_id = value
            .stack_id
            .as_ref()
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
            .map(StackId::from);
        Ok(HunkAssignment {
            id: value.id.map(|id| Uuid::parse_str(&id)).transpose()?,
            hunk_header: header,
            path: value.path,
            path_bytes: value.path_bytes.into(),
            stack_id,
            hunk_locks: None,
            line_nums_added: None,   // derived data (not persisted)
            line_nums_removed: None, // derived data (not persisted)
            diff: None,              // derived data (not persisted)
        })
    }
}

impl TryFrom<HunkAssignment> for but_db::HunkAssignment {
    type Error = anyhow::Error;
    fn try_from(value: HunkAssignment) -> Result<Self, Self::Error> {
        let header = value
            .hunk_header
            .map(|h| {
                serde_json::to_string(&h)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize hunk_header: {}", e))
            })
            .transpose()?;
        Ok(but_db::HunkAssignment {
            id: value.id.map(|id| id.to_string()),
            hunk_header: header,
            path: value.path,
            path_bytes: value.path_bytes.into(),
            stack_id: value.stack_id.map(|id| id.to_string()),
        })
    }
}

impl From<HunkAssignment> for but_core::DiffSpec {
    fn from(value: HunkAssignment) -> Self {
        let hunk_headers = if let Some(header) = value.hunk_header {
            vec![but_core::HunkHeader {
                old_start: header.old_start,
                old_lines: header.old_lines,
                new_start: header.new_start,
                new_lines: header.new_lines,
            }]
        } else {
            vec![]
        };
        but_core::DiffSpec {
            previous_path: None, // TODO
            path: value.path_bytes.clone(),
            hunk_headers,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// Indicates that the assignment request was rejected due to locking - the hunk depends on a commit in the stack it is currently in.
pub struct AssignmentRejection {
    /// The request that was rejected.
    request: HunkAssignmentRequest,
    /// The locks that caused the rejection.
    locks: Vec<HunkLock>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// A request to update a hunk assignment.
/// If a a file has multiple hunks, the UI client should send a list of assignment requests with the appropriate hunk headers.
pub struct HunkAssignmentRequest {
    /// The hunk that is being assigned. Together with path_bytes, this identifies the hunk.
    /// If the file is binary, or too large to load, this will be None and in this case the path name is the only identity.
    /// If the file has hunk headers, then header info MUST be provided.
    pub hunk_header: Option<HunkHeader>,
    /// The file path of the hunk in bytes.
    pub path_bytes: BString,
    /// The stack to which the hunk is assigned. If set to None, the hunk is set as "unassigned".
    /// If a stack id is set, it must be one of the applied stacks.
    pub stack_id: Option<StackId>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
/// Same as `but_core::ui::WorktreeChanges`, but with the addition of hunk assignments.
pub struct WorktreeChanges {
    #[serde(flatten)]
    pub worktree_changes: but_core::ui::WorktreeChanges,
    pub assignments: Vec<HunkAssignment>,
    pub assignments_error: Option<serde_error::Error>,
    pub dependencies: Option<HunkDependencies>,
    pub dependencies_error: Option<serde_error::Error>,
}

impl From<but_core::ui::WorktreeChanges> for WorktreeChanges {
    fn from(worktree_changes: but_core::ui::WorktreeChanges) -> Self {
        WorktreeChanges {
            worktree_changes,
            assignments: vec![],
            assignments_error: None,
            dependencies: None,
            dependencies_error: None,
        }
    }
}

impl From<but_core::WorktreeChanges> for WorktreeChanges {
    fn from(worktree_changes: but_core::WorktreeChanges) -> Self {
        let ui_changes: but_core::ui::WorktreeChanges = worktree_changes.into();
        ui_changes.into()
    }
}

impl HunkAssignmentRequest {
    pub fn matches_assignment(&self, assignment: &HunkAssignment) -> bool {
        self.path_bytes == assignment.path_bytes && self.hunk_header == assignment.hunk_header
    }
}

impl PartialEq for HunkAssignment {
    fn eq(&self, other: &Self) -> bool {
        self.hunk_header == other.hunk_header && self.path_bytes == other.path_bytes
    }
}

impl HunkAssignment {
    /// Whether there is overlap between the two hunks.
    fn intersects(&self, other: HunkAssignment) -> bool {
        if self == &other {
            return true;
        }
        if self.path_bytes != other.path_bytes {
            return false;
        }

        // If both assignments have no hunk headers, they represent whole-file changes
        // and therefore intersect (same file, whole content)
        if self.hunk_header.is_none() && other.hunk_header.is_none() {
            return true;
        }

        // If one has no hunk header, it represents the whole file,
        // so it intersects with any hunk in that same file
        if self.hunk_header.is_none() || other.hunk_header.is_none() {
            return true;
        }

        // Both have hunk headers - check if they're equal first
        if self.hunk_header == other.hunk_header {
            return true;
        }

        // Both have hunk headers - check if the ranges overlap
        if let (Some(header), Some(other_header)) = (self.hunk_header, other.hunk_header) {
            return header.old_range().intersects(other_header.old_range())
                && header.new_range().intersects(other_header.new_range());
        }

        false
    }
}

/// Sets the assignment for a hunk. It must be already present in the current assignments, errors out if it isn't.
/// If the stack is not in the list of applied stacks, it errors out.
/// Returns the updated assignments list.
///
/// Optionally takes pre-computed hunk dependencies. If not provided, they will
/// be computed.
///
/// The provided hunk dependnecies should be computed for all workspace changes.
pub fn assign(
    ctx: &mut Context,
    requests: Vec<HunkAssignmentRequest>,
    deps: Option<&HunkDependencies>,
) -> Result<Vec<AssignmentRejection>> {
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let applied_stacks = vb_state
        .list_stacks_in_workspace()?
        .iter()
        .map(|s| s.id)
        .collect::<Vec<_>>();

    let deps = if let Some(deps) = deps {
        deps
    } else {
        &hunk_dependencies_for_workspace_changes_by_worktree_dir(ctx, None)?
    };

    let repo = &*ctx.repo.get()?;
    let worktree_changes: Vec<but_core::TreeChange> =
        but_core::diff::worktree_changes(repo)?.changes;
    let mut worktree_assignments = vec![];
    for change in &worktree_changes {
        let diff = change.unified_patch(repo, ctx.settings().context_lines);
        worktree_assignments.extend(diff_to_assignments(
            diff.ok().flatten(),
            change.path.clone(),
        ));
    }

    // Reconcile worktree with the persisted assignments
    let db = &mut *ctx.db.get_mut()?;
    let persisted_assignments = state::assignments(db)?;
    let with_worktree = reconcile::assignments(
        &worktree_assignments,
        &persisted_assignments,
        &applied_stacks,
        MultipleOverlapping::SetMostLines,
        true,
    );

    // Reconcile with the requested changes
    let with_requests = reconcile::assignments(
        &with_worktree,
        &requests_to_assignments(requests.clone()),
        &applied_stacks,
        MultipleOverlapping::SetMostLines,
        true,
    );

    // Reconcile with hunk locks
    let lock_assignments = hunk_dependency_assignments(deps);
    let with_locks = reconcile::assignments(
        &with_requests,
        &lock_assignments,
        &applied_stacks,
        MultipleOverlapping::SetNone,
        false,
    );

    state::set_assignments(db, with_locks.clone())?;

    // Request where the stack_id is different from the outcome are considered rejections - this is due to locking
    // Collect all the rejected requests together with the locks that caused the rejection
    let mut rejections = vec![];
    for req in requests {
        let locks = with_locks
            .iter()
            .filter(|assignment| {
                req.matches_assignment(assignment) && req.stack_id != assignment.stack_id
            })
            .flat_map(|assignment| assignment.hunk_locks.clone().unwrap_or_default())
            .collect_vec();
        if !locks.is_empty() {
            rejections.push(AssignmentRejection {
                request: req.clone(),
                locks,
            });
        }
    }
    Ok(rejections)
}

/// Similar to the `reconcile_with_worktree_and_locks` function.
/// TODO: figure out a better name for this function
pub fn assignments_with_fallback(
    ctx: &mut Context,
    set_assignment_from_locks: bool,
    worktree_changes: Option<impl IntoIterator<Item = impl Into<but_core::TreeChange>>>,
    deps: Option<&HunkDependencies>,
) -> Result<(Vec<HunkAssignment>, Option<anyhow::Error>)> {
    let hunk_assignments = reconcile_worktree_changes_with_worktree_and_locks(
        ctx,
        set_assignment_from_locks,
        worktree_changes,
        deps,
    )?;
    Ok((hunk_assignments, None))
}

fn reconcile_worktree_changes_with_worktree_and_locks(
    ctx: &mut Context,
    set_assignment_from_locks: bool,
    worktree_changes: Option<impl IntoIterator<Item = impl Into<but_core::TreeChange>>>,
    deps: Option<&HunkDependencies>,
) -> Result<Vec<HunkAssignment>> {
    let repo = ctx.repo.get()?;
    let worktree_changes: Vec<but_core::TreeChange> = match worktree_changes {
        Some(wtc) => wtc.into_iter().map(Into::into).collect(),
        None => but_core::diff::worktree_changes(&repo)?.changes,
    };
    let deps = if let Some(deps) = deps {
        deps
    } else {
        &hunk_dependencies_for_workspace_changes_by_worktree_dir(
            ctx,
            Some(worktree_changes.clone()),
        )?
    };

    if worktree_changes.is_empty() {
        return Ok(vec![]);
    }
    let mut worktree_assignments = vec![];
    for change in &worktree_changes {
        let diff = change.unified_patch(&repo, ctx.settings().context_lines);
        worktree_assignments.extend(diff_to_assignments(
            diff.ok().flatten(),
            change.path.clone(),
        ));
    }
    drop(repo);
    let reconciled = reconcile_with_worktree_and_locks(
        ctx,
        set_assignment_from_locks,
        &worktree_assignments,
        deps,
    )?;

    let db = &mut *ctx.db.get_mut()?;
    state::set_assignments(db, reconciled.clone())?;
    Ok(reconciled)
}

/// Returns the current hunk assignments for the workspace.
///
/// Reconciles the current hunk assignments with the current worktree changes.
/// It takes  the current hunk assignments as well as the current worktree changes, producing a new set of hunk assignments.
///
/// Inside, fuzzy matching is performed, such that hunks that were previously assigned
/// and now in they have been modified in the worktree are still assigned to the same stack.
///
/// If a hunk was deleted in the worktree, it is removed from the assignments list.
/// If a completely new hunk is added it is accounted for with a HunkAssignment with stack_id None.
///
/// If a stack is no longer present in the workspace (either unapplied or deleted), any assignments to it are removed.
///
/// If a hunk has a dependency on a particular stack and it has been previously assigned to another stack, the assignment is updated to reflect that dependency.
/// If a hunk has a dependency but it has not been previously assigned to any stack, it is left unassigned (stack_id is None). This is so that the hunk assignment workflow can remain optional.
///
/// This needs to be ran only after the worktree has changed.
///
/// If `worktree_changes` is `None`, they will be fetched automatically.
#[instrument(skip(ctx, worktree_assignments, deps), err(Debug))]
fn reconcile_with_worktree_and_locks(
    ctx: &mut Context,
    set_assignment_from_locks: bool,
    worktree_assignments: &[HunkAssignment],
    deps: &HunkDependencies,
) -> Result<Vec<HunkAssignment>> {
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let applied_stacks = vb_state
        .list_stacks_in_workspace()?
        .iter()
        .map(|s| s.id)
        .collect::<Vec<_>>();

    let db = &mut *ctx.db.get_mut()?;
    let persisted_assignments = state::assignments(db)?;
    let with_worktree = reconcile::assignments(
        worktree_assignments,
        &persisted_assignments,
        &applied_stacks,
        MultipleOverlapping::SetMostLines,
        true,
    );

    let lock_assignments = hunk_dependency_assignments(deps);
    let with_locks = reconcile::assignments(
        &with_worktree,
        &lock_assignments,
        &applied_stacks,
        MultipleOverlapping::SetNone,
        set_assignment_from_locks,
    );

    Ok(with_locks)
}

fn hunk_dependency_assignments(deps: &HunkDependencies) -> Vec<HunkAssignment> {
    let mut assignments = vec![];
    for (path, hunk, locks) in &deps.diffs {
        // If there are locks towards more than one stack, this means double locking and the assignment None - the user can resolve this by partial committing.
        let locked_to_stack_ids_count = locks
            .iter()
            .map(|lock| lock.stack_id)
            .collect::<std::collections::HashSet<_>>()
            .len();
        let stack_id = if locked_to_stack_ids_count == 1 {
            Some(locks[0].stack_id)
        } else {
            None
        };
        let assignment = HunkAssignment {
            id: None,
            hunk_header: Some(hunk.into()),
            path: path.clone(),
            path_bytes: path.clone().into(),
            stack_id,
            hunk_locks: Some(locks.clone()),
            line_nums_added: None,   // derived data (not persisted)
            line_nums_removed: None, // derived data (not persisted)
            diff: None,              // derived data (not persisted)
        };
        assignments.push(assignment);
    }
    assignments
}

/// This also generates a UUID for the assignment
fn diff_to_assignments(diff: Option<UnifiedPatch>, path: BString) -> Vec<HunkAssignment> {
    let path_str = path.to_str_lossy();
    if let Some(diff) = diff {
        match diff {
            but_core::UnifiedPatch::Binary => vec![HunkAssignment {
                id: Some(Uuid::new_v4()),
                hunk_header: None,
                path: path_str.into(),
                path_bytes: path,
                stack_id: None,
                hunk_locks: None,
                line_nums_added: None,
                line_nums_removed: None,
                diff: None,
            }],
            but_core::UnifiedPatch::TooLarge { .. } => vec![HunkAssignment {
                id: Some(Uuid::new_v4()),
                hunk_header: None,
                path: path_str.into(),
                path_bytes: path,
                stack_id: None,
                hunk_locks: None,
                line_nums_added: None,
                line_nums_removed: None,
                diff: None,
            }],
            but_core::UnifiedPatch::Patch {
                hunks,
                is_result_of_binary_to_text_conversion,
                ..
            } => {
                // If there are no hunks, then the assignment is for the whole file
                if is_result_of_binary_to_text_conversion || hunks.is_empty() {
                    vec![HunkAssignment {
                        id: Some(Uuid::new_v4()),
                        hunk_header: None,
                        path: path_str.into(),
                        path_bytes: path,
                        stack_id: None,
                        hunk_locks: None,
                        line_nums_added: None,
                        line_nums_removed: None,
                        diff: None,
                    }]
                } else {
                    hunks
                        .iter()
                        .map(|hunk| {
                            let (line_nums_added_new, line_nums_removed_old) =
                                line_nums_from_hunk(&hunk.diff, hunk.old_start, hunk.new_start);
                            HunkAssignment {
                                id: Some(Uuid::new_v4()),
                                hunk_header: Some(hunk.into()),
                                path: path_str.clone().into(),
                                path_bytes: path.clone(),
                                stack_id: None,
                                hunk_locks: None,
                                line_nums_added: Some(line_nums_added_new),
                                line_nums_removed: Some(line_nums_removed_old),
                                diff: Some(hunk.diff.clone()),
                            }
                        })
                        .collect()
                }
            }
        }
    } else {
        vec![HunkAssignment {
            id: Some(Uuid::new_v4()),
            hunk_header: None,
            path: path_str.into(),
            path_bytes: path.clone(),
            stack_id: None,
            hunk_locks: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        }]
    }
}

/// Given a diff, it extracts the line numbers that were added and removed in the hunk (in a old and new format)
/// The line numbers are relative to the start of the hunk (old_start and new_start respectively). The start is inclusive.
fn line_nums_from_hunk(diff: &BString, old_start: u32, new_start: u32) -> (Vec<usize>, Vec<usize>) {
    let mut line_nums_removed_old = vec![];
    let mut line_nums_added_new = vec![];
    let mut old_line_num = old_start as usize;
    let mut new_line_num = new_start as usize;
    // Split the diff into lines
    let lines = diff.lines();
    for line in lines {
        let Some(first_char) = line.first() else {
            continue;
        };
        match *first_char {
            b'+' => {
                // Line added in new version
                line_nums_added_new.push(new_line_num);
                new_line_num += 1;
            }
            b'-' => {
                // Line removed from old version
                line_nums_removed_old.push(old_line_num);
                old_line_num += 1;
            }
            b' ' => {
                // Context line (unchanged)
                old_line_num += 1;
                new_line_num += 1;
            }
            b'@' => {
                // Header line, skip
                continue;
            }
            _ => {
                // Other lines (context or other), treat as context
                old_line_num += 1;
                new_line_num += 1;
            }
        }
    }
    (line_nums_added_new, line_nums_removed_old)
}

fn requests_to_assignments(request: Vec<HunkAssignmentRequest>) -> Vec<HunkAssignment> {
    let mut assignments = vec![];
    for req in request {
        let assignment = HunkAssignment {
            id: None,
            hunk_header: req.hunk_header,
            path: req.path_bytes.to_str_lossy().into(),
            path_bytes: req.path_bytes,
            stack_id: req.stack_id,
            hunk_locks: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        };
        assignments.push(assignment);
    }
    assignments
}

pub fn assignments_to_requests(assignments: Vec<HunkAssignment>) -> Vec<HunkAssignmentRequest> {
    assignments
        .into_iter()
        .map(|assignment| HunkAssignmentRequest {
            hunk_header: assignment.hunk_header,
            path_bytes: assignment.path_bytes,
            stack_id: assignment.stack_id,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use bstr::BString;
    use but_core::{HunkHeader, ref_metadata::StackId};

    use super::*;
    use crate::reconcile::MultipleOverlapping;

    impl HunkAssignment {
        pub fn new(
            path: &str,
            start: u32,
            end: u32,
            stack_id: Option<usize>,
            id: Option<usize>,
        ) -> HunkAssignment {
            HunkAssignment {
                id: id.map(id_seq),
                hunk_header: Some(HunkHeader {
                    old_start: start,
                    old_lines: end,
                    new_start: start,
                    new_lines: end,
                }),
                path: path.to_string(),
                path_bytes: BString::from(path),
                stack_id: stack_id.map(stack_id_seq),
                hunk_locks: None,
                line_nums_added: None,
                line_nums_removed: None,
                diff: None,
            }
        }
    }

    impl HunkAssignmentRequest {
        pub fn new(
            path: &str,
            start: u32,
            end: u32,
            stack_id: Option<usize>,
        ) -> HunkAssignmentRequest {
            HunkAssignmentRequest {
                hunk_header: Some(HunkHeader {
                    old_start: start,
                    old_lines: end,
                    new_start: start,
                    new_lines: end,
                }),
                path_bytes: BString::from(path),
                stack_id: stack_id.map(stack_id_seq),
            }
        }
    }

    fn stack_id_seq(num: usize) -> StackId {
        StackId::from(id_seq(num))
    }

    fn id_seq(num: usize) -> Uuid {
        assert!(num < 10);
        uuid::Uuid::parse_str(&format!("00000000-0000-0000-0000-00000000000{}", num % 10)).unwrap()
    }

    fn deep_eq(a: &HunkAssignment, b: &HunkAssignment) -> bool {
        a.id == b.id
            && a.hunk_header == b.hunk_header
            && a.path == b.path
            && a.path_bytes == b.path_bytes
            && a.stack_id == b.stack_id
            && a.hunk_locks == b.hunk_locks
    }
    fn assert_eq(a: Vec<HunkAssignment>, b: Vec<HunkAssignment>) {
        assert!(
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| deep_eq(x, y)),
            "HunkAssignment vectors are not deeply equal.\nLeft: {a:#?}\nRight: {b:#?}"
        );
    }

    #[test]
    fn test_intersection() {
        assert!(
            !HunkAssignment::new("foo.rs", 10, 5, None, None)
                .intersects(HunkAssignment::new("foo.rs", 16, 5, None, None))
        );
        assert!(
            !HunkAssignment::new("foo.rs", 10, 6, None, None) // Lines 10 to 15
                .intersects(HunkAssignment::new("foo.rs", 16, 5, None, None)) // Lines 16 to 20
        );
        assert!(
            HunkAssignment::new("foo.rs", 10, 7, None, None) // Lines 10 to 16
                .intersects(HunkAssignment::new("foo.rs", 16, 5, None, None)) // Lines 16 to 20
        );
    }

    #[test]
    fn test_reconcile_exact_match_and_no_intersection() {
        let previous_assignments = vec![HunkAssignment::new("foo.rs", 10, 5, Some(1), Some(1))];
        let worktree_assignments = vec![
            HunkAssignment::new("foo.rs", 10, 5, None, None),
            HunkAssignment::new("foo.rs", 20, 5, None, None),
        ];
        let applied_stacks = vec![stack_id_seq(1), stack_id_seq(2)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultipleOverlapping::SetMostLines,
            true,
        );
        assert_eq(
            result,
            vec![
                HunkAssignment::new("foo.rs", 10, 5, Some(1), Some(1)),
                HunkAssignment::new("foo.rs", 20, 5, None, None),
            ],
        );
    }

    #[test]
    fn test_reconcile_exact_match_unapplied_branch_unassigns() {
        let previous_assignments = vec![HunkAssignment::new("foo.rs", 10, 5, Some(1), Some(1))];
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 10, 5, None, Some(1))];
        let applied_stacks = vec![stack_id_seq(2)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultipleOverlapping::SetMostLines,
            true,
        );
        assert_eq(
            result,
            vec![HunkAssignment::new("foo.rs", 10, 5, None, Some(1))],
        );
    }

    #[test]
    fn test_reconcile_with_overlap_preserves_assignment() {
        let previous_assignments = vec![HunkAssignment::new("foo.rs", 10, 5, Some(1), Some(1))];
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 12, 7, None, Some(1))];
        let applied_stacks = vec![stack_id_seq(1)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultipleOverlapping::SetMostLines,
            true,
        );
        assert_eq(
            result,
            vec![HunkAssignment::new("foo.rs", 12, 7, Some(1), Some(1))],
        );
    }

    #[test]
    fn test_double_overlap_picks_the_bigger_previous_assignment() {
        let previous_assignments = vec![
            HunkAssignment::new("foo.rs", 1, 15, Some(1), Some(1)),
            HunkAssignment::new("foo.rs", 17, 20, Some(2), Some(2)),
        ];
        let applied_stacks = vec![stack_id_seq(1), stack_id_seq(2)];
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 5, 18, None, None)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultipleOverlapping::SetMostLines,
            true,
        );
        assert_eq(
            result,
            vec![HunkAssignment::new("foo.rs", 5, 18, Some(2), Some(2))],
        );
    }

    #[test]
    fn test_double_overlap_unassigns() {
        let previous_assignments = vec![
            HunkAssignment::new("foo.rs", 5, 15, Some(1), Some(1)),
            HunkAssignment::new("foo.rs", 17, 25, Some(2), Some(2)),
        ];
        let applied_stacks = vec![stack_id_seq(1), stack_id_seq(2)];
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 5, 18, None, None)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultipleOverlapping::SetNone,
            true,
        );
        assert_eq(
            result,
            vec![HunkAssignment::new("foo.rs", 5, 18, None, Some(2))],
        );
    }

    #[test]
    fn test_reconcile_not_updating_unassigned() {
        let previous_assignments = vec![HunkAssignment::new("foo.rs", 10, 15, Some(1), None)];
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 12, 17, Some(2), None)];
        let applied_stacks = vec![stack_id_seq(1)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultipleOverlapping::SetMostLines,
            false,
        );
        // TODO: This is actually broken
        assert_eq!(
            result,
            vec![HunkAssignment::new("foo.rs", 12, 17, Some(1), None)]
        );
    }

    #[test]
    fn test_hunk_assignment_partial_eq() {
        let hunk1 = HunkAssignment::new("foo.rs", 10, 15, Some(1), Some(3));
        let hunk2 = HunkAssignment::new("foo.rs", 10, 15, Some(2), Some(4));
        assert_eq!(hunk1, hunk2);
    }

    #[test]
    fn test_hunk_assignment_partial_eq_different_path() {
        let hunk1 = HunkAssignment::new("foo.rs", 10, 15, Some(1), None);
        let hunk2 = HunkAssignment::new("bar.rs", 10, 15, Some(2), None);
        assert_ne!(hunk1, hunk2);
    }

    #[test]
    fn test_intersects_binary_files() {
        // Test that binary files (no hunk headers) with same path intersect
        let binary1 = HunkAssignment {
            id: Some(id_seq(1)),
            hunk_header: None,
            path: "image.png".to_string(),
            path_bytes: BString::from("image.png"),
            stack_id: Some(stack_id_seq(1)),
            hunk_locks: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        };

        let binary2 = HunkAssignment {
            id: Some(id_seq(2)),
            hunk_header: None,
            path: "image.png".to_string(),
            path_bytes: BString::from("image.png"),
            stack_id: None,
            hunk_locks: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        };

        assert!(
            binary1.clone().intersects(binary2.clone()),
            "Binary files with same path should intersect"
        );
        assert!(
            binary2.intersects(binary1),
            "Intersection should be symmetric"
        );
    }

    #[test]
    fn test_intersects_mixed_hunk_headers() {
        // Test file with hunk header intersects with file without hunk header
        let text_with_hunk = HunkAssignment::new("file.txt", 10, 5, Some(1), Some(1));

        let whole_file = HunkAssignment {
            id: Some(id_seq(2)),
            hunk_header: None,
            path: "file.txt".to_string(),
            path_bytes: BString::from("file.txt"),
            stack_id: None,
            hunk_locks: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        };

        assert!(
            text_with_hunk.clone().intersects(whole_file.clone()),
            "Text file with hunk should intersect with whole-file assignment"
        );
        assert!(
            whole_file.intersects(text_with_hunk),
            "Whole-file assignment should intersect with any hunk in same file"
        );
    }

    #[test]
    fn test_intersects_different_paths_no_headers() {
        // Binary files with different paths should not intersect
        let binary1 = HunkAssignment {
            id: Some(id_seq(1)),
            hunk_header: None,
            path: "image1.png".to_string(),
            path_bytes: BString::from("image1.png"),
            stack_id: Some(stack_id_seq(1)),
            hunk_locks: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        };

        let binary2 = HunkAssignment {
            id: Some(id_seq(2)),
            hunk_header: None,
            path: "image2.png".to_string(),
            path_bytes: BString::from("image2.png"),
            stack_id: None,
            hunk_locks: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        };

        assert!(
            !binary1.intersects(binary2),
            "Binary files with different paths should not intersect"
        );
    }

    #[test]
    fn test_reconcile_with_binary_files() {
        // Test that reconciliation preserves stack assignments for binary files
        let applied_stacks = vec![stack_id_seq(1)];

        let previous_assignments = vec![
            // Binary file assigned to stack 1
            HunkAssignment {
                id: Some(id_seq(1)),
                hunk_header: None,
                path: "logo.png".to_string(),
                path_bytes: BString::from("logo.png"),
                stack_id: Some(stack_id_seq(1)),
                hunk_locks: None,
                line_nums_added: None,
                line_nums_removed: None,
                diff: None,
            },
            // Text file assigned to stack 1
            HunkAssignment::new("code.rs", 10, 5, Some(1), Some(2)),
        ];

        let worktree_assignments = vec![
            // Same binary file, initially unassigned
            HunkAssignment {
                id: Some(id_seq(3)),
                hunk_header: None,
                path: "logo.png".to_string(),
                path_bytes: BString::from("logo.png"),
                stack_id: None,
                hunk_locks: None,
                line_nums_added: None,
                line_nums_removed: None,
                diff: None,
            },
            // Same text file, modified
            HunkAssignment::new("code.rs", 10, 7, None, Some(4)),
        ];

        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultipleOverlapping::SetMostLines,
            true,
        );

        // Binary file should maintain its stack assignment
        assert_eq!(
            result[0].stack_id,
            Some(stack_id_seq(1)),
            "Binary file should maintain stack assignment"
        );

        // Text file should also maintain its stack assignment
        assert_eq!(
            result[1].stack_id,
            Some(stack_id_seq(1)),
            "Text file should maintain stack assignment"
        );
    }

    #[test]
    fn test_reconcile_file_type_change() {
        // Test file changing from text to binary maintains assignment
        let applied_stacks = vec![stack_id_seq(1)];

        // Previously was a text file with hunks
        let previous_assignments = vec![HunkAssignment::new("data.file", 1, 10, Some(1), Some(1))];

        // Now it's a binary file (no hunk header)
        let worktree_assignments = vec![HunkAssignment {
            id: Some(id_seq(2)),
            hunk_header: None,
            path: "data.file".to_string(),
            path_bytes: BString::from("data.file"),
            stack_id: None,
            hunk_locks: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        }];

        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &applied_stacks,
            MultipleOverlapping::SetMostLines,
            true,
        );

        assert_eq!(
            result[0].stack_id,
            Some(stack_id_seq(1)),
            "File should maintain stack assignment when changing from text to binary"
        );
    }
}
