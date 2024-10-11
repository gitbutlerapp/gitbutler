use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use gitbutler_error::error;

use super::{storage, storage::UpdateRequest, Project, ProjectId};
use crate::AuthKey;

#[derive(Clone)]
pub struct Controller {
    local_data_dir: PathBuf,
    projects_storage: storage::Storage,
}

impl Controller {
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            projects_storage: storage::Storage::from_path(&path),
            local_data_dir: path,
        }
    }

    pub fn add<P: AsRef<Path>>(&self, path: P) -> Result<Project> {
        let path = path.as_ref();
        let all_projects = self
            .projects_storage
            .list()
            .context("failed to list projects from storage")?;
        if all_projects.iter().any(|project| project.path == path) {
            bail!("project already exists");
        }
        if !path.exists() {
            bail!("path not found");
        }
        if !path.is_dir() {
            bail!("not a directory");
        }
        match gix::open_opts(path, gix::open::Options::isolated()) {
            Ok(repo) if repo.is_bare() => {
                bail!("bare repositories are unsupported");
            }
            Ok(repo) if repo.worktree().map_or(false, |wt| !wt.is_main()) => {
                if path.join(".git").is_file() {
                    bail!("can only work in main worktrees");
                };
            }
            Ok(repo) => {
                match repo.work_dir() {
                    None => bail!("Cannot add non-bare repositories without a workdir"),
                    Some(wd) => {
                        if !wd.join(".git").is_dir() {
                            bail!("A git-repository without a `.git` directory cannot currently be added");
                        }
                    }
                }
            }
            Err(err) => {
                return Err(anyhow::Error::from(err))
                    .context(error::Context::new("must be a Git repository"));
            }
        }

        let id = uuid::Uuid::new_v4().to_string();

        // title is the base name of the file
        let title = path
            .iter()
            .last()
            .map_or_else(|| id.clone(), |p| p.to_str().unwrap().to_string());

        let project = Project {
            id: ProjectId::generate(),
            title,
            path: path.to_path_buf(),
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

        Ok(project)
    }

    pub fn update(&self, project: &UpdateRequest) -> Result<Project> {
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

    pub fn get(&self, id: ProjectId) -> Result<Project> {
        self.get_inner(id, false)
    }

    /// Only get the project information. No state validation is done.
    /// This is intended to be used only when updating the path of a missing project.
    pub fn get_raw(&self, id: ProjectId) -> Result<Project> {
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut project = self.projects_storage.get(id)?;
        Ok(project)
    }

    /// Like [`Self::get()`], but will assure the project still exists and is valid by
    /// opening a git repository. This should only be done for critical points in time.
    pub fn get_validated(&self, id: ProjectId) -> Result<Project> {
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

    pub fn list(&self) -> Result<Vec<Project>> {
        self.projects_storage.list().map_err(Into::into)
    }

    pub fn delete(&self, id: ProjectId) -> Result<()> {
        let Some(project) = self.projects_storage.try_get(id)? else {
            return Ok(());
        };

        self.projects_storage
            .purge(project.id)
            .map_err(anyhow::Error::from)?;

        if let Err(error) = std::fs::remove_dir_all(self.project_metadata_dir(project.id)) {
            tracing::error!(project_id = %id, ?error, "failed to remove project data",);
        }

        if let Err(error) = std::fs::remove_file(project.path.join(".git/gitbutler.json")) {
            tracing::error!(project_id = %project.id, ?error, "failed to remove .git/gitbutler.json data",);
        }

        if project.gb_dir().exists() {
            if let Err(error) = std::fs::remove_dir_all(project.gb_dir()) {
                tracing::error!(project_id = %project.id, ?error, "failed to remove {:?} on project delete", project.gb_dir());
            }
        }

        Ok(())
    }

    pub fn project_metadata_dir(&self, id: ProjectId) -> PathBuf {
        self.local_data_dir.join("projects").join(id.to_string())
    }
}
