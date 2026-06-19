use but_core::{RefMetadata as _, RepositoryExt, sync::RepoExclusive};
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
        Ok(Context {
            settings,
            gitdir: gitdir.clone(),
            project_data_dir: project_data_dir.clone(),
            cache_mode,
            repo_open_mode,
            legacy_project: legacy_project.clone(),
            repo: new_ondemand_repo(gitdir.clone(), repo_open_mode),
            git2_repo: new_ondemand_git2_repo(gitdir.clone()),
            db: new_ondemand_db(project_data_dir.clone()),
            app_cache: new_ondemand_app_cache(app_cache_dir.clone(), cache_mode),
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
    ) -> anyhow::Result<(impl but_core::RefMetadata + 'static, but_graph::Workspace)> {
        let ws = self.workspace_from_head()?;
        Ok((self.meta()?, ws))
    }

    /// Make `target` the project's default target, persisting it both as project metadata
    /// in Git config and as the legacy `default_target`, which keeps fields that don't
    /// exist in project metadata, like the remote URL.
    pub fn set_default_target(
        &self,
        target: but_meta::virtual_branches_legacy_types::Target,
    ) -> anyhow::Result<()> {
        let project_meta = but_core::ref_metadata::ProjectMeta::try_from(&target)?;
        {
            let repo = self.repo.get()?;
            project_meta.persist_to_local_config(&repo)?;
        }
        self.legacy_meta()?.set_default_target(target)?;
        self.invalidate_workspace_cache()?;
        Ok(())
    }

    /// Re-port project metadata from the legacy `virtual_branches.toml` to Git config.
    ///
    /// Use this after the TOML was restored from a snapshot and is the source of truth,
    /// so ported repositories don't keep reading outdated values from Git config.
    ///
    /// Repositories that weren't ported yet are left alone: their TOML is still the only
    /// source of truth, and porting here would hide future TOML-only writes by older
    /// binaries behind the one-way ported marker.
    pub fn resync_project_meta_from_legacy(&self) -> anyhow::Result<()> {
        let repo = self.repo.get()?;
        if !but_core::ref_metadata::ProjectMeta::is_ported_repo(&repo)? {
            return Ok(());
        }
        let project_meta = self
            .meta_inner_read_only()?
            .workspace(but_core::WORKSPACE_REF_NAME.try_into()?)?
            .project_meta();
        let project_meta =
            but_core::ref_metadata::repair_target_metadata_for_migration(&project_meta, &repo);
        project_meta.persist_to_local_config(&repo)?;
        Ok(())
    }

    /// Return a wrapper for metadata that only supports read-only access when presented with the project wide permission
    /// to read data.
    /// This is helping to prevent races with mutable instances.
    // TODO: For a correct implementation, this would also have to hold on to `_read_only`.
    pub fn legacy_meta(&self) -> anyhow::Result<but_meta::VirtualBranchesTomlMetadata> {
        self.meta_inner_reconcile_on_drop()
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
        self.meta_inner_reconcile_on_drop()
    }

    fn meta_inner_reconcile_on_drop(
        &self,
    ) -> anyhow::Result<but_meta::VirtualBranchesTomlMetadata> {
        but_meta::VirtualBranchesTomlMetadata::from_path(
            self.project_data_dir().join("virtual_branches.toml"),
        )
    }
}
