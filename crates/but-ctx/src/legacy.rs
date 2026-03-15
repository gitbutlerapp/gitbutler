use but_core::{RepositoryExt, sync::RepoExclusive};
use but_settings::AppSettings;
use tracing::instrument;

use crate::{
    Context, LegacyProjectId, ProjectHandleOrLegacyProjectId, RepoOpenMode, ThreadSafeContext,
    app_settings, new_ondemand_app_cache, new_ondemand_cache, new_ondemand_db,
    new_ondemand_git2_repo, new_ondemand_repo, open_repo,
};

pub(crate) mod types {
    /// A UUID based project ID which is associated with metadata via `<app-dir>/projects.json`
    ///
    /// The goal is to bring this metadata into `<project-data-dir>/`, and use `ProjectHandle` in future
    /// which is self-describing and able to point to a path on disk while being URL safe.
    pub type LegacyProjectId = but_project_handle::LegacyProjectId;

    /// Project metadata and utilities to access it. Superseded by [`crate::Context`].
    pub type LegacyProject = gitbutler_project::Project;
}

/// Legacy Lifecycle
impl Context {
    /// Open the repository identified by `legacy_project` and `settings`.
    pub fn new_from_legacy_project_and_settings(
        legacy_project: &gitbutler_project::Project,
        settings: AppSettings,
    ) -> anyhow::Result<Self> {
        Self::new_from_legacy_project_and_settings_with_repo_open_mode(
            legacy_project,
            settings,
            RepoOpenMode::Standard,
        )
    }

    /// Open the repository identified by `legacy_project` and `settings`, while controlling
    /// how the repository sources configuration via `repo_open_mode`.
    pub fn new_from_legacy_project_and_settings_with_repo_open_mode(
        legacy_project: &gitbutler_project::Project,
        settings: AppSettings,
        repo_open_mode: RepoOpenMode,
    ) -> anyhow::Result<Self> {
        let gitdir = legacy_project.git_dir().to_owned();
        let repo = open_repo(&gitdir, repo_open_mode)?;
        let project_data_dir = repo.gitbutler_storage_path()?;
        let app_cache_dir = but_path::app_cache_dir().ok();
        Ok(Context {
            settings,
            gitdir: gitdir.clone(),
            project_data_dir: project_data_dir.clone(),
            repo_open_mode,
            legacy_project: legacy_project.clone(),
            repo: new_ondemand_repo(gitdir.clone(), repo_open_mode),
            git2_repo: new_ondemand_git2_repo(gitdir.clone()),
            db: new_ondemand_db(project_data_dir.clone()),
            cache: new_ondemand_cache(project_data_dir),
            app_cache: new_ondemand_app_cache(app_cache_dir.clone()),
            app_cache_dir,
            workspace: Default::default(),
        }
        .with_repo(repo))
    }

    /// Open the repository identified by `legacy_project` and `settings`.
    pub fn new_from_legacy_project(
        legacy_project: gitbutler_project::Project,
    ) -> anyhow::Result<Self> {
        Context::new_from_legacy_project_and_settings(
            &legacy_project,
            app_settings(but_path::app_config_dir()?)?,
        )
    }

    /// Create a context from a legacy `project_id`,
    /// which requires reading `projects.json` to map it to metadata.
    pub fn new_from_legacy_project_id(project_id: LegacyProjectId) -> anyhow::Result<Self> {
        let legacy_project =
            gitbutler_project::get(ProjectHandleOrLegacyProjectId::LegacyProjectId(project_id))?;
        Context::new_from_legacy_project_and_settings(
            &legacy_project,
            app_settings(but_path::app_config_dir()?)?,
        )
    }
}

impl TryFrom<LegacyProjectId> for Context {
    type Error = anyhow::Error;

    fn try_from(value: LegacyProjectId) -> Result<Self, Self::Error> {
        Context::new_from_legacy_project_id(value)
    }
}

impl TryFrom<LegacyProjectId> for ThreadSafeContext {
    type Error = anyhow::Error;

    fn try_from(value: LegacyProjectId) -> Result<Self, Self::Error> {
        let ctx: Context = value.try_into()?;
        Ok(ctx.into_sync())
    }
}

/// Legacy - none of this should be kept.
impl Context {
    /// Create a new workspace as seen from the current HEAD and return it,
    /// along with read-only metadata.
    ///
    /// The write-permission is required to obtain an exclusive metadata instance, which is needed
    /// to lock the workspace and its metadata for modification.
    #[deprecated = "Prefer Context::workspace_from_head_for_editing()"]
    #[instrument(
        name = "DEPRECATED: Context::workspace_and_meta_from_head",
        level = "debug",
        skip_all,
        err(Debug)
    )]
    pub fn workspace_and_meta_from_head(
        &self,
        _exclusive_access: &RepoExclusive,
    ) -> anyhow::Result<(
        impl but_core::RefMetadata + 'static,
        but_graph::projection::Workspace,
    )> {
        let ws = self.workspace_from_head()?;
        Ok((self.meta()?, ws))
    }

    /// Return a wrapper for metadata that only supports read-only access when presented with the project wide permission
    /// to read data.
    /// This is helping to prevent races with mutable instances.
    // TODO: For a correct implementation, this would also have to hold on to `_read_only`.
    pub fn legacy_meta(&self) -> anyhow::Result<but_meta::VirtualBranchesTomlMetadata> {
        self.meta_inner()
    }

    /// Return a wrapper for metadata for read and write access when presented with the project wide permission
    /// to write data.
    /// This is helping to prevent races with mutable instances.
    // TODO: remove _exclusive as we don't need it anymore with a DB based implementation as long as the instances
    //       starts a transaction to isolate reads.
    //       For a correct implementation, this would also have to hold on to `_exclusive`.
    pub fn legacy_meta_mut(
        &mut self,
        _exclusive: &RepoExclusive,
    ) -> anyhow::Result<but_meta::VirtualBranchesTomlMetadata> {
        self.meta_inner()
    }
}
