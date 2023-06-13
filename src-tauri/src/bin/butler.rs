use clap::Parser;
use colored::Colorize;

use git_butler_tauri::projects;
use git_butler_tauri::storage;
use git_butler_tauri::project_repository;
use git_butler_tauri::gb_repository;
use git_butler_tauri::users;
use git_butler_tauri::database;
use git_butler_tauri::sessions;
use git_butler_tauri::virtual_branches;

#[derive(Parser)]
struct Cli {
    command: String
}

fn main() {
    let args = Cli::parse();
    match args.command.as_str() {
        "status" => {
            println!("{}", "Butler Status".green());
        },
        _ => println!("Unknown command: {}", args.command)
    }

    // setup project repository and gb_repository
    let path = "/Users/scottchacon/projects/gitbutler-client";

    let local_data_dir = "/Users/scottchacon/Library/Application Support/com.gitbutler.app.dev";
    let storage = storage::Storage::from_path(local_data_dir);
    let projects_storage = projects::Storage::new(storage.clone());

    // find the project in project storage that matches the cwd
    let projects = projects_storage.list_projects().unwrap();
    let project = projects.into_iter().find(|p| p.path == path).unwrap();
    println!("{}", "project:".to_string().red());
    println!("  id: {}", project.id.blue());
    println!("  title: {}", project.title.blue());
    println!("  description: {}", project.description.clone().unwrap_or("none".to_string()).blue());
    println!("  last_fetched_ts: {:?}", project.last_fetched_ts);
    println!("  path: {}", project.path.blue());

    let api = project.api.as_ref().unwrap();
    println!("  {}:", "api".to_string().red());
    println!("   api name: {}", api.name.blue());
    println!("   api description: {}", api.description.clone().unwrap().blue());
    println!("   repo id: {}", api.repository_id.blue());
    println!("   git url: {}", api.git_url.blue());
    println!("   created: {}", api.created_at.blue());
    println!("   updated: {}", api.updated_at.blue());

    println!("{}", "project repo:".to_string().red());
    let project_repository = project_repository::Repository::open(&project).unwrap();
    println!("  HEAD: {}", project_repository.get_head().unwrap().name().unwrap().blue());

    let gb_repo = gb_repository::Repository::open(
        local_data_dir.to_string(),
        project.id.clone(),
        projects_storage,
        users::Storage::new(storage),
    )
    .expect("failed to open repository");


    println!("{}", "sessions:".to_string().red());
    // convert data_dir to a path

    let db_path = std::path::Path::new(&local_data_dir).join("database.sqlite3");
    let database = database::Database::open(&db_path).unwrap();

    // sessions storage
    let sessions_db = sessions::Database::new(database.clone());
    let sessions = sessions_db.list_by_project_id(&project.id, project.last_fetched_ts).unwrap();
    //list the sessions
    for session in &sessions {
        println!("  id: {}", session.id.blue());
    }

    // gitbutler repo stuff
    println!("{}", "gb repo:".to_string().red());
    if let Ok(Some(session)) = gb_repo.get_current_session() {
        let session_reader = sessions::Reader::open(&gb_repo, &session).unwrap();

        println!("{}", "virtual branches:".to_string().red());
        let mut iter = virtual_branches::Iterator::new(&session_reader).unwrap();
        while let Some(item) = iter.next() {
            if let Ok(item) = item {
                println!("{:?}", item);
            }
        }
    }
    

}