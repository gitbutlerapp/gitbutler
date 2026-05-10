//!
//!
//! Hunk - File, range
//!
//! HunkAssignment - None or Some(Stack in workspace)
//!
//! reconcile_assignments - takes worktree changes (`Vec<TreeChange>`) + current assignments (`Vec<HunkAssignment>`)
//! returns updated assignments (`Vec<HunkAssignment>`)
//!
//! set_assignments

mod reconcile;
mod state;

use std::collections::{BTreeMap, HashMap};

use anyhow::Result;
use bstr::{BString, ByteSlice};
use but_core::{DiffSpec, HunkHeader, TreeChange, UnifiedPatch, ref_metadata::StackId};
use but_db::{HunkAssignmentsHandle, HunkAssignmentsHandleMut};
use but_hunk_dependency::ui::HunkDependencies;
use gix::ObjectId;
use reconcile::MultipleOverlapping;
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
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
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_bytes")
    )]
    pub path_bytes: BString,
    /// The stack to which the hunk is assigned, derived from `branch_ref_bytes`
    /// through workspace projection.
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::stack_id_opt")
    )]
    pub stack_id: Option<StackId>,
    /// The assigned branch as a full ref name (e.g. `refs/heads/my-branch`).
    /// This is the source of truth for assignment targeting.
    #[serde(with = "but_serde::fullname_bytes_opt")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::fullname_bytes_opt")
    )]
    pub branch_ref_bytes: Option<gix::refs::FullName>,
    /// The line numbers that were added in this hunk.
    pub line_nums_added: Option<Vec<usize>>,
    /// The line numbers that were removed in this hunk.
    pub line_nums_removed: Option<Vec<usize>>,
    /// The hunk diff for internal usage. This is not to be persisted or sent over the API.
    #[serde(skip)]
    pub diff: Option<BString>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HunkAssignment);

impl HunkAssignment {
    pub fn from_tree_change(change: &TreeChange, patch: Option<UnifiedPatch>) -> Vec<Self> {
        diff_to_assignments(patch, change.path.clone())
    }
}

impl TryFrom<but_db::HunkAssignment> for HunkAssignment {
    type Error = anyhow::Error;
    fn try_from(value: but_db::HunkAssignment) -> Result<Self, Self::Error> {
        let header = value
            .hunk_header
            .as_ref()
            .and_then(|h| serde_json::from_str(h).ok());
        let legacy_stack_id = value
            .stack_id
            .as_ref()
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
            .map(StackId::from);
        Ok(HunkAssignment {
            id: value.id.map(|id| Uuid::parse_str(&id)).transpose()?,
            hunk_header: header,
            path: value.path,
            path_bytes: value.path_bytes.into(),
            stack_id: legacy_stack_id,
            branch_ref_bytes: value
                .branch_ref_bytes
                .map(|b| gix::refs::FullName::try_from(BString::from(b)))
                .transpose()
                .map_err(|e| anyhow::anyhow!("Failed to parse branch_ref_bytes: {e}"))?,
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
                    .map_err(|e| anyhow::anyhow!("Failed to serialize hunk_header: {e}"))
            })
            .transpose()?;
        Ok(but_db::HunkAssignment {
            id: value.id.map(|id| id.to_string()),
            hunk_header: header,
            path: value.path,
            path_bytes: value.path_bytes.into(),
            stack_id: None,
            branch_ref_bytes: value.branch_ref_bytes.map(|r| r.into_inner().into()),
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type",
    content = "subject"
)]
pub enum AbsorptionTarget {
    Branch {
        branch_name: String,
    },
    HunkAssignments {
        assignments: Vec<HunkAssignment>,
    },
    TreeChanges {
        changes: Vec<but_core::ui::TreeChange>,
        // Optionally, the stack to which the changes are assigned
        #[cfg_attr(feature = "export-schema", schemars(with = "Option<String>"))]
        assigned_stack_id: Option<StackId>,
    },
    #[default]
    All,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(AbsorptionTarget);

/// Reason why a file is being absorbed to a particular commit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AbsorptionReason {
    /// File has hunk range overlap with this commit
    HunkDependency,
    /// File is assigned to this stack and this is the topmost commit
    StackAssignment,
    /// Default to leftmost stack's topmost commit
    DefaultStack,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(AbsorptionReason);

impl AbsorptionReason {
    pub fn description(&self) -> &str {
        match self {
            AbsorptionReason::HunkDependency => "files locked to commit due to hunk range overlap",
            AbsorptionReason::StackAssignment => "last commit in the assigned stack",
            AbsorptionReason::DefaultStack => "last commit in the primary lane",
        }
    }
}

/// Information about a file being absorbed
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FileAbsorption {
    pub path: String,
    pub assignment: HunkAssignment,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(FileAbsorption);

/// Information about absorptions grouped by commit
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct CommitAbsorption {
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    pub stack_id: but_core::ref_metadata::StackId,
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    #[serde(with = "but_serde::object_id")]
    pub commit_id: gix::ObjectId,
    pub commit_summary: String,
    pub files: Vec<FileAbsorption>,
    pub reason: AbsorptionReason,
}

#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(CommitAbsorption);

/// JSON output structure for a file being absorbed
#[derive(Debug, Serialize)]
pub struct JsonFileAbsorption {
    pub path: String,
    pub hunks: Vec<String>,
}

/// JSON output structure for a commit absorption
#[derive(Debug, Serialize)]
pub struct JsonCommitAbsorption {
    pub commit_id: String,
    pub commit_summary: String,
    pub reason: AbsorptionReason,
    pub reason_description: String,
    pub files: Vec<JsonFileAbsorption>,
}

/// JSON output structure for the entire absorb operation
#[derive(Debug, Serialize)]
pub struct JsonAbsorbOutput {
    pub total_files: usize,
    pub commits: Vec<JsonCommitAbsorption>,
}

/// The target for a hunk assignment request.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type",
    content = "subject"
)]
pub enum HunkAssignmentTarget {
    /// Assign to the topmost branch of the given stack.
    Stack {
        #[serde(rename = "stackId")]
        #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
        stack_id: StackId,
    },
    /// Assign directly to the given branch ref bytes.
    Branch {
        #[serde(rename = "branchRefBytes")]
        #[cfg_attr(
            feature = "export-schema",
            schemars(schema_with = "but_schemars::bstring_bytes")
        )]
        branch_ref_bytes: BString,
    },
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HunkAssignmentTarget);

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
/// A request to update a hunk assignment.
/// If a file has multiple hunks, the UI client should send a list of assignment
/// requests with the appropriate hunk headers.
pub struct HunkAssignmentRequest {
    /// The hunk that is being assigned. Together with path_bytes, this identifies the hunk.
    /// If the file is binary, or too large to load, this will be None and in this case the path name is the only identity.
    /// If the file has hunk headers, then header info MUST be provided.
    pub hunk_header: Option<HunkHeader>,
    /// The file path of the hunk in bytes.
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_bytes")
    )]
    pub path_bytes: BString,
    /// Where to assign this hunk. `None` means "unassigned".
    pub target: Option<HunkAssignmentTarget>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(HunkAssignmentRequest);

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
/// Same as `but_core::ui::WorktreeChanges`, but with the addition of hunk assignments.
pub struct WorktreeChanges {
    #[serde(flatten)]
    pub worktree_changes: but_core::ui::WorktreeChanges,
    pub assignments: Vec<HunkAssignment>,
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::serde_error_opt")
    )]
    pub assignments_error: Option<serde_error::Error>,
    pub dependencies: Option<HunkDependencies>,
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::serde_error_opt")
    )]
    pub dependencies_error: Option<serde_error::Error>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(WorktreeChanges);

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

/// Applies assignment requests by reconciling them with the current worktree and persisted assignments.
/// Persists the updated assignments and returns `Ok(())` on success.
///
/// `context_lines` determines the amount of context lines in diffs, and it should match the UI.
pub fn assign(
    db: HunkAssignmentsHandleMut,
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
    requests: Vec<HunkAssignmentRequest>,
    context_lines: u32,
) -> Result<()> {
    let branches_by_stack = workspace_branches_by_stack(workspace);

    let worktree_changes: Vec<but_core::TreeChange> =
        but_core::diff::worktree_changes(repo)?.changes;
    let mut worktree_assignments = vec![];
    for change in &worktree_changes {
        let diff = change.unified_patch(repo, context_lines);
        worktree_assignments.extend(HunkAssignment::from_tree_change(
            change,
            diff.ok().flatten(),
        ));
    }

    // Reconcile worktree with the persisted assignments
    let mut persisted_assignments = state::assignments(db.to_ref())?;
    backfill_branch_ref_from_legacy_stack_id(&mut persisted_assignments, workspace);
    let with_worktree = reconcile::assignments(
        &worktree_assignments,
        &persisted_assignments,
        &branches_by_stack,
        MultipleOverlapping::SetMostLines,
        true,
    );

    // Reconcile with the requested changes
    let request_assignments = requests_to_assignments(requests, workspace)?;
    let mut with_requests = reconcile::assignments(
        &with_worktree,
        &request_assignments,
        &branches_by_stack,
        MultipleOverlapping::SetMostLines,
        true,
    );

    derive_stack_ids(&mut with_requests, workspace);
    state::set_assignments(db, with_requests)?;

    Ok(())
}

/// Reconcile persisted hunk assignments with the current worktree state.
///
/// `db` is the mutable hunk-assignment store that is read from and updated with
/// the reconciled assignments. `repo` is used to compute worktree changes and
/// render hunk patches when `worktree_changes` is not provided. `ws`
/// supplies the current stack and workspace projection used to derive stack IDs and
/// validate assignment ownership. `worktree_changes` can provide a caller-owned
/// worktree change list to re-use for performance, and when it is `None`,
/// changes are read from `repo`.
/// `context_lines` controls the amount of diff context used while converting
/// each worktree change into hunk assignments.
///
/// Returns `(assignments, fallback_error)`. `assignments` is the reconciled list
/// that was also persisted back to `db`. `fallback_error` is a warning channel
/// for callers: when present, assignment computation recovered by using a less
/// precise fallback and kept returning usable `assignments`, but the original
/// error is preserved so callers can log or surface degraded accuracy. It is
/// `None` when assignment reconciliation completed normally.
pub fn assignments_with_fallback(
    db: HunkAssignmentsHandleMut,
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
    worktree_changes: Option<impl IntoIterator<Item = impl Into<but_core::TreeChange>>>,
    context_lines: u32,
) -> Result<(Vec<HunkAssignment>, Option<anyhow::Error>)> {
    let hunk_assignments =
        reconcile_worktree_changes_with_worktree(db, repo, ws, worktree_changes, context_lines)?;
    Ok((hunk_assignments, None))
}

fn reconcile_worktree_changes_with_worktree(
    db: HunkAssignmentsHandleMut,
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
    worktree_changes: Option<impl IntoIterator<Item = impl Into<but_core::TreeChange>>>,
    context_lines: u32,
) -> Result<Vec<HunkAssignment>> {
    let worktree_changes: Vec<but_core::TreeChange> = match worktree_changes {
        Some(wtc) => wtc.into_iter().map(Into::into).collect(),
        None => but_core::diff::worktree_changes(repo)?.changes,
    };

    if worktree_changes.is_empty() {
        return Ok(vec![]);
    }
    let mut worktree_assignments = vec![];
    for change in &worktree_changes {
        let diff = change.unified_patch(repo, context_lines);
        worktree_assignments.extend(HunkAssignment::from_tree_change(
            change,
            diff.ok().flatten(),
        ));
    }
    let mut reconciled = reconcile_with_worktree(db.to_ref(), workspace, &worktree_assignments)?;

    derive_stack_ids(&mut reconciled, workspace);
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
/// This needs to be ran only after the worktree has changed.
#[instrument(skip(db, workspace, worktree_assignments), err(Debug))]
fn reconcile_with_worktree(
    db: HunkAssignmentsHandle,
    workspace: &but_graph::Workspace,
    worktree_assignments: &[HunkAssignment],
) -> Result<Vec<HunkAssignment>> {
    let branches_by_stack = workspace_branches_by_stack(workspace);

    let mut persisted_assignments = state::assignments(db)?;
    backfill_branch_ref_from_legacy_stack_id(&mut persisted_assignments, workspace);
    let with_worktree = reconcile::assignments(
        worktree_assignments,
        &persisted_assignments,
        &branches_by_stack,
        MultipleOverlapping::SetMostLines,
        true,
    );

    Ok(with_worktree)
}

/// Backfill legacy stack-backed rows to the topmost branch of that stack before
/// reconciliation runs.
fn backfill_branch_ref_from_legacy_stack_id(
    assignments: &mut [HunkAssignment],
    workspace: &but_graph::Workspace,
) {
    for assignment in assignments.iter_mut() {
        if assignment.branch_ref_bytes.is_none()
            && let Some(stack_id) = assignment.stack_id
        {
            assignment.branch_ref_bytes = workspace
                .find_stack_by_id(stack_id)
                .and_then(|stack| stack.ref_name())
                .map(|ref_name| ref_name.to_owned());
        }
    }
}

/// Collect the workspace branches keyed by stack for reconciliation validation.
fn workspace_branches_by_stack(
    workspace: &but_graph::Workspace,
) -> HashMap<StackId, Vec<gix::refs::FullName>> {
    let mut branches_by_stack = HashMap::new();
    for stack in &workspace.stacks {
        if let Some(id) = stack.id {
            let branch_refs: Vec<gix::refs::FullName> = stack
                .segments
                .iter()
                .filter_map(|s| s.ref_name().map(|r| r.to_owned()))
                .collect();
            branches_by_stack.insert(id, branch_refs);
        }
    }
    branches_by_stack
}

/// Derive `stack_id` from the assigned branch ref for API compatibility.
fn derive_stack_ids(assignments: &mut [HunkAssignment], workspace: &but_graph::Workspace) {
    for assignment in assignments.iter_mut() {
        assignment.stack_id = assignment.branch_ref_bytes.as_ref().and_then(|branch_ref| {
            workspace
                .find_segment_and_stack_by_refname(branch_ref.as_ref())
                .and_then(|(stack, _segment)| stack.id)
        });
    }
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
                branch_ref_bytes: None,
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
                branch_ref_bytes: None,
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
                        branch_ref_bytes: None,
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
                                branch_ref_bytes: None,
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
            branch_ref_bytes: None,
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

fn requests_to_assignments(
    request: Vec<HunkAssignmentRequest>,
    workspace: &but_graph::Workspace,
) -> Result<Vec<HunkAssignment>> {
    let mut assignments = vec![];
    for req in request {
        let HunkAssignmentRequest {
            hunk_header,
            path_bytes,
            target,
        } = req;
        let branch_ref_bytes = match target {
            None => None,
            Some(HunkAssignmentTarget::Branch { branch_ref_bytes }) => Some(
                gix::refs::FullName::try_from(branch_ref_bytes).map_err(|err| {
                    anyhow::anyhow!(
                        "Invalid branch_ref_bytes in assignment request for '{}': {err}",
                        path_bytes.to_str_lossy()
                    )
                })?,
            ),
            Some(HunkAssignmentTarget::Stack { stack_id }) => Some(
                workspace
                    .find_stack_by_id(stack_id)
                    .and_then(|stack| stack.ref_name())
                    .map(|ref_name| ref_name.to_owned())
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Unknown stack_id {stack_id} in assignment request for '{}'",
                            path_bytes.to_str_lossy()
                        )
                    })?,
            ),
        };
        let assignment = HunkAssignment {
            id: None,
            hunk_header,
            path: path_bytes.to_str_lossy().into(),
            path_bytes,
            stack_id: None,
            branch_ref_bytes,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        };
        assignments.push(assignment);
    }
    Ok(assignments)
}

pub fn assignments_to_requests(assignments: Vec<HunkAssignment>) -> Vec<HunkAssignmentRequest> {
    assignments
        .into_iter()
        .map(|assignment| {
            let target = assignment
                .branch_ref_bytes
                .map(|branch_ref_bytes| HunkAssignmentTarget::Branch {
                    branch_ref_bytes: BString::from(branch_ref_bytes.as_bstr()),
                })
                .or_else(|| {
                    assignment
                        .stack_id
                        .map(|stack_id| HunkAssignmentTarget::Stack { stack_id })
                });
            HunkAssignmentRequest {
                hunk_header: assignment.hunk_header,
                path_bytes: assignment.path_bytes,
                target,
            }
        })
        .collect()
}

/// Convert HunkAssignments to DiffSpecs
pub fn convert_assignments_to_diff_specs(
    assignments: &[HunkAssignment],
) -> anyhow::Result<Vec<DiffSpec>> {
    let mut specs_by_path: BTreeMap<BString, Vec<HunkAssignment>> = BTreeMap::new();

    // Group assignments by file path
    for assignment in assignments {
        specs_by_path
            .entry(assignment.path_bytes.clone())
            .or_default()
            .push(assignment.clone());
    }

    // Convert to DiffSpecs
    let mut diff_specs = Vec::new();
    for (path, hunks) in specs_by_path {
        let mut hunk_headers = Vec::new();
        for hunk in hunks {
            if let Some(header) = hunk.hunk_header {
                hunk_headers.push(header);
            }
        }

        diff_specs.push(DiffSpec {
            previous_path: None, // TODO: Handle renames
            path: path.clone(),
            hunk_headers,
        });
    }

    Ok(diff_specs)
}

/// Tracks mappings between old and new commit IDs during rebase operations
#[derive(Debug, Clone, Default)]
pub struct CommitMap {
    map: HashMap<ObjectId, ObjectId>,
}

impl CommitMap {
    /// Find the final mapped commit ID by following the chain of mappings
    pub fn find_mapped_id(&self, commit_id: ObjectId) -> ObjectId {
        let mut current_id = commit_id;
        while let Some(mapped_id) = self.map.get(&current_id) {
            current_id = *mapped_id;
        }
        current_id
    }

    /// Add a mapping from old commit ID to new commit ID
    pub fn add_mapping(&mut self, old_commit_id: ObjectId, new_commit_id: ObjectId) {
        self.map.insert(old_commit_id, new_commit_id);
    }
}

/// Type alias for grouped changes by commit
pub type GroupedChanges = BTreeMap<
    (but_core::ref_metadata::StackId, gix::ObjectId),
    (Vec<HunkAssignment>, AbsorptionReason),
>;

#[cfg(test)]
mod tests {
    use bstr::BString;
    use but_core::{HunkHeader, ref_metadata::StackId};
    use but_graph::{
        SegmentIndex, Workspace,
        workspace::{Stack, StackSegment, WorkspaceKind},
    };

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
                branch_ref_bytes: None,
                line_nums_added: None,
                line_nums_removed: None,
                diff: None,
            }
        }

        /// Like `new()`, but also sets `branch_ref_bytes`.
        pub fn with_branch_ref_bytes(mut self, branch_ref_bytes: Option<&str>) -> Self {
            self.branch_ref_bytes = branch_ref_bytes.map(|s| {
                gix::refs::FullName::try_from(s.to_string())
                    .expect("test branch ref should be valid")
            });
            self
        }
    }

    impl HunkAssignmentRequest {
        pub fn new(path: &str, start: u32, end: u32) -> HunkAssignmentRequest {
            HunkAssignmentRequest {
                hunk_header: Some(HunkHeader {
                    old_start: start,
                    old_lines: end,
                    new_start: start,
                    new_lines: end,
                }),
                path_bytes: BString::from(path),
                target: None,
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

    fn empty_workspace() -> Workspace {
        Workspace {
            graph: but_graph::Graph::default(),
            id: Default::default(),
            kind: WorkspaceKind::AdHoc,
            stacks: vec![],
            lower_bound: None,
            lower_bound_segment_id: None,
            target_ref: None,
            target_commit: None,
            metadata: None,
        }
    }

    fn branch_ref(name: &str) -> gix::refs::FullName {
        gix::refs::FullName::try_from(name.to_owned()).expect("test branch ref should be valid")
    }

    fn stack_segment(id: usize, branch_ref_name: Option<&str>) -> StackSegment {
        StackSegment {
            ref_info: branch_ref_name.map(|name| but_graph::RefInfo {
                ref_name: branch_ref(name),
                commit_id: None,
                worktree: None,
            }),
            remote_tracking_ref_name: None,
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            id: SegmentIndex::new(id),
            commits: vec![],
            commits_outside: None,
            base: None,
            base_segment_id: None,
            commits_by_segment: vec![],
            commits_on_remote: vec![],
            metadata: None,
            is_entrypoint: false,
        }
    }

    fn stack(id: Option<usize>, branch_ref_names: &[&str], segment_offset: usize) -> Stack {
        Stack {
            id: id.map(stack_id_seq),
            segments: branch_ref_names
                .iter()
                .enumerate()
                .map(|(idx, name)| stack_segment(segment_offset + idx, Some(name)))
                .collect(),
        }
    }

    fn workspace_with_stacks(stacks: Vec<Stack>) -> Workspace {
        Workspace {
            stacks,
            ..empty_workspace()
        }
    }

    fn deep_eq(a: &HunkAssignment, b: &HunkAssignment) -> bool {
        a.id == b.id
            && a.hunk_header == b.hunk_header
            && a.path == b.path
            && a.path_bytes == b.path_bytes
            && a.branch_ref_bytes == b.branch_ref_bytes
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
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &HashMap::new(),
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
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &HashMap::new(),
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
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &HashMap::new(),
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
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 5, 18, None, None)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &HashMap::new(),
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
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 5, 18, None, None)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &HashMap::new(),
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
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &HashMap::new(),
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
    fn test_requests_to_assignments_rejects_invalid_branch_target() {
        let err = requests_to_assignments(
            vec![HunkAssignmentRequest {
                hunk_header: None,
                path_bytes: BString::from("foo.rs"),
                target: Some(HunkAssignmentTarget::Branch {
                    branch_ref_bytes: BString::from("not a valid ref"),
                }),
            }],
            &empty_workspace(),
        )
        .expect_err("invalid branch targets should fail");

        assert!(
            err.to_string().contains("Invalid branch_ref_bytes"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn test_requests_to_assignments_rejects_unknown_stack_target() {
        let err = requests_to_assignments(
            vec![HunkAssignmentRequest {
                hunk_header: None,
                path_bytes: BString::from("foo.rs"),
                target: Some(HunkAssignmentTarget::Stack {
                    stack_id: stack_id_seq(1),
                }),
            }],
            &empty_workspace(),
        )
        .expect_err("unknown stack targets should fail");

        assert!(
            err.to_string().contains("Unknown stack_id"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn test_derive_stack_ids_replaces_stale_stack_id_from_branch_ref() {
        let workspace = workspace_with_stacks(vec![
            stack(Some(1), &["refs/heads/feature-a"], 0),
            stack(Some(2), &["refs/heads/feature-b"], 10),
        ]);
        let mut assignments = vec![
            HunkAssignment::new("foo.rs", 10, 5, Some(1), Some(1))
                .with_branch_ref_bytes(Some("refs/heads/feature-b")),
        ];

        derive_stack_ids(&mut assignments, &workspace);

        assert_eq!(assignments[0].stack_id, Some(stack_id_seq(2)));
    }

    #[test]
    fn test_derive_stack_ids_clears_stack_id_for_missing_branch_ref() {
        let workspace = workspace_with_stacks(vec![stack(Some(1), &["refs/heads/feature-a"], 0)]);
        let mut assignments = vec![
            HunkAssignment::new("foo.rs", 10, 5, Some(1), Some(1))
                .with_branch_ref_bytes(Some("refs/heads/missing")),
        ];

        derive_stack_ids(&mut assignments, &workspace);

        assert_eq!(assignments[0].stack_id, None);
    }

    #[test]
    fn test_derive_stack_ids_returns_none_when_matching_stack_has_no_id() {
        let workspace = workspace_with_stacks(vec![stack(None, &["refs/heads/feature-a"], 0)]);
        let mut assignments = vec![
            HunkAssignment::new("foo.rs", 10, 5, Some(1), Some(1))
                .with_branch_ref_bytes(Some("refs/heads/feature-a")),
        ];

        derive_stack_ids(&mut assignments, &workspace);

        assert_eq!(assignments[0].stack_id, None);
    }

    #[test]
    fn test_backfill_branch_ref_from_legacy_stack_id_uses_stack_tip_branch() {
        let workspace = workspace_with_stacks(vec![stack(
            Some(2),
            &["refs/heads/feature-tip", "refs/heads/feature-base"],
            0,
        )]);
        let mut assignments = vec![HunkAssignment::new("foo.rs", 10, 5, Some(2), Some(1))];

        backfill_branch_ref_from_legacy_stack_id(&mut assignments, &workspace);

        assert_eq!(
            assignments[0].branch_ref_bytes,
            Some(branch_ref("refs/heads/feature-tip"))
        );
    }

    #[test]
    fn test_backfill_branch_ref_from_legacy_stack_id_ignores_unknown_stack() {
        let workspace = workspace_with_stacks(vec![stack(Some(1), &["refs/heads/feature-a"], 0)]);
        let mut assignments = vec![HunkAssignment::new("foo.rs", 10, 5, Some(2), Some(1))];

        backfill_branch_ref_from_legacy_stack_id(&mut assignments, &workspace);

        assert_eq!(assignments[0].branch_ref_bytes, None);
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
            branch_ref_bytes: None,
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
            branch_ref_bytes: None,
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
            branch_ref_bytes: None,
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
            branch_ref_bytes: None,
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
            branch_ref_bytes: None,
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
        let previous_assignments = vec![
            // Binary file assigned to stack 1
            HunkAssignment {
                id: Some(id_seq(1)),
                hunk_header: None,
                path: "logo.png".to_string(),
                path_bytes: BString::from("logo.png"),
                stack_id: Some(stack_id_seq(1)),
                branch_ref_bytes: None,
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
                branch_ref_bytes: None,
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
            &HashMap::new(),
            MultipleOverlapping::SetMostLines,
            true,
        );

        assert_eq!(result.len(), 2, "Both files should be in the result");

        let binary_result = result.iter().find(|a| a.path == "logo.png").unwrap();
        assert!(
            binary_result.hunk_header.is_none(),
            "Binary file should remain without a hunk header"
        );

        let text_result = result.iter().find(|a| a.path == "code.rs").unwrap();
        assert!(
            text_result.hunk_header.is_some(),
            "Text file should retain its hunk header"
        );
    }

    #[test]
    fn test_reconcile_file_type_change() {
        // Test file changing from text to binary maintains assignment
        // Previously was a text file with hunks
        let previous_assignments = vec![HunkAssignment::new("data.file", 1, 10, Some(1), Some(1))];

        // Now it's a binary file (no hunk header)
        let worktree_assignments = vec![HunkAssignment {
            id: Some(id_seq(2)),
            hunk_header: None,
            path: "data.file".to_string(),
            path_bytes: BString::from("data.file"),
            stack_id: None,
            branch_ref_bytes: None,
            line_nums_added: None,
            line_nums_removed: None,
            diff: None,
        }];

        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &HashMap::new(),
            MultipleOverlapping::SetMostLines,
            true,
        );

        assert_eq!(result.len(), 1, "File should be in the result");
    }

    #[test]
    fn test_reconcile_preserves_branch_ref_bytes() {
        let feature_ref = gix::refs::FullName::try_from("refs/heads/feature".to_string()).unwrap();
        let branches = HashMap::from([(stack_id_seq(1), vec![feature_ref])]);
        let previous_assignments = vec![
            HunkAssignment::new("foo.rs", 10, 5, Some(1), Some(1))
                .with_branch_ref_bytes(Some("refs/heads/feature")),
        ];
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 10, 5, None, None)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &branches,
            MultipleOverlapping::SetMostLines,
            true,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].branch_ref_bytes.as_ref().map(|r| r.to_string()),
            Some("refs/heads/feature".to_string()),
            "branch_ref_bytes should be preserved through reconciliation"
        );
    }

    #[test]
    fn test_reconcile_clears_branch_ref_bytes_with_stack() {
        // When a stack is unapplied, both stack_id and branch_ref_bytes should be cleared.
        let previous_assignments = vec![
            HunkAssignment::new("foo.rs", 10, 5, Some(1), Some(1))
                .with_branch_ref_bytes(Some("refs/heads/feature")),
        ];
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 10, 5, None, None)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &HashMap::new(),
            MultipleOverlapping::SetMostLines,
            true,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].branch_ref_bytes, None,
            "branch_ref_bytes should be cleared when no matching branch in workspace"
        );
    }

    #[test]
    fn test_double_overlap_set_none_clears_branch_ref_bytes() {
        // When SetNone triggers due to conflicting stacks, branch_ref_bytes should also be cleared.
        let previous_assignments = vec![
            HunkAssignment::new("foo.rs", 5, 15, Some(1), Some(1))
                .with_branch_ref_bytes(Some("refs/heads/feature-a")),
            HunkAssignment::new("foo.rs", 17, 25, Some(2), Some(2))
                .with_branch_ref_bytes(Some("refs/heads/feature-b")),
        ];
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 5, 18, None, None)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &HashMap::new(),
            MultipleOverlapping::SetNone,
            true,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].branch_ref_bytes, None,
            "branch_ref_bytes should be None on conflicting multi-overlap"
        );
    }

    #[test]
    fn test_reconcile_branch_ref_bytes_not_updated_when_unassigned_and_flag_off() {
        // When update_unassigned is false, branch_ref_bytes should not be propagated to unassigned hunks.
        let previous_assignments = vec![
            HunkAssignment::new("foo.rs", 10, 15, Some(1), None)
                .with_branch_ref_bytes(Some("refs/heads/feature")),
        ];
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 12, 17, None, None)];
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &HashMap::new(),
            MultipleOverlapping::SetMostLines,
            false,
        );
        assert_eq!(result.len(), 1);
        // With update_unassigned=false, worktree hunk has no stack_id, so it should NOT adopt from previous
        assert_eq!(result[0].branch_ref_bytes, None);
    }

    #[test]
    fn test_reconcile_clears_stale_branch_ref_bytes() {
        // When a branch is deleted from the workspace but the stack remains,
        // branch_ref_bytes should be cleared while stack_id is preserved.
        let previous_assignments = vec![
            HunkAssignment::new("foo.rs", 10, 5, Some(1), Some(1))
                .with_branch_ref_bytes(Some("refs/heads/deleted-branch")),
        ];
        let worktree_assignments = vec![HunkAssignment::new("foo.rs", 10, 5, None, None)];
        // The stack exists but only has a different branch — "deleted-branch" is gone
        let other_ref =
            gix::refs::FullName::try_from("refs/heads/other-branch".to_string()).unwrap();
        let branches = HashMap::from([(stack_id_seq(1), vec![other_ref])]);
        let result = reconcile::assignments(
            &worktree_assignments,
            &previous_assignments,
            &branches,
            MultipleOverlapping::SetMostLines,
            true,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].branch_ref_bytes, None,
            "branch_ref_bytes should be cleared when branch is no longer in workspace"
        );
    }
}
