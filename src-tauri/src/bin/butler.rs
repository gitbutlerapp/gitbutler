use clap::Parser;
use colored::Colorize;
use dialoguer::{console::Term, theme::ColorfulTheme, Input, MultiSelect, Select};

use git2::Repository;
use std::{collections::HashMap, time};
use uuid::Uuid;

use git_butler_tauri::{
    database, gb_repository, project_repository, projects, reader, sessions, storage, users,
    virtual_branches::{self, list_virtual_branches},
};

#[derive(Parser)]
struct Cli {
    command: String,
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

        let projects_storage = projects::Storage::new(storage);
        let projects = projects_storage.list_projects().unwrap();
        let project = projects.into_iter().find(|p| p.path == path).unwrap();

        let gb_repository = gb_repository::Repository::open(
            local_data_dir,
            project.id.clone(),
            projects_storage,
            users_storage,
        )
        .expect("failed to open repository");

        let db_path = std::path::Path::new(&local_data_dir).join("database.sqlite3");
        let database = database::Database::open(db_path).unwrap();
        let sessions_db = sessions::Database::new(database);

        Self {
            path: path.to_string(),
            local_data_dir: local_data_dir.to_string(),
            project,
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
        "info" => run_info(butler), // shows internal data states for the project
        "status" => run_status(butler), // shows virtual branch status
        "new" => run_new(butler),   // create new empty virtual branch
        "move" => run_move(butler), // move file ownership from one branch to another
        "setup" => run_setup(butler), // sets target sha from remote branch
        "commit" => run_commit(butler), // creates trees from the virtual branch content and creates a commit
        "branches" => run_branches(butler),
        "remotes" => run_remotes(butler),
        "flush" => run_flush(butler), // artificially forces a session flush
        _ => println!("Unknown command: {}", args.command),
    }
}

fn run_remotes(butler: ButlerCli) {
    let branches =
        virtual_branches::remote_branches(&butler.gb_repository, &butler.project_repository())
            .unwrap();
    for branch in branches {
        println!("{:?}", branch);
    }
}

fn run_flush(butler: ButlerCli) {
    println!("Flushing sessions");
    butler.gb_repository.flush().unwrap();
}

fn run_branches(butler: ButlerCli) {
    let branches = list_virtual_branches(&butler.gb_repository, &butler.project_repository())
        .expect("failed to list branches");
    for branch in branches {
        println!("{}", branch.id.red());
        println!("{}", branch.name.red());
        for file in branch.files {
            println!("  {}", file.path.blue());
            for hunk in file.hunks {
                println!("--");
                println!("    {}", hunk.diff.green());
                println!("--");
            }
        }
    }
}

fn run_commit(butler: ButlerCli) {
    // get the branch to commit
    let current_session = butler
        .gb_repository
        .get_or_create_current_session()
        .expect("failed to get or create currnt session");
    let current_session_reader = sessions::Reader::open(&butler.gb_repository, &current_session)
        .expect("failed to open current session reader");

    let virtual_branches = virtual_branches::Iterator::new(&current_session_reader)
        .expect("failed to read virtual branches")
        .collect::<Result<Vec<virtual_branches::branch::Branch>, reader::Error>>()
        .expect("failed to read virtual branches")
        .into_iter()
        .collect::<Vec<_>>();

    let mut ids = Vec::new();
    let mut names = Vec::new();
    for branch in virtual_branches {
        ids.push(branch.id);
        names.push(branch.name);
    }
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&names)
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap();
    let commit_branch = ids[selection.unwrap()].clone();
    println!("Committing virtual branch {}", commit_branch.red());

    // get the commit message
    let message: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Commit message")
        .interact_text()
        .unwrap();
    virtual_branches::commit(
        &butler.gb_repository,
        &butler.project_repository(),
        &commit_branch,
        &message,
    );
}

fn run_new(butler: ButlerCli) {
    let current_session = butler
        .gb_repository
        .get_or_create_current_session()
        .expect("failed to get or create currnt session");
    let current_session_reader = sessions::Reader::open(&butler.gb_repository, &current_session)
        .expect("failed to open current session reader");

    let target_reader = virtual_branches::target::Reader::new(&current_session_reader);
    let default_target = match target_reader.read_default() {
        Ok(target) => target,
        Err(reader::Error::NotFound) => return,
        Err(e) => panic!("failed to read default target: {}", e),
    };

    println!("  base sha: {}", default_target.sha.to_string().blue());
    // lookup tree for this sha
    let git_repository = butler.git_repository();
    let commit = git_repository.find_commit(default_target.sha).unwrap();
    let tree = commit.tree().unwrap();

    let input: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("New branch name")
        .interact_text()
        .unwrap();

    let now = time::UNIX_EPOCH.elapsed().unwrap().as_millis();

    let branch = virtual_branches::Branch {
        id: Uuid::new_v4().to_string(),
        name: input,
        applied: true,
        upstream: "".to_string(),
        tree: tree.id(),
        head: default_target.sha,
        created_timestamp_ms: now,
        updated_timestamp_ms: now,
        ownership: vec![],
    };

    let writer = virtual_branches::branch::Writer::new(&butler.gb_repository);
    writer.write(&branch).unwrap();
}

fn run_move(butler: ButlerCli) {
    // get the files to move
    let files =
        virtual_branches::get_status_files(&butler.gb_repository, &butler.project_repository())
            .expect("failed to get status files");
    let selected_files = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Which files do you want to move?")
        .items(&files[..])
        .interact()
        .expect("failed to get selections")
        .iter()
        .map(|i| files[*i].clone().into())
        .collect::<Vec<_>>();
    println!("Selected files: {:?}", selected_files);

    let current_session = butler
        .gb_repository
        .get_or_create_current_session()
        .expect("failed to get or create currnt session");
    let current_session_reader = sessions::Reader::open(&butler.gb_repository, &current_session)
        .expect("failed to open current session reader");

    let virtual_branches = virtual_branches::Iterator::new(&current_session_reader)
        .expect("failed to read virtual branches")
        .collect::<Result<Vec<virtual_branches::branch::Branch>, reader::Error>>()
        .expect("failed to read virtual branches")
        .into_iter()
        .collect::<Vec<_>>();

    let mut ids = Vec::new();
    let mut names = Vec::new();
    for branch in virtual_branches {
        ids.push(branch.id);
        names.push(branch.name);
    }
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&names)
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap();
    let target_branch_id = ids[selection.unwrap()].clone();

    println!("Moving {} files", selected_files.len());
    virtual_branches::move_files(&butler.gb_repository, &target_branch_id, &selected_files)
        .expect("failed to move files");
}

// TODO: vbranches: split function that identifies part of a file and moves that hunk to another branch
// - writes the ownership simply as: path/to/file:line_number-line_number

fn run_setup(butler: ButlerCli) {
    println!(
        "  HEAD: {}",
        butler
            .project_repository()
            .get_head()
            .unwrap()
            .name()
            .unwrap()
            .blue()
    );
    let items = butler.project_repository().git_remote_branches().unwrap();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap();

    match selection {
        Some(index) => {
            println!("Setting target to: {}", items[index].red());
            butler
                .gb_repository
                .set_target_branch(&butler.project_repository(), items[index].clone())
                .unwrap();
        }
        None => println!("User did not select anything"),
    }
}

fn run_status(butler: ButlerCli) {
    let statuses =
        virtual_branches::get_status_by_branch(&butler.gb_repository, &butler.project_repository())
            .expect("failed to get status by branch");
    for (branch, files) in statuses {
        println!("branch: {}", branch.name.blue());
        println!("  head: {}", branch.head.to_string().green());
        println!("  tree: {}", branch.tree.to_string().green());
        println!("    id: {}", branch.id.green());
        println!(" files:");
        for file in files {
            println!("        {}", file.path);
            for hunk in file.hunks {
                println!("          {}", hunk.id);
            }
        }
        println!();
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
    println!(
        "  description: {}",
        butler
            .project
            .description
            .clone()
            .unwrap_or("none".to_string())
            .blue()
    );
    println!("  last_fetched_ts: {:?}", butler.project.last_fetched_ts);
    println!("  path: {}", butler.project.path.blue());

    if let Some(api) = butler.project.api.as_ref() {
        println!("  {}:", "api".to_string().red());
        println!("   api name: {}", api.name.blue());
        println!(
            "   api description: {}",
            api.description.clone().unwrap().blue()
        );
        println!("   repo id: {}", api.repository_id.blue());
        println!("   git url: {}", api.git_url.blue());
        println!("   created: {}", api.created_at.blue());
        println!("   updated: {}", api.updated_at.blue());
    }

    println!("{}", "project repo:".to_string().red());
    println!(
        "  HEAD: {}",
        butler
            .project_repository()
            .get_head()
            .unwrap()
            .name()
            .unwrap()
            .blue()
    );

    println!("{}", "sessions:".to_string().red());
    // sessions storage
    let sessions = butler
        .sessions_db
        .list_by_project_id(&butler.project.id, butler.project.last_fetched_ts)
        .unwrap();
    //list the sessions
    for session in &sessions {
        println!("  id: {}", session.id.blue());
    }

    // gitbutler repo stuff
    // read default target

    let current_session = butler
        .gb_repository
        .get_or_create_current_session()
        .expect("failed to get or create currnt session");
    let current_session_reader = sessions::Reader::open(&butler.gb_repository, &current_session)
        .expect("failed to open current session reader");

    let target_reader = virtual_branches::target::Reader::new(&current_session_reader);
    match target_reader.read_default() {
        Ok(target) => {
            println!("{}", "target:".to_string().red());
            println!("  base sha: {}", target.sha.to_string().blue());
        }
        Err(reader::Error::NotFound) => {}
        Err(e) => panic!("failed to read default target: {}", e),
    };

    println!("{}", "virtual branches:".to_string().red());
    virtual_branches::Iterator::new(&current_session_reader)
        .expect("failed to read virtual branches")
        .collect::<Result<Vec<virtual_branches::branch::Branch>, reader::Error>>()
        .expect("failed to read virtual branches")
        .into_iter()
        .for_each(|branch| {
            println!("  {}", branch.name);
            for file in &branch.ownership {
                println!("    {}", file);
            }
        });
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
