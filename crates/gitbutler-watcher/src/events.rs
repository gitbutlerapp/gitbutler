use but_ctx::ProjectHandleOrLegacyProjectId;
use gitbutler_operating_modes::OperatingMode;

/// An event telling the receiver something about the state of the application which just changed.
#[derive(Debug, Clone)]
#[expect(missing_docs)]
pub enum Change {
    GitFetch(ProjectHandleOrLegacyProjectId),
    GitHead {
        project_id: ProjectHandleOrLegacyProjectId,
        head: String,
        operating_mode: OperatingMode,
    },
    GitActivity {
        project_id: ProjectHandleOrLegacyProjectId,
        head_sha: String,
    },
    WorktreeChanges {
        project_id: ProjectHandleOrLegacyProjectId,
        changes: but_hunk_assignment::WorktreeChanges,
    },
}
