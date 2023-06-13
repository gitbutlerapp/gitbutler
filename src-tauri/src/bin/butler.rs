use clap::Parser;
use colored::Colorize;
use git2::Repository;
use dirs;
use dialoguer::{console::Term, theme::ColorfulTheme, Select};

use git_butler_tauri::{
    projects, storage, project_repository, gb_repository,
    users, database, sessions, virtual_branches
};

#[derive(Parser)]
struct Cli {
    command: String
}

struct ButlerCli {
    path: String,
    local_data_dir: String,
    project: projects::Project,
    gb_repository: gb_repository::Repository,
    sessions_db: sessions::Database,
}

impl ButlerCli {
    fn from(path: &str, local_data_dir: &str) -> Self {
        let storage = storage::Storage::from_path(local_data_dir);
        let users_storage = users::Storage::new(storage.clone());

        let projects_storage = projects::Storage::new(storage.clone());
        let projects = projects_storage.list_projects().unwrap();
        let project = projects.into_iter().find(|p| p.path == path).unwrap();

        let gb_repository = gb_repository::Repository::open(
            local_data_dir.to_string(),
            project.id.clone(),
            projects_storage,
            users_storage,
        )
        .expect("failed to open repository");

        let db_path = std::path::Path::new(&local_data_dir).join("database.sqlite3");
        let database = database::Database::open(&db_path).unwrap();
        let sessions_db = sessions::Database::new(database.clone());

        Self {
            path: path.to_string(),
            local_data_dir: local_data_dir.to_string(),
            project: project.clone(),
            gb_repository,
            sessions_db,
        }
    }

    fn project_repository(&self) -> project_repository::Repository {
        project_repository::Repository::open(&self.project).unwrap()
    }

    fn git_repository(&self) -> git2::Repository {
        git2::Repository::open(&self.path).unwrap()
    }
}

fn main() {
    // setup project repository and gb_repository
    let local_data_dir = find_local_data_dir().unwrap();
    let path = find_git_directory().unwrap();

    let butler = ButlerCli::from(&path, &local_data_dir);

    let args = Cli::parse();
    match args.command.as_str() {
        "status" => {
            run_status(butler);
        },
        "setup" => {
            run_setup(butler);
        },
        _ => println!("Unknown command: {}", args.command)
    }
}

fn run_setup(butler: ButlerCli) {
    println!("  HEAD: {}", butler.project_repository().get_head().unwrap().name().unwrap().blue());
    let repo = butler.git_repository();
    let items = butler.project_repository().git_remote_branches().unwrap();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(0)
        .interact_on_opt(&Term::stderr()).unwrap();

    match selection {
        Some(index) => {
            println!("Setting target to: {}", items[index].red());

            // lookup a branch by name
            let branch = repo.find_branch(&items[index], git2::BranchType::Remote).unwrap();

            let remote = repo.branch_remote_name(&branch.get().name().unwrap()).unwrap();
            let remote_url = repo.find_remote(remote.as_str().unwrap()).unwrap();
            let remote_url_str = remote_url.url().unwrap();
            println!("remote: {}", remote_url_str);

            let target = virtual_branches::target::Target {
                name: branch.name().unwrap().unwrap().to_string(),
                remote: remote_url_str.to_string(),
                sha: branch.get().peel_to_commit().unwrap().id(),
            };

            let target_writer = virtual_branches::target::Writer::new(&butler.gb_repository);
            target_writer.write_default(&target).unwrap();
        }
        None => println!("User did not select anything")
    }
}

// just print status information for the project
fn run_status(butler: ButlerCli) {
    println!("path: {}", butler.path.yellow());
    println!("data_dir: {}", butler.local_data_dir.yellow());

    // find the project in project storage that matches the cwd
    println!("{}", "project:".to_string().red());
    println!("  id: {}", butler.project.id.blue());
    println!("  title: {}", butler.project.title.blue());
    println!("  description: {}", butler.project.description.clone().unwrap_or("none".to_string()).blue());
    println!("  last_fetched_ts: {:?}", butler.project.last_fetched_ts);
    println!("  path: {}", butler.project.path.blue());

    let api = butler.project.api.as_ref().unwrap();
    println!("  {}:", "api".to_string().red());
    println!("   api name: {}", api.name.blue());
    println!("   api description: {}", api.description.clone().unwrap().blue());
    println!("   repo id: {}", api.repository_id.blue());
    println!("   git url: {}", api.git_url.blue());
    println!("   created: {}", api.created_at.blue());
    println!("   updated: {}", api.updated_at.blue());

    println!("{}", "project repo:".to_string().red());
    println!("  HEAD: {}", butler.project_repository().get_head().unwrap().name().unwrap().blue());

    println!("{}", "sessions:".to_string().red());
    // sessions storage
    let sessions = butler.sessions_db.list_by_project_id(&butler.project.id, butler.project.last_fetched_ts).unwrap();
    //list the sessions
    for session in &sessions {
        println!("  id: {}", session.id.blue());
    }

    // gitbutler repo stuff
    // read default target
    println!("{}", "default target:".to_string().red());
    if let Ok(Some(session)) = butler.gb_repository.get_current_session() {
        let session_reader = sessions::Reader::open(&butler.gb_repository, &session).unwrap();

        let branch_reader = virtual_branches::target::Reader::new(&session_reader);
        if let Ok(target) = branch_reader.read_default() {
            println!("  name: {}", target.name.blue());
            println!("  remote: {}", target.remote.blue());
            println!("  sha: {}", target.sha.to_string().blue());
        }

        println!("{}", "virtual branches:".to_string().red());
        let mut iter = virtual_branches::Iterator::new(&session_reader).unwrap();
        while let Some(item) = iter.next() {
            if let Ok(item) = item {
                println!("{:?}", item);
            }
        }
    }
}

fn find_git_directory() -> Option<String> {
    match Repository::discover("./") {
        Ok(repo) => {
            let mut path = repo.workdir().map(|path| path.to_string_lossy().to_string()).unwrap();
            path = path.trim_end_matches('/').to_string();
            Some(path)
        },
        Err(_) => None,
    }
}

fn find_local_data_dir() -> Option<String> {
    let mut path = dirs::config_dir().unwrap();
    path.push("com.gitbutler.app.dev");
    Some(path.to_string_lossy().to_string())
}