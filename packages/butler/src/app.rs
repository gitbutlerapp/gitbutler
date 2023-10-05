use std::path;

use anyhow::{Context, Result};
use git2::Repository;

use gitbutler::{database, gb_repository, project_repository, projects, sessions, storage, users};

pub struct App {
    path: path::PathBuf,
    local_data_dir: path::PathBuf,
    project: projects::Project,
    gb_repository: gb_repository::Repository,
    sessions_db: sessions::Database,
    user: Option<users::User>,
}

impl App {
    pub fn new() -> Result<Self> {
        let path = find_git_directory().context("failed to find project directory")?;
        let local_data_dir = find_local_data_dir().context("could not find local data dir")?;

        let storage = storage::Storage::from(&local_data_dir);
        let users_storage = users::Storage::from(&storage);

        let projects_storage = projects::Storage::try_from(&storage)?;
        let projects = projects_storage
            .list_projects()
            .context("failed to list projects")?;

        let project = projects
            .into_iter()
            .find(|p| p.path == path)
            .context("failed to find project")?;

        let user = users_storage.get().context("failed to get user")?;
        let gb_repository =
            gb_repository::Repository::open(&local_data_dir, &project, user.as_ref())
                .context("failed to open repository")?;

        let db_path = std::path::Path::new(&local_data_dir).join("database.sqlite3");
        let database = database::Database::try_from(&db_path).context("failed to open database")?;
        let sessions_db = sessions::Database::from(database);

        Ok(Self {
            path,
            local_data_dir,
            project,
            gb_repository,
            sessions_db,
            user,
        })
    }

    pub fn user(&self) -> Option<&users::User> {
        self.user.as_ref()
    }

    pub fn path(&self) -> &path::PathBuf {
        &self.path
    }

    pub fn local_data_dir(&self) -> &path::PathBuf {
        &self.local_data_dir
    }

    pub fn project(&self) -> &projects::Project {
        &self.project
    }

    pub fn sessions_db(&self) -> &sessions::Database {
        &self.sessions_db
    }

    pub fn project_repository(&self) -> project_repository::Repository {
        project_repository::Repository::open(&self.project).unwrap()
    }

    pub fn gb_repository(&self) -> &gb_repository::Repository {
        &self.gb_repository
    }
}

fn find_git_directory() -> Option<path::PathBuf> {
    match Repository::discover("./") {
        Ok(repo) => repo.workdir().map(|p| p.to_path_buf()),
        Err(_) => None,
    }
}

fn find_local_data_dir() -> Option<path::PathBuf> {
    let mut path = dirs::config_dir().unwrap();
    path.push("com.gitbutler.app.dev");
    Some(path::PathBuf::from(path.to_string_lossy().to_string()))
}
