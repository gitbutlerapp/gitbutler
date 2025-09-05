use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use gitbutler_error::error;

use super::{storage, storage::UpdateRequest, Project, ProjectId};
use crate::{project::AddProjectOutcome, AuthKey};

#[derive(Clone, Debug)]
pub(crate) struct Controller {
    local_data_dir: PathBuf,
    projects_storage: storage::Storage,
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

    pub(crate) fn add<P: AsRef<Path>>(&self, path: P) -> Result<AddProjectOutcome> {
        let path = path.as_ref();
        let all_projects = self
            .projects_storage
            .list()
            .context("failed to list projects from storage")?;
        if let Some(existing_project) = all_projects.iter().find(|project| project.path == path) {
            return Ok(AddProjectOutcome::AlreadyExists(
                existing_project.to_owned(),
            ));
        }
        if !path.exists() {
            return Ok(AddProjectOutcome::PathNotFound);
        }
        if !path.is_dir() {
            return Ok(AddProjectOutcome::NotADirectory);
        }
        match gix::open_opts(path, gix::open::Options::isolated()) {
            Ok(repo) if repo.is_bare() => {
                return Ok(AddProjectOutcome::BareRepository);
            }
            Ok(repo) if repo.worktree().is_some_and(|wt| !wt.is_main()) => {
                if path.join(".git").is_file() {
                    return Ok(AddProjectOutcome::NonMainWorktree);
                };
            }
            Ok(repo) => match repo.workdir() {
                None => {
                    return Ok(AddProjectOutcome::NoWorkdir);
                }
                Some(wd) => {
                    if !wd.join(".git").is_dir() {
                        return Ok(AddProjectOutcome::NoDotGitDirectory);
                    }
                }
            },
            Err(err) => {
                return Ok(AddProjectOutcome::NotAGitRepository(err.to_string()));
            }
        }

        let id = uuid::Uuid::new_v4().to_string();

        // title is the base name of the file
        let title = path
            .iter()
            .next_back()
            .map_or_else(|| id.clone(), |p| p.to_str().unwrap().to_string());

        let project = Project {
            id: ProjectId::generate(),
            title,
            path: gix::path::realpath(path)?,
            api: None,
            ..Default::default()
        };

        self.projects_storage
            .add(&project)
            .context("failed to add project to storage")?;

        // Create a .git/gitbutler directory for app data
        if let Err(error) = std::fs::create_dir_all(project.gb_dir()) {
            tracing::error!(project_id = %project.id, ?error, "failed to create {:?} on project add", project.gb_dir());
        }

        Ok(AddProjectOutcome::Added(project))
    }

    pub(crate) fn update(&self, project: &UpdateRequest) -> Result<Project> {
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

        // FIXME(qix-): On windows, we have to force to system executable.
        // FIXME(qix-): This is a hack for now, and will be smoothed over in the future.
        #[cfg(windows)]
        let project_owned = {
            let mut project = project.clone();
            project.preferred_key = Some(AuthKey::SystemExecutable);
            project
        };

        #[cfg(windows)]
        let project = &project_owned;

        self.projects_storage.update(project)
    }

    pub(crate) fn get(&self, id: ProjectId) -> Result<Project> {
        self.get_inner(id, false)
    }

    /// Only get the project information. No state validation is done.
    /// This is intended to be used only when updating the path of a missing project.
    pub(crate) fn get_raw(&self, id: ProjectId) -> Result<Project> {
        #[cfg_attr(not(windows), allow(unused_mut))]
        let project = self.projects_storage.get(id)?;
        Ok(project)
    }

    /// Like [`Self::get()`], but will assure the project still exists and is valid by
    /// opening a git repository. This should only be done for critical points in time.
    pub(crate) fn get_validated(&self, id: ProjectId) -> Result<Project> {
        self.get_inner(id, true)
    }

    fn get_inner(&self, id: ProjectId, validate: bool) -> Result<Project> {
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut project = self.projects_storage.get(id)?;
        if validate {
            let worktree_dir = &project.path;
            if gix::open_opts(worktree_dir, gix::open::Options::isolated()).is_err() {
                let suffix = if !worktree_dir.exists() {
                    " as it does not exist"
                } else {
                    ""
                };
                return Err(anyhow!(
                    "Could not open repository at '{}'{suffix}",
                    worktree_dir.display()
                )
                .context(error::Code::ProjectMissing));
            }
        }

        if !project.gb_dir().exists() {
            if let Err(error) = std::fs::create_dir_all(project.gb_dir()) {
                tracing::error!(project_id = %project.id, ?error, "failed to create \"{}\" on project get", project.gb_dir().display());
            }
        }
        // Clean up old virtual_branches.toml that was never used
        let old_virtual_branches_path = project.path.join(".git").join("virtual_branches.toml");
        if old_virtual_branches_path.exists() {
            if let Err(error) = std::fs::remove_file(old_virtual_branches_path) {
                tracing::error!(project_id = %project.id, ?error, "failed to remove old virtual_branches.toml");
            }
        }

        // FIXME(qix-): On windows, we have to force to system executable
        #[cfg(windows)]
        {
            project.preferred_key = AuthKey::SystemExecutable;
        }

        Ok(project)
    }

    pub(crate) fn list(&self) -> Result<Vec<Project>> {
        self.projects_storage.list()
    }

    pub(crate) fn delete(&self, id: ProjectId) -> Result<()> {
        let Some(project) = self.projects_storage.try_get(id)? else {
            return Ok(());
        };

        self.projects_storage.purge(project.id)?;

        if let Err(error) = std::fs::remove_dir_all(self.project_metadata_dir(project.id)) {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::error!(project_id = %id, ?error, "failed to remove project data",);
            }
        }

        if project.gb_dir().exists() {
            if let Err(error) = std::fs::remove_dir_all(project.gb_dir()) {
                tracing::error!(project_id = %project.id, ?error, "failed to remove {:?} on project delete", project.gb_dir());
            }
        }

        Ok(())
    }

    fn project_metadata_dir(&self, id: ProjectId) -> PathBuf {
        self.local_data_dir.join("projects").join(id.to_string())
    }
}
