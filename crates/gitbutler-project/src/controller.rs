use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, anyhow, bail};
use but_error::Code;

use super::{Project, storage, storage::UpdateRequest};
use crate::{AuthKey, ProjectHandle, ProjectHandleOrLegacyProjectId, project::AddProjectOutcome};

#[derive(Clone, Debug)]
pub(crate) struct Controller {
    local_data_dir: PathBuf,
    projects_storage: storage::Storage,
}

struct ResolvedProjectRepo {
    repo: gix::Repository,
    worktree_dir: PathBuf,
}

#[expect(clippy::result_large_err)]
fn normalize_project_repo(
    repo: gix::Repository,
) -> std::result::Result<ResolvedProjectRepo, AddProjectOutcome> {
    if repo.is_bare() {
        return Err(AddProjectOutcome::BareRepository);
    }
    // Submodules also use a `.git` file in the worktree, so the repository identity must come
    // from the resolved gitdir instead of the worktree-local `.git` entry.
    if repo.worktree().is_some_and(|wt| !wt.is_main()) {
        return Err(AddProjectOutcome::NonMainWorktree);
    }

    let Some(worktree_dir) = repo.workdir().map(ToOwned::to_owned) else {
        return Err(AddProjectOutcome::NoWorkdir);
    };

    Ok(ResolvedProjectRepo { repo, worktree_dir })
}

#[expect(clippy::result_large_err)]
fn resolve_project_repo_exact(
    path: &Path,
) -> std::result::Result<ResolvedProjectRepo, AddProjectOutcome> {
    let repo = match gix::open_opts(path, gix::open::Options::isolated()) {
        Ok(repo) => repo,
        Err(err) => return Err(AddProjectOutcome::NotAGitRepository(err.to_string())),
    };

    normalize_project_repo(repo)
}

#[expect(clippy::result_large_err)]
fn resolve_project_repo_by_discovery(
    path: &Path,
) -> std::result::Result<ResolvedProjectRepo, AddProjectOutcome> {
    let repo = match gix::discover(path) {
        Ok(repo) => repo,
        Err(err) => return Err(AddProjectOutcome::NotAGitRepository(err.to_string())),
    };

    normalize_project_repo(repo)
}

fn find_existing_project_by_git_dir(
    all_projects: &[Project],
    git_dir: &Path,
) -> Result<Option<Project>> {
    for project in all_projects {
        let project = project
            .clone()
            .migrated()
            .unwrap_or_else(|_| project.clone());
        if project.git_dir_opt() == Some(git_dir) {
            return Ok(Some(project));
        }
    }
    Ok(None)
}

impl Controller {
    /// Assure we can list projects, and if not possibly existing projects files will be renamed, and an error is produced early.
    pub(crate) fn assure_app_can_startup_or_fix_it(
        &self,
        projects: Result<Vec<Project>>,
    ) -> Result<Vec<Project>> {
        match projects {
            Ok(works) => Ok(works),
            Err(probably_file_load_err) => {
                let projects_path = self.local_data_dir.join("projects.json");
                let max_attempts = 255;
                for round in 1..max_attempts {
                    let backup_path = self
                        .local_data_dir
                        .join(format!("projects.json.maybe-broken-{round:02}"));
                    if backup_path.is_file() {
                        continue;
                    }

                    if let Err(err) = std::fs::rename(&projects_path, &backup_path) {
                        tracing::error!(
                            "Failed to rename {} to {} - application may fail to startup: {err}",
                            projects_path.display(),
                            backup_path.display()
                        );
                    }

                    bail!(
                        "Could not open projects file at '{}'.\nIt was moved to {}.\nReopen or refresh the app to start fresh.\nError was: {probably_file_load_err}",
                        projects_path.display(),
                        backup_path.display()
                    );
                }
                bail!("There were already {max_attempts} backup project files - giving up")
            }
        }
    }
}

impl Controller {
    pub(crate) fn from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            projects_storage: storage::Storage::from_path(&path),
            local_data_dir: path,
        }
    }

    pub(crate) fn add_with_best_effort<P: AsRef<Path>>(
        &self,
        worktree_dir: P,
    ) -> Result<AddProjectOutcome> {
        let worktree_dir = worktree_dir.as_ref();
        if !worktree_dir.exists() {
            return Ok(AddProjectOutcome::PathNotFound);
        }
        if !worktree_dir.is_dir() {
            return Ok(AddProjectOutcome::NotADirectory);
        }

        let all_projects = self
            .projects_storage
            .list()
            .context("failed to list projects from storage")?;
        let resolved_path = gix::path::realpath(worktree_dir)?;
        let resolved_repo = match resolve_project_repo_by_discovery(&resolved_path) {
            Ok(repo) => repo,
            Err(outcome) => return Ok(outcome),
        };

        if let Some(existing_project) =
            find_existing_project_by_git_dir(&all_projects, resolved_repo.repo.git_dir())?
        {
            return Ok(AddProjectOutcome::AlreadyExists(existing_project));
        }

        self.add(resolved_repo.worktree_dir)
    }

    pub(crate) fn add(&self, worktree_dir: impl AsRef<Path>) -> Result<AddProjectOutcome> {
        let worktree_dir = worktree_dir.as_ref();
        if !worktree_dir.exists() {
            return Ok(AddProjectOutcome::PathNotFound);
        }
        if !worktree_dir.is_dir() {
            return Ok(AddProjectOutcome::NotADirectory);
        }
        let resolved_path = gix::path::realpath(worktree_dir)?;
        let resolved_repo = match resolve_project_repo_exact(&resolved_path) {
            Ok(repo) => repo,
            Err(outcome) => return Ok(outcome),
        };
        let all_projects = self
            .projects_storage
            .list()
            .context("failed to list projects from storage")?;
        if let Some(existing_project) =
            find_existing_project_by_git_dir(&all_projects, resolved_repo.repo.git_dir())?
        {
            return Ok(AddProjectOutcome::AlreadyExists(existing_project));
        }
        let repo = resolved_repo.repo;
        let id = ProjectHandleOrLegacyProjectId::ProjectHandle(ProjectHandle::from_path(
            repo.git_dir(),
        )?);

        let title = resolved_repo.worktree_dir.file_name().map_or_else(
            || resolved_repo.worktree_dir.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );

        let project = Project {
            title,
            // TODO(1.0): make this always `None`, until the field can be removed for good.
            worktree_dir: resolved_repo.worktree_dir,
            api: None,
            git_dir: repo.git_dir().to_owned(),
            ..Project::default_with_id(id)
        };

        self.projects_storage
            .add(&project)
            .context("failed to add project to storage")?;

        // Create the repository-local GitButler storage directory for app data.
        match project.gb_dir() {
            Ok(gb_dir) => {
                if let Err(error) = std::fs::create_dir_all(&gb_dir) {
                    tracing::error!(project_id = %project.id, ?error, "failed to create \"{}\" on project add", gb_dir.display());
                }
            }
            Err(error) => {
                tracing::error!(project_id = %project.id, ?error, "failed to resolve storage directory on project add");
            }
        }

        Ok(AddProjectOutcome::Added(project))
    }

    #[cfg_attr(not(windows), allow(unused_mut))]
    pub(crate) fn update(&self, mut project: UpdateRequest) -> Result<Project> {
        #[cfg(not(windows))]
        if let Some(AuthKey::Local {
            private_key_path, ..
        }) = &project.preferred_key
        {
            use resolve_path::PathResolveExt;
            let private_key_path = private_key_path.resolve();

            if !private_key_path.exists() {
                bail!(
                    "private key at \"{}\" not found",
                    private_key_path.display()
                );
            }

            if !private_key_path.is_file() {
                bail!(
                    "private key at \"{}\" is not a file",
                    private_key_path.display()
                );
            }
        }

        #[cfg(windows)]
        {
            project.preferred_key = Some(AuthKey::SystemExecutable);
        }

        self.projects_storage.update(project)
    }

    pub(crate) fn get(&self, id: ProjectHandleOrLegacyProjectId) -> Result<Project> {
        self.get_inner(id, false)
    }

    /// Only get the project information. No state validation is done.
    /// This is intended to be used only when updating the path of a missing project.
    pub(crate) fn get_raw(&self, id: ProjectHandleOrLegacyProjectId) -> Result<Project> {
        #[cfg_attr(not(windows), allow(unused_mut))]
        let project = self.projects_storage.get(id)?;
        Ok(project)
    }

    /// Like [`Self::get()`], but will assure the project still exists and is valid by
    /// opening a git repository. This should only be done for critical points in time.
    pub(crate) fn get_validated(&self, id: ProjectHandleOrLegacyProjectId) -> Result<Project> {
        self.get_inner(id, true)
    }

    fn get_inner(&self, id: ProjectHandleOrLegacyProjectId, validate: bool) -> Result<Project> {
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut project = self.projects_storage.get(id)?;
        // BACKWARD-COMPATIBLE MIGRATION
        project.migrate()?;
        if validate {
            let repo = project.open_isolated_repo();
            if repo.is_err() {
                let suffix = if !project.worktree_dir.exists() {
                    " as it does not exist"
                } else {
                    ""
                };
                return Err(anyhow!(
                    "Could not open repository at '{}'{suffix}",
                    project.worktree_dir.display()
                )
                .context(Code::ProjectMissing));
            }
        }

        match project.gb_dir() {
            Ok(gb_dir) => {
                if !gb_dir.exists()
                    && let Err(error) = std::fs::create_dir_all(&gb_dir)
                {
                    tracing::error!(project_id = %project.id, ?error, "failed to create \"{}\" on project get", gb_dir.display());
                }
            }
            Err(error) => {
                tracing::error!(project_id = %project.id, ?error, "failed to resolve storage directory on project get");
            }
        }
        // Clean up old virtual_branches.toml that was never used
        let old_virtual_branches_path = project.git_dir().join("virtual_branches.toml");
        if old_virtual_branches_path.exists()
            && let Err(error) = std::fs::remove_file(old_virtual_branches_path)
        {
            tracing::error!(project_id = %project.id, ?error, "failed to remove old virtual_branches.toml");
        }

        #[cfg(windows)]
        {
            project.preferred_key = AuthKey::SystemExecutable;
        }

        Ok(project)
    }

    pub(crate) fn list(&self) -> Result<Vec<Project>> {
        self.projects_storage.list()
    }

    pub(crate) fn delete(&self, id: ProjectHandleOrLegacyProjectId) -> Result<()> {
        let Some(project) = self.projects_storage.try_get(id.clone())? else {
            return Ok(());
        };

        let project_id = project.id.clone();
        self.projects_storage.purge(project_id.clone())?;

        if let Err(error) = std::fs::remove_dir_all(self.project_metadata_dir(project_id))
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::error!(project_id = %id, ?error, "failed to remove project data",);
        }

        match project.gb_dir() {
            Ok(gb_dir) => {
                if gb_dir.exists()
                    && let Err(error) = std::fs::remove_dir_all(&gb_dir)
                {
                    tracing::error!(project_id = %project.id, ?error, "failed to remove \"{}\" on project delete", gb_dir.display());
                }
            }
            Err(error) => {
                tracing::error!(project_id = %project.id, ?error, "failed to resolve storage directory on project delete");
            }
        }

        // Delete references in the gitbutler namespace
        if let Err(err) = project
            .open_isolated_repo()
            .and_then(|repo| delete_gitbutler_references(&repo))
        {
            tracing::error!(project_id = %project.id, ?err, "failed to delete gitbutler references");
        }

        Ok(())
    }

    fn project_metadata_dir(&self, id: ProjectHandleOrLegacyProjectId) -> PathBuf {
        self.local_data_dir.join("projects").join(id.to_string())
    }
}

fn delete_gitbutler_references(repo: &gix::Repository) -> Result<()> {
    let platform = repo.references()?;

    let safe = but_core::branch::SafeDelete::new(repo)?;
    for reference in platform
        .prefixed(b"refs/heads/gitbutler/")?
        .chain(platform.prefixed(b"refs/gitbutler/")?)
        .filter_map(Result::ok)
    {
        match safe.delete_reference(&reference) {
            Ok(out) => {
                if let Some(worktrees) = out.checked_out_in_worktree_dirs {
                    tracing::warn!(
                        ref_name = %reference.name().as_bstr(),
                        checked_out_in = ?worktrees,
                        "won't delete gitbutler reference as it is checked out"
                    );
                }
            }
            Err(err) => {
                tracing::warn!(
                    ref_name = %reference.name().as_bstr(),
                    ?err,
                    "failed to delete gitbutler reference"
                );
            }
        }
    }

    Ok(())
}
