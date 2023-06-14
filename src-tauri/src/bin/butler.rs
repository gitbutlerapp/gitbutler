use clap::Parser;
use colored::Colorize;
use git2::Repository;
use dirs;
use dialoguer::{console::Term, theme::ColorfulTheme, Select};

use git_butler_tauri::writer::Writer;
use git_butler_tauri::{
    projects, storage, project_repository, gb_repository,
    users, database, sessions, virtual_branches, reader
};
use git_butler_tauri::reader::Reader;

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
        "info" => {
            run_info(butler);   // shows internal data states for the project
        },
        "status" => {
            run_status(butler); // shows virtual branch status
        },
        "flush" => {
            run_flush(butler); // manually flushes an active session
        },
        "setup" => {
            run_setup(butler);  // sets target sha from remote branch
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

            // TODO: if there are no virtual branches, calculate the sha as the merge-base between HEAD in project_repository and this target commit

            let target = virtual_branches::target::Target {
                name: branch.name().unwrap().unwrap().to_string(),
                remote: remote_url_str.to_string(),
                sha: branch.get().peel_to_commit().unwrap().id(),
            };

            let target_writer = virtual_branches::target::Writer::new(&butler.gb_repository);
            target_writer.write_default(&target).unwrap();

            // create default virtual branch if one doesnt exist
            // TODO: check if an active one exists (using the iterator?)
            // let branch_path = butler.gb_repository.branch_root();

            let branch_writer = butler.gb_repository.get_branch_writer().unwrap();
            let writer = virtual_branches::branch::Writer {
                repository: &butler.gb_repository,
                writer: branch_writer,
            };
            let branch = virtual_branches::branch::Branch {
                id: "default-branch".to_string(),
                name: "default branch".to_string(),
                applied: true,
                upstream: "".to_string(),
                created_timestamp_ms: 0,
                updated_timestamp_ms: 0,
                tree: branch.get().peel_to_commit().unwrap().id(),
            };
            writer.write(&branch).unwrap();
        }
        None => println!("User did not select anything")
    }
}

fn run_flush(butler: ButlerCli) {
    println!("{}", "flush session:".to_string().red());
    // for all active virtual branches, determine new base trees and write them, then clear the session deltas
}

// list the virtual branches and their file statuses (statusi?)
fn run_status(butler: ButlerCli) {
    println!("{}", "virtual branches:".to_string().red());
    if let Some(sha) = get_base_sha(&butler) {
        println!("  base sha: {}", sha.blue());

        let repo = butler.git_repository();
        let oid = git2::Oid::from_str(&sha).unwrap();
        let commit = repo.find_commit(oid).unwrap();
        let tree = commit.tree().unwrap();

        // list the files that are different between the wd and the base sha
        let mut opts = git2::DiffOptions::new();
        opts.recurse_untracked_dirs(true)
            .include_untracked(true)
            .show_untracked_content(true);
        let diff = repo.diff_tree_to_workdir(Some(&tree), Some(&mut opts)).unwrap();

        let deltas = diff.deltas();
        for delta in deltas {
            let old_file = delta.old_file();
            let new_file = delta.new_file();

            if let Some(path) = new_file.path() {
                println!("n:{:?}", path);
            } else if let Some(path) = old_file.path() {
                println!("o:{:?}", path);
            }
        }
    } else {
        println!("  no base sha set, run butler setup");
    }
}

// notes:
            //let head = self.git_repository.head()?;
            //let tree = head.peel_to_tree()?;

// just print information for the project
fn run_info(butler: ButlerCli) {
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
        let branch_reader = butler.gb_repository.get_branch_reader().unwrap();

        let target_reader = virtual_branches::target::Reader::new(&session_reader, &branch_reader);
        if let Ok(target) = target_reader.read_default() {
            println!("  name: {}", target.name.blue());
            println!("  sha: {}", target.sha.to_string().blue());
        }

        println!("{}", "virtual branches:".to_string().red());
        let mut iter = virtual_branches::Iterator::new(&session_reader).unwrap();
        while let Some(item) = iter.next() {
            if let Ok(item) = item {
                println!("{:?}", item);
            }
        }
    } else {
        println!("  no current session");
        if let Some(sha) = get_base_sha(&butler) {
            println!("  base sha: {}", sha.blue());
        }
    }
}

fn get_base_sha(butler: &ButlerCli) -> Option<String> {
    let branch_reader = butler.gb_repository.get_branch_reader().unwrap();
    if let Ok(reader::Content::UTF8(sha)) = branch_reader.read("branches/target/sha") {
        Some(sha)
    } else {
        None
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