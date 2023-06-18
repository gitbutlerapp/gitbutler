use std::time;
use clap::Parser;
use colored::Colorize;
use git2::Repository;
use dirs;
use dialoguer::{console::Term, theme::ColorfulTheme, MultiSelect, Select, Input};
use uuid::Uuid;

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
        "info"   => run_info(butler),   // shows internal data states for the project
        "status" => run_status(butler), // shows virtual branch status
        "new"    => run_new(butler),    // create new empty virtual branch
        "move"   => run_move(butler),   // move file ownership from one branch to another
        "setup"  => run_setup(butler),  // sets target sha from remote branch
        "commit" => run_commit(butler), // creates trees from the virtual branch content and creates a commit
        _ => println!("Unknown command: {}", args.command)
    }
}

fn run_commit(butler: ButlerCli) {
    // get the branch to commit
    let mut branches = vec![]; 
    let branch_reader = butler.gb_repository.get_branch_dir_reader();
    let mut iter = virtual_branches::Iterator::new(&branch_reader).unwrap();
    while let Some(item) = iter.next() {
        if let Ok(item) = item {
            branches.push(item.name);
        }
    }
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&branches)
        .default(0)
        .interact_on_opt(&Term::stderr()).unwrap();
    let commit_branch = branches[selection.unwrap()].clone();
    println!("Committing virtual branch {}", commit_branch.red());

    // get the commit message
    let message: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Commit message")
        .interact_text()
        .unwrap();

    // get the files to commit
    if let Some(sha) = get_base_sha(&butler) {
        let statuses = get_status_by_branch(&butler);
        for (branch_id, files) in statuses {
            let mut branch = butler.gb_repository.get_virtual_branch(&branch_id).unwrap();
            if branch.name == commit_branch {
                println!("  branch: {}", branch_id.blue());
                println!("    base: {}", sha.blue());

                // read the base sha into an index
                let git_repository = butler.git_repository();
                let base_oid = git2::Oid::from_str(&sha).unwrap();
                let base_commit = git_repository.find_commit(base_oid).unwrap();
                let base_tree = base_commit.tree().unwrap();
                let parent_commit = git_repository.find_commit(branch.head).unwrap();
                let mut index = git_repository.index().unwrap();
                index.read_tree(&base_tree).unwrap();

                // now update the index with content in the working directory for each file
                for file in files {
                    println!("{}", file);
                    // convert this string to a Path
                    let file = std::path::Path::new(&file);

                    // TODO: deal with removals too
                    index.add_path(&file).unwrap();
                }

                // now write out the tree
                let tree_oid = index.write_tree().unwrap();

                // only commit if it's a new tree
                if tree_oid != branch.tree {
                    let tree = git_repository.find_tree(tree_oid).unwrap();
                    // now write a commit
                    let (author, committer) = butler.gb_repository.git_signatures().unwrap();
                    let commit_oid = git_repository.commit(
                        None,
                        &author,
                        &committer,
                        &message,
                        &tree,
                        &[&parent_commit],
                    ).unwrap();
                    // write this new commit to the virtual branch
                    println!("    commit: {}", commit_oid.to_string().blue());

                    // update the virtual branch head
                    branch.tree = tree_oid;
                    branch.head = commit_oid;
                    let writer = virtual_branches::branch::Writer::new(&butler.gb_repository);
                    writer.write(&branch).unwrap();
                }
            }
        }
    }

    // create the tree

    // create the commit

}

fn run_new(butler: ButlerCli) {
    if let Some(sha) = get_base_sha(&butler) {
        println!("  base sha: {}", sha.blue());
        let oid = git2::Oid::from_str(&sha).unwrap();
        // lookup tree for this sha
        let git_repository = butler.git_repository();
        let commit = git_repository.find_commit(oid).unwrap();
        let tree = commit.tree().unwrap();

        let input: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("New branch name")
            .interact_text()
            .unwrap();

        let now = time::UNIX_EPOCH
            .elapsed()
            .unwrap()
            .as_millis();

        let branch = virtual_branches::Branch {
            id: Uuid::new_v4().to_string(),
            name: input,
            applied: true,
            upstream: "".to_string(),
            tree: tree.id(),
            head: oid,
            created_timestamp_ms: now,
            updated_timestamp_ms: now,
            ownership: vec![],
        };

        let writer = virtual_branches::branch::Writer::new(&butler.gb_repository);
        writer.write(&branch).unwrap();
    }
}

fn run_move(butler: ButlerCli) {
    // get the files to move
    let files = get_status_files(&butler);
    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Which files do you want to move?")
        .items(&files[..])
        .interact()
        .unwrap();
    let mut selected_files = vec![];
    for selection in &selections {
        selected_files.push(files[*selection].clone());
    }

    // get the branch to move to
    let mut branches = vec![]; 
    let branch_reader = butler.gb_repository.get_branch_dir_reader();
    let mut iter = virtual_branches::Iterator::new(&branch_reader).unwrap();
    while let Some(item) = iter.next() {
        if let Ok(item) = item {
            branches.push(item.name);
        }
    }
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&branches)
        .default(0)
        .interact_on_opt(&Term::stderr()).unwrap();
    let new_branch = branches[selection.unwrap()].clone();

    println!("Moving {} files to {}", selections.len(), new_branch.red());

    // rewrite ownership of both branches
    let writer = virtual_branches::branch::Writer::new(&butler.gb_repository);
    let mut iter = virtual_branches::Iterator::new(&branch_reader).unwrap();
    while let Some(item) = iter.next() {
        if let Ok(item) = item {
            let mut branch = item;
            if branch.name == new_branch {
                branch.ownership.extend(selected_files.iter().map(|f| f.to_string()));
            } else {
                branch.ownership.retain(|f| !selected_files.contains(f));
            }
            writer.write(&branch).unwrap();
        }
    }
}

// TODO: vbranches: split function that identifies part of a file and moves that hunk to another branch
// - writes the ownership simply as: path/to/file:line_number-line_number

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

            let commit = branch.get().peel_to_commit().unwrap();
            let target = virtual_branches::target::Target {
                name: branch.name().unwrap().unwrap().to_string(),
                remote: remote_url_str.to_string(),
                sha: commit.id(),
            };

            let target_writer = virtual_branches::target::Writer::new(&butler.gb_repository);
            target_writer.write_default(&target).unwrap();

            let now = time::UNIX_EPOCH
                .elapsed()
                .unwrap()
                .as_millis();
            let writer = butler.gb_repository.get_branch_writer();
            let branch = virtual_branches::branch::Branch {
                id: Uuid::new_v4().to_string(),
                name: "default branch".to_string(),
                applied: true,
                upstream: "".to_string(),
                created_timestamp_ms: now,
                updated_timestamp_ms: now,
                tree: commit.tree().unwrap().id(),
                head: commit.id(),
                ownership: vec![],
            };
            writer.write(&branch).unwrap();
        }
        None => println!("User did not select anything")
    }
}


fn run_status(butler: ButlerCli) {
    let statuses = get_status_by_branch(&butler);
    for (branch_id, files) in statuses {
        let branch = butler.gb_repository.get_virtual_branch(&branch_id).unwrap();
        println!("branch: {}", branch.name.blue());
        println!("  head: {}", branch.head.to_string().green());
        println!("  tree: {}", branch.tree.to_string().green());
        println!("    id: {}", branch.id.green());
        println!(" files:");
        for file in files {
            println!("        {}", file);
        }
        println!("");
    }
}

// list the virtual branches and their file statuses (statusi?)
fn get_status_by_branch(butler: &ButlerCli) -> Vec<(String, Vec<String>)> {
    let mut statuses = vec![];

    if let Some(sha) = get_base_sha(&butler) {
        //println!("  base sha: {}", sha.blue());
        let branch_reader = butler.gb_repository.get_branch_dir_reader();

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

        let mut all_files = vec![];
        let mut new_ownership = vec![];

        let deltas = diff.deltas();
        for delta in deltas {
            let mut file_path = "".to_string();
            let old_file = delta.old_file();
            let new_file = delta.new_file();

            if let Some(path) = new_file.path() {
                file_path = path.to_str().unwrap().to_string();
            } else if let Some(path) = old_file.path() {
                file_path = path.to_str().unwrap().to_string();
            }
            all_files.push(file_path.clone());

            let mut branch_iter = virtual_branches::Iterator::new(&branch_reader).unwrap();
            let mut file_found = false;
            while let Some(item) = branch_iter.next() {
                if let Ok(item) = item {
                    for file in item.ownership {
                        if file == file_path {
                            file_found = true;
                        }
                    }
                }
            }
            if !file_found {
                new_ownership.push(file_path.clone());
            }
        }

        //println!("new ownership: {:?}", new_ownership);

        let _vbranch_reader = virtual_branches::branch::Reader::new(&branch_reader);
        let mut iter = virtual_branches::Iterator::new(&branch_reader).unwrap();
        while let Some(item) = iter.next() {
            if let Ok(branch) = item {
                let mut files = vec![];
                if !new_ownership.is_empty() {
                    // in this case, lets add any newly changed files to the first branch we see and persist it
                    let mut branch = branch.clone();
                    branch.ownership.extend(new_ownership.clone());
                    new_ownership.clear();

                    // ok, write the updated data back
                    let writer = virtual_branches::branch::Writer::new(&butler.gb_repository);
                    writer.write(&branch).unwrap();

                    for file in branch.ownership {
                        if all_files.contains(&file) {
                            // push the file to the status list
                            files.push(file.clone());
                        }
                    }
                } else {
                    for file in branch.ownership {
                        if all_files.contains(&file) {
                            files.push(file.clone());
                        }
                    }
                }
                statuses.push((branch.id.clone(), files.clone()));
            }
        }
    } else {
        println!("  no base sha set, run butler setup");
    }

    statuses
}

fn get_status_files(butler: &ButlerCli) -> Vec<String> {
    if let Some(sha) = get_base_sha(&butler) {
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

        let mut all_files = vec![];

        let deltas = diff.deltas();
        for delta in deltas {
            let mut file_path = "".to_string();
            let old_file = delta.old_file();
            let new_file = delta.new_file();

            if let Some(path) = new_file.path() {
                file_path = path.to_str().unwrap().to_string();
            } else if let Some(path) = old_file.path() {
                file_path = path.to_str().unwrap().to_string();
            }
            all_files.push(file_path.clone());
        }
        all_files
    } else {
        vec![]
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
    println!("{}", "target:".to_string().red());
    if let Some(sha) = get_base_sha(&butler) {
        println!("  base sha: {}", sha.blue());
    }

    println!("{}", "virtual branches:".to_string().red());
    // sort of abusing the iterator here, but it works
    let branch_reader = butler.gb_repository.get_branch_dir_reader();
    let mut iter = virtual_branches::Iterator::new(&branch_reader).unwrap();
    while let Some(item) = iter.next() {
        if let Ok(item) = item {
            println!("  {}", item.name);
            for file in item.ownership {
                println!("    {}", file);
            }
        }
    }
}

fn get_base_sha(butler: &ButlerCli) -> Option<String> {
    let reader = butler.gb_repository.get_branch_dir_reader();
    let target_reader = virtual_branches::target::Reader::new(&reader);
    if let Ok(target) = target_reader.read_default() {
        Some(target.sha.to_string())
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