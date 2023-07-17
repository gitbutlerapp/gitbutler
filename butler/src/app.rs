use anyhow::{Context, Result};
use git2::Repository;

use git_butler_tauri::{
    database, gb_repository, project_repository, projects, sessions, storage, users,
};

pub struct App {
    path: String,
    local_data_dir: String,
    project: projects::Project,
    gb_repository: gb_repository::Repository,
    sessions_db: sessions::Database,
}

impl App {
    pub fn new() -> Result<Self> {
        let path = find_git_directory().context("failed to find project directory")?;
        let local_data_dir = find_local_data_dir().context("could not find local data dir")?;

        let storage = storage::Storage::from_path(local_data_dir.clone());
        let users_storage = users::Storage::new(storage.clone());

        let projects_storage = projects::Storage::new(storage);
        let projects = projects_storage
            .list_projects()
            .context("failed to list projects")?;

        let project = projects
            .into_iter()
            .find(|p| p.path == path)
            .context("failed to find project")?;

        let gb_repository = gb_repository::Repository::open(
            local_data_dir.clone(),
            project.id.clone(),
            projects_storage,
            users_storage,
        )
        .context("failed to open repository")?;

        let db_path = std::path::Path::new(&local_data_dir).join("database.sqlite3");
        let database = database::Database::open(db_path).context("failed to open database")?;
        let sessions_db = sessions::Database::new(database);

        Ok(Self {
            path,
            local_data_dir,
            project,
            gb_repository,
            sessions_db,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn local_data_dir(&self) -> &str {
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

fn find_git_directory() -> Option<String> {
    match Repository::discover("./") {
        Ok(repo) => {
            let mut path = repo
                .workdir()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap();
            path = path.trim_end_matches('/').to_string();
            Some(path)
        }
        Err(_) => None,
    }
}

fn find_local_data_dir() -> Option<String> {
    let mut path = dirs::config_dir().unwrap();
    path.push("com.gitbutler.app.dev");
    Some(path.to_string_lossy().to_string())
}
