use crate::{Context, LegacyProjectId};
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;

pub(crate) mod types {
    /// A UUID based project ID which is associated with metadata via `<app-dir>/projects.json`
    ///
    /// The goal is to bring this metadata into `<project-data-dir>/`, and use `ProjectHandle` in future
    /// which is self-describing and able to point to a path on disk while being URL safe.
    pub type LegacyProjectId = gitbutler_project::ProjectId;

    /// Project metadata and utilities to access it. Superseded by [`Context`].
    pub type LegacyProject = gitbutler_project::Project;
}

/// Legacy Lifecycle
impl Context {
    /// Create a context from a legacy `project_id`,
    /// which requires reading `projects.json` to map it to metadata.
    pub fn new_from_legacy_project_id(project_id: LegacyProjectId) -> anyhow::Result<Self> {
        let legacy_project = gitbutler_project::get(project_id)?;
        let repo = gix::open(legacy_project.git_dir())?;
        Ok(Context {
            settings: AppSettings::load_from_default_path_creating()?,
            legacy_project,
            repo,
        })
    }
}

/// Legacy - none of this should be kept.
impl Context {
    /// Return a context for calling into `gitbutler-` functions.
    pub fn legacy_ctx(&self) -> anyhow::Result<CommandContext> {
        CommandContext::open(&self.legacy_project, self.settings.clone())
    }

    /// Return a wrapper for metadata that only supports read-only access when presented with the project wide permission
    /// to read data.
    /// This is helping to prevent races with mutable instances.
    // TODO: remove _read_only as we don't need it anymore with a DB based implementation as long as the instances
    //       starts a transaction to isolate reads.
    //       For a correct implementation, this would also have to hold on to `_read_only`.
    pub fn legacy_meta(
        &self,
        _read_only: &but_core::sync::WorktreeReadPermission,
    ) -> anyhow::Result<but_meta::VirtualBranchesTomlMetadata> {
        but_meta::VirtualBranchesTomlMetadata::from_path(
            self.project_data_dir().join("virtual_branches.toml"),
        )
    }
}
