use gitbutler_operating_modes::OperatingMode;
use gitbutler_project::ProjectId;

/// An event telling the receiver something about the state of the application which just changed.
#[derive(Debug, Clone)]
#[expect(missing_docs)]
pub enum Change {
    RefInfo {
        project_id: ProjectId,
        ref_info: but_workspace::ui::RefInfo,
    },
    GitFetch(ProjectId),
    GitHead {
        project_id: ProjectId,
        head: String,
        operating_mode: OperatingMode,
    },
    GitActivity {
        project_id: ProjectId,
        head_sha: String,
    },
    WorktreeChanges {
        project_id: ProjectId,
        changes: but_hunk_assignment::WorktreeChanges,
    },
}
