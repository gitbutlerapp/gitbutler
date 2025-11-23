use but_core::sync::{WorktreeReadPermission, WorktreeWritePermission};
use but_settings::AppSettings;

use crate::{Context, LegacyProjectId, new_ondemand_db, new_ondemand_git2_repo, new_ondemand_repo};

pub(crate) mod types {
    /// A UUID based project ID which is associated with metadata via `<app-dir>/projects.json`
    ///
    /// The goal is to bring this metadata into `<project-data-dir>/`, and use `ProjectHandle` in future
    /// which is self-describing and able to point to a path on disk while being URL safe.
    pub type LegacyProjectId = gitbutler_project::ProjectId;

    /// Project metadata and utilities to access it. Superseded by [`Context`].
    pub type LegacyProject = gitbutler_project::Project;
}

mod repository_ext;
pub use repository_ext::RepositoryExtLite;

/// Legacy Lifecycle
impl Context {
    /// Open the repository identified by `legacy_project` and `settings`.
    pub fn new_from_legacy_project_and_settings(
        legacy_project: &gitbutler_project::Project,
        settings: AppSettings,
    ) -> Self {
        let gitdir = legacy_project.git_dir().to_owned();
        Context {
            settings,
            gitdir: gitdir.clone(),
            legacy_project: legacy_project.clone(),
            repo: new_ondemand_repo(gitdir.clone()),
            git2_repo: new_ondemand_git2_repo(gitdir.clone()),
            db: new_ondemand_db(gitdir),
        }
    }

    /// Open the repository identified by `legacy_project` and `settings`.
    pub fn new_from_legacy_project(
        legacy_project: gitbutler_project::Project,
    ) -> anyhow::Result<Self> {
        let gitdir = legacy_project.git_dir().to_owned();
        Ok(Context {
            settings: AppSettings::load_from_default_path_creating()?,
            gitdir: gitdir.clone(),
            legacy_project,
            repo: new_ondemand_repo(gitdir.clone()),
            git2_repo: new_ondemand_git2_repo(gitdir.clone()),
            db: new_ondemand_db(gitdir),
        })
    }

    /// Create a context from a legacy `project_id`,
    /// which requires reading `projects.json` to map it to metadata.
    pub fn new_from_legacy_project_id(project_id: LegacyProjectId) -> anyhow::Result<Self> {
        let legacy_project = gitbutler_project::get(project_id)?;
        let gitdir = legacy_project.git_dir().to_owned();
        Ok(Context {
            settings: AppSettings::load_from_default_path_creating()?,
            gitdir: gitdir.clone(),
            legacy_project,
            repo: new_ondemand_repo(gitdir.clone()),
            git2_repo: new_ondemand_git2_repo(gitdir.clone()),
            db: new_ondemand_db(gitdir),
        })
    }
}

/// Trampolines that create new uncached instances of major types.
impl Context {
    /// Open the repository with standard options and create a new Graph traversal from the given `ref_name`,
    /// along with a new metadata instance, and the graph itself.
    ///
    /// The write-permission is required to obtain a mutable metadata instance. Note that it must be held
    /// for until the end of the operation for the protection to be effective.
    ///
    /// Use [`Self::graph_and_meta_and_repo_from_head()`] if control over the repository configuration is needed.
    pub fn graph_and_legacy_meta_mut_and_repo_from_reference(
        &self,
        ref_name: &gix::refs::FullNameRef,
        _write: &mut WorktreeWritePermission,
    ) -> anyhow::Result<(
        gix::Repository,
        // Specifically used for migrations, hence the original type for direct access.
        but_meta::VirtualBranchesTomlMetadata,
        but_graph::Graph,
    )> {
        let repo = self.repo.get()?;
        let meta = self.meta_inner()?;
        let mut reference = repo.find_reference(ref_name)?;
        let commit_id = reference.peel_to_commit()?.id();
        let graph = but_graph::Graph::from_commit_traversal(
            commit_id,
            reference.name().to_owned(),
            &meta,
            but_graph::init::Options::limited(),
        )?;
        Ok((repo.clone(), meta, graph))
    }

    /// Open the repository with standard options and create a new Graph traversal from the current HEAD,
    /// along with a new metadata instance, and the graph itself.
    ///
    /// The write-permission is required to obtain a mutable metadata instance. Note that it must be held
    /// for until the end of the operation for the protection to be effective.
    ///
    /// Use [`Self::graph_and_meta_and_repo_from_head()`] if control over the repository configuration is needed.
    // TODO: make this non-legacy once we don't need the legacy refmetadata implementation anymore.
    pub fn graph_and_meta_mut_and_repo_from_head(
        &self,
        _write: &mut WorktreeWritePermission,
    ) -> anyhow::Result<(
        gix::Repository,
        impl but_core::RefMetadata + 'static,
        but_graph::Graph,
    )> {
        let repo = self.repo.get()?;
        let meta = self.meta_inner()?;
        let graph = but_graph::Graph::from_head(&repo, &meta, but_graph::init::Options::limited())?;
        Ok((repo.clone(), meta, graph))
    }

    /// Create a new Graph traversal from the current HEAD, using (and returning) the given `repo` (configured by the caller),
    /// along with a new metadata instance, and the graph itself.
    ///
    /// The read-permission is required to obtain a shared metadata instance. Note that it must be held
    /// for until the end of the operation for the protection to be effective.
    // TODO: make this non-legacy once we don't need the legacy refmetadata implementation anymore.
    pub fn graph_and_meta_and_repo_from_head(
        &self,
        repo: gix::Repository,
        _read_only: &WorktreeReadPermission,
    ) -> anyhow::Result<(
        gix::Repository,
        impl but_core::RefMetadata + 'static,
        but_graph::Graph,
    )> {
        let meta = self.meta_inner()?;
        let graph = but_graph::Graph::from_head(&repo, &meta, but_graph::init::Options::limited())?;
        Ok((repo, meta, graph))
    }
}

/// Legacy - none of this should be kept.
impl Context {
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
        self.meta_inner()
    }

    fn meta_inner(&self) -> anyhow::Result<but_meta::VirtualBranchesTomlMetadata> {
        but_meta::VirtualBranchesTomlMetadata::from_path(
            self.project_data_dir().join("virtual_branches.toml"),
        )
    }
}
