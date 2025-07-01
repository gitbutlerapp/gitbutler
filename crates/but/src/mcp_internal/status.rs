use std::path::Path;

use but_core::{TreeChange, UnifiedDiff};
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RichHunk {
    /// The diff string.
    pub diff: String,
    /// The stack ID this hunk is assigned to, if any.
    pub assigned_to_stack: Option<but_workspace::StackId>,
    /// The locks this hunk has, if any.
    pub dependency_locks: Vec<but_hunk_dependency::ui::HunkLock>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    /// The path of the file that has changed.
    pub path: String,
    /// The file change status
    pub status: String,
    /// The hunk changes in the file.
    pub hunks: Vec<RichHunk>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    /// List of stacks applied to the project's workspace
    stacks: Vec<but_workspace::ui::StackEntry>,
    /// Unified diff changes that could be committed.
    file_changes: Vec<FileChange>,
}

pub fn project_status(project_dir: &Path) -> anyhow::Result<ProjectStatus> {
    let repo = crate::mcp_internal::project::project_repo(project_dir)?;

    let worktree = but_core::diff::worktree_changes(&repo)?;
    let diff = unified_diff_for_changes(
        &repo,
        worktree.changes.clone(),
        crate::mcp_internal::UI_CONTEXT_LINES,
    )?;

    let stacks = list_applied_stacks(project_dir)?;
    let assignments = hunk_assignments(project_dir)?;

    let serializable = ProjectStatus {
        stacks,
        file_changes: get_file_changes(&diff, assignments),
    };
    Ok(serializable)
}

fn get_file_changes(
    changes: &[(TreeChange, UnifiedDiff)],
    assingments: Vec<but_hunk_assignment::HunkAssignment>,
) -> Vec<FileChange> {
    let mut file_changes = vec![];
    for (change, unified_diff) in changes.iter() {
        match unified_diff {
            but_core::UnifiedDiff::Patch { hunks, .. } => {
                let path = change.path.to_string();
                let status = match &change.status {
                    but_core::TreeStatus::Addition { .. } => "added".to_string(),
                    but_core::TreeStatus::Deletion { .. } => "deleted".to_string(),
                    but_core::TreeStatus::Modification { .. } => "modified".to_string(),
                    but_core::TreeStatus::Rename { previous_path, .. } => {
                        format!("renamed from {}", previous_path)
                    }
                };

                let hunks = hunks
                    .iter()
                    .map(|hunk| {
                        let diff = hunk.diff.to_string();
                        let assignment = assingments
                            .iter()
                            .find(|a| {
                                a.path_bytes == change.path && a.hunk_header == Some(hunk.into())
                            })
                            .map(|a| (a.stack_id, a.hunk_locks.clone()));

                        let (assigned_to_stack, dependency_locks) =
                            if let Some((stack_id, locks)) = assignment {
                                let locks = locks.unwrap_or_default();
                                (stack_id, locks)
                            } else {
                                (None, vec![])
                            };

                        RichHunk {
                            diff,
                            assigned_to_stack,
                            dependency_locks,
                        }
                    })
                    .collect::<Vec<_>>();

                file_changes.push(FileChange {
                    path,
                    status,
                    hunks,
                });
            }
            _ => continue,
        }
    }

    file_changes
}

fn list_applied_stacks(current_dir: &Path) -> anyhow::Result<Vec<but_workspace::ui::StackEntry>> {
    let project = crate::mcp_internal::project::project_from_path(current_dir)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    let repo = ctx.gix_repo_for_merging_non_persisting()?;
    let meta = crate::mcp_internal::project::ref_metadata_toml(ctx.project())?;
    but_workspace::stacks_v3(&repo, &meta, but_workspace::StacksFilter::InWorkspace)
}

fn unified_diff_for_changes(
    repo: &gix::Repository,
    changes: Vec<but_core::TreeChange>,
    context_lines: u32,
) -> anyhow::Result<Vec<(but_core::TreeChange, but_core::UnifiedDiff)>> {
    changes
        .into_iter()
        .map(|tree_change| {
            tree_change
                .unified_diff(repo, context_lines)
                .map(|diff| (tree_change, diff.expect("no submodule")))
        })
        .collect::<Result<Vec<_>, _>>()
}

pub fn hunk_assignments(
    current_dir: &Path,
) -> anyhow::Result<Vec<but_hunk_assignment::HunkAssignment>> {
    let project = super::project::project_from_path(current_dir)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        ctx,
        false,
        None::<Vec<but_core::TreeChange>>,
        None,
    )?;

    Ok(assignments)
}
