use clap::Parser;
use colored::Colorize;

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
}

fn main() {
    // setup project repository and gb_repository
    let path = "/Users/scottchacon/projects/gitbutler-client";
    let local_data_dir = "/Users/scottchacon/Library/Application Support/com.gitbutler.app.dev";

    let butler = ButlerCli::from(path, local_data_dir);

    let args = Cli::parse();
    match args.command.as_str() {
        "status" => {
            run_status(butler);
        },
        _ => println!("Unknown command: {}", args.command)
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
    println!("{}", "gb repo:".to_string().red());
    if let Ok(Some(session)) = butler.gb_repository.get_current_session() {
        let session_reader = sessions::Reader::open(&butler.gb_repository, &session).unwrap();

        println!("{}", "virtual branches:".to_string().red());
        let mut iter = virtual_branches::Iterator::new(&session_reader).unwrap();
        while let Some(item) = iter.next() {
            if let Ok(item) = item {
                println!("{:?}", item);
            }
        }
    }
}