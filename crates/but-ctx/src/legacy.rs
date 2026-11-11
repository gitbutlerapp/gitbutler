use but_core::{RepositoryExt, sync::RepoExclusive};
use but_settings::AppSettings;
use tracing::instrument;

use crate::{
    CacheMode, Context, LegacyProjectId, ProjectHandleOrLegacyProjectId, RepoOpenMode,
    ThreadSafeContext, app_settings, new_ondemand_app_cache, new_ondemand_db,
    new_ondemand_git2_repo, new_ondemand_repo, open_repo,
};

pub(crate) mod types {
    /// A UUID based project ID which is associated with metadata via `<app-dir>/projects.json`
    ///
    /// The goal is to bring this metadata into `<project-data-dir>/`, and use `ProjectHandle` in future
    /// which is self-describing and able to point to a path on disk while being URL safe.
    pub type LegacyProjectId = but_project_handle::LegacyProjectId;

    /// Superseded by [`crate::Context`].
    pub type LegacyProject = gitbutler_project::Project;
}

impl Context {
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

    #[allow(
        deprecated,
        reason = "Context owns the deprecated boundary cache and must initialize it."
    )]
    pub fn new_from_legacy_project_and_settings_with_repo_open_mode(
        legacy_project: &gitbutler_project::Project,
        settings: AppSettings,
        repo_open_mode: RepoOpenMode,
    ) -> anyhow::Result<Self> {
        let gitdir = legacy_project.git_dir().to_owned();
        let repo = open_repo(&gitdir, repo_open_mode)?;
        let project_data_dir = repo.gitbutler_storage_path()?;
        let app_cache_dir = but_path::app_cache_dir().ok();
        let cache_mode = CacheMode::Disk;
        Context {
            settings,
            gitdir: gitdir.clone(),
            project_data_dir: project_data_dir.clone(),
            cache_mode,
            repo_open_mode,
            legacy_project: legacy_project.clone(),
            repo: new_ondemand_repo(gitdir.clone(), repo_open_mode),
            git2_repo: new_ondemand_git2_repo(gitdir),
            db: new_ondemand_db(project_data_dir),
            app_cache: new_ondemand_app_cache(app_cache_dir.clone(), cache_mode),
            app_cache_dir,
            workspace: Default::default(),
        }
        .with_repo(repo)
    }

    pub fn new_from_legacy_project(
        legacy_project: gitbutler_project::Project,
    ) -> anyhow::Result<Self> {
        Context::new_from_legacy_project_and_settings(
            &legacy_project,
            app_settings(but_path::app_config_dir()?)?,
        )
    }

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

impl Context {
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
    ) -> anyhow::Result<(impl but_core::RefMetadata + 'static, but_graph::Workspace)> {
        let ws = self.workspace_from_head()?;
        Ok((self.meta()?, ws))
    }

    pub fn legacy_meta(&self) -> anyhow::Result<but_meta::VirtualBranchesTomlMetadata> {
        but_meta::VirtualBranchesTomlMetadata::from_path(
            self.project_data_dir().join("virtual_branches.toml"),
        )
    }

    // TODO: remove _exclusive as we don't need it anymore with a DB based implementation as long as the instances
    //       starts a transaction to isolate reads.
    //       For a correct implementation, this would also have to hold on to `_exclusive`.
    pub fn legacy_meta_mut(
        &mut self,
        _exclusive: &RepoExclusive,
    ) -> anyhow::Result<but_meta::VirtualBranchesTomlMetadata> {
        but_meta::VirtualBranchesTomlMetadata::from_path(
            self.project_data_dir().join("virtual_branches.toml"),
        )
    }
}
