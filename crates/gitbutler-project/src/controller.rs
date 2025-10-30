use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use but_core::{GitConfigSettings, RepositoryExt};
use gitbutler_error::error;

use super::{Project, ProjectId, storage, storage::UpdateRequest};
use crate::{AuthKey, project::AddProjectOutcome};

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

    pub(crate) fn add(&self, worktree_dir: impl AsRef<Path>) -> Result<AddProjectOutcome> {
        let worktree_dir = worktree_dir.as_ref();
        let all_projects = self
            .projects_storage
            .list()
            .context("failed to list projects from storage")?;
        if let Some(existing_project) = all_projects
            .iter()
            .find(|project| project.worktree_dir_but_should_use_git_dir() == worktree_dir)
        {
            return Ok(AddProjectOutcome::AlreadyExists(
                existing_project.to_owned(),
            ));
        }
        if !worktree_dir.exists() {
            return Ok(AddProjectOutcome::PathNotFound);
        }
        if !worktree_dir.is_dir() {
            return Ok(AddProjectOutcome::NotADirectory);
        }
        let resolved_path = gix::path::realpath(worktree_dir)?;
        // Make sure the repo is opened from the resolved path - it must be absolute for persistence.
        let repo = match gix::open_opts(&resolved_path, gix::open::Options::isolated()) {
            Ok(repo) if repo.is_bare() => {
                return Ok(AddProjectOutcome::BareRepository);
            }
            Ok(repo) if repo.worktree().is_some_and(|wt| !wt.is_main()) => {
                if worktree_dir.join(".git").is_file() {
                    return Ok(AddProjectOutcome::NonMainWorktree);
                };
                repo
            }
            Ok(repo) => match repo.workdir() {
                None => {
                    return Ok(AddProjectOutcome::NoWorkdir);
                }
                Some(wd) => {
                    if !wd.join(".git").is_dir() {
                        return Ok(AddProjectOutcome::NoDotGitDirectory);
                    }
                    repo
                }
            },
            Err(err) => {
                return Ok(AddProjectOutcome::NotAGitRepository(err.to_string()));
            }
        };

        let id = ProjectId::generate();

        // Resolve the path first to get the actual directory name
        let title_is_not_normal_component = worktree_dir
            .components()
            .next_back()
            .is_none_or(|c| !matches!(c, Component::Normal(_)));
        let path_for_title = if title_is_not_normal_component {
            &resolved_path
        } else {
            worktree_dir
        };

        let title = path_for_title.file_name().map_or_else(
            || id.to_string(),
            |name| name.to_string_lossy().into_owned(),
        );

        let project = Project {
            id,
            title,
            // TODO(1.0): make this always `None`, until the field can be removed for good.
            worktree_dir: resolved_path,
            api: None,
            git_dir: repo.git_dir().to_owned(),
            ..Default::default()
        };

        self.projects_storage
            .add(&project)
            .context("failed to add project to storage")?;

        // Create a .git/gitbutler directory for app data
        if let Err(error) = std::fs::create_dir_all(project.gb_dir()) {
            tracing::error!(project_id = %project.id, ?error, "failed to create {:?} on project add", project.gb_dir());
        }

        // Check if the remote is a Gerrit remote and set config accordingly
        if let Ok(true) = is_gerrit_remote(&repo) {
            let gerrit_config = GitConfigSettings {
                gitbutler_gerrit_mode: Some(true),
                ..Default::default()
            };
            repo.set_git_settings(&gerrit_config).ok();
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
        // BACKWARD-COMPATIBLE MIGRATION
        project.migrate()?;
        if validate {
            let repo = project.open_isolated();
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
                .context(error::Code::ProjectMissing));
            }
        }

        if !project.gb_dir().exists()
            && let Err(error) = std::fs::create_dir_all(project.gb_dir())
        {
            tracing::error!(project_id = %project.id, ?error, "failed to create \"{}\" on project get", project.gb_dir().display());
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

    pub(crate) fn delete(&self, id: ProjectId) -> Result<()> {
        let Some(project) = self.projects_storage.try_get(id)? else {
            return Ok(());
        };

        self.projects_storage.purge(project.id)?;

        if let Err(error) = std::fs::remove_dir_all(self.project_metadata_dir(project.id))
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::error!(project_id = %id, ?error, "failed to remove project data",);
        }

        if project.gb_dir().exists()
            && let Err(error) = std::fs::remove_dir_all(project.gb_dir())
        {
            tracing::error!(project_id = %project.id, ?error, "failed to remove {:?} on project delete", project.gb_dir());
        }

        // Delete references in the gitbutler namespace
        if let Err(err) = project
            .open_isolated()
            .and_then(|repo| delete_gitbutler_references(&repo))
        {
            tracing::error!(project_id = %project.id, ?err, "failed to delete gitbutler references");
        }

        Ok(())
    }

    fn project_metadata_dir(&self, id: ProjectId) -> PathBuf {
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

pub fn is_gerrit_remote(repo: &gix::Repository) -> anyhow::Result<bool> {
    use gix::{bstr::ByteSlice, remote::Direction};

    // Magic refspec that we use to determine if the remote is a Gerrit remote
    let gerrit_notes_ref = "refs/notes/review";

    let remote_name = repo
        .remote_default_name(Direction::Push)
        .ok_or_else(|| anyhow::anyhow!("No fetch remotes found"))?;

    let mut remote = repo.find_remote(remote_name.as_bstr())?;
    remote.replace_refspecs(vec![gerrit_notes_ref], Direction::Push)?;
    remote = remote.with_fetch_tags(gix::remote::fetch::Tags::None);

    let (map, _) = remote
        .connect(Direction::Push)?
        .ref_map(gix::progress::Discard, Default::default())?;

    Ok(!map.remote_refs.is_empty())
}
