use crate::crdt::{self, TextDocument};
use crate::projects::Project;
use futures::{
    channel::mpsc::{channel, Receiver},
    SinkExt, StreamExt,
};
use git2::{Commit, Oid, Repository};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use path_absolutize::*;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

// thing that figures out which watcher we want to use
fn async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (mut tx, rx) = channel(1);

    // Automatically select the best implementation for your platform.
    // You can also access each implementation directly e.g. INotifyWatcher.
    let watcher = RecommendedWatcher::new(
        move |res| {
            futures::executor::block_on(async {
                tx.send(res).await.unwrap();
            })
        },
        Config::default(),
    )?;

    Ok((watcher, rx))
}

// this waits for a change to the filesystem, and then writes the crdt/delta data to the .git directory
pub async fn watch<P: AsRef<Project>>(project: &P) -> notify::Result<()> {
    let (mut watcher, mut rx) = async_watcher()?;

    // Open the repository at this path
    let path = Path::new(&project.as_ref().path);
    let repo = match Repository::open(path) {
        Ok(repo) => repo,
        Err(e) => panic!("failed to open: {}", e),
    };

    // Add a path to be watched. All files and directories at that path and
    // below will be monitored for changes.
    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    // TODO: probably debounce this
    while let Some(res) = rx.next().await {
        match res {
            Ok(event) => {
                register_file_change(&repo, project.as_ref(), event.kind, event.paths);
            }
            Err(e) => log::error!("watch error: {:?}", e),
        }
    }

    Ok(())
}

// this is what is called when the FS watcher detects a change
// it should figure out delta data (crdt) and update the file at .git/gb/session/deltas/path/to/file
// it also writes the metadata stuff which marks the beginning of a session if a session is not yet started
fn register_file_change(
    repo: &Repository,
    project: &Project,
    kind: EventKind,
    files: Vec<PathBuf>,
) {
    write_beginning_meta_files(&repo);

    let project_path = PathBuf::from(&project.path);
    for file_path in files {
        if !file_path.is_file() {
            continue;
        }

        let relative_file_path = file_path
            .absolutize()
            .unwrap()
            .strip_prefix(project_path.clone())
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        if repo.is_path_ignored(&relative_file_path).unwrap_or(true) {
            continue;
        }

        if EventKind::is_modify(&kind) {
            log::info!("File modified: {:?}", file_path);
        } else if EventKind::is_create(&kind) {
            log::info!("File created: {:?}", file_path);
        } else if EventKind::is_remove(&kind) {
            log::info!("File removed: {:?}", file_path);
        }

        let relative_deltas_path = PathBuf::from(".git/gb/session/deltas/");
        let delta_path = project_path
            .join(relative_deltas_path)
            .join(relative_file_path.clone())
            .clone();
        std::fs::create_dir_all(delta_path.parent().unwrap()).unwrap();

        let deltas = if delta_path.exists() {
            let raw_deltas = std::fs::read_to_string(delta_path.clone())
                .expect(format!("Failed to read {}", delta_path.to_str().unwrap()).as_str());
            let deltas: Vec<crdt::Delta> = serde_json::from_str(&raw_deltas)
                .expect(format!("Failed to parse {}", delta_path.to_str().unwrap()).as_str());
            Some(deltas)
        } else {
            None
        };

        // first, we need to check if the file exists in the meta commit
        let meta_commit = get_meta_commit(&repo);
        let tree = meta_commit.tree().unwrap();
        let commit_blob = if let Ok(object) = tree.get_path(Path::new(&relative_file_path)) {
            // if file found, check if delta file exists
            let blob = object.to_object(&repo).unwrap().into_blob().unwrap();
            let contents = String::from_utf8(blob.content().to_vec()).unwrap();
            Some(contents)
        } else {
            None
        };

        let mut text_doc = match (commit_blob, deltas) {
            (Some(contents), Some(deltas)) => {
                println!("found deltas and commit blob");
                TextDocument::new(&contents, deltas)
            }
            (Some(contents), None) => {
                println!("found commit blob, no deltas");
                TextDocument::new(&contents, vec![])
            }
            (None, Some(deltas)) => {
                println!("found deltas, no commit blob");
                TextDocument::from_deltas(deltas)
            }
            (None, None) => {
                println!("no deltas or commit blob");
                TextDocument::from_deltas(vec![])
            }
        };

        let contents = std::fs::read_to_string(file_path.clone())
            .expect(format!("Failed to read {}", file_path.to_str().unwrap()).as_str());

        if !text_doc.update(&contents) {
            // if the document hasn't changed, we don't need to write a delta.
            continue;
        }

        let deltas = text_doc.get_deltas();

        log::info!("Writing delta to {}", delta_path.to_str().unwrap());

        let mut file = File::create(delta_path).unwrap();
        file.write_all(serde_json::to_string(&deltas).unwrap().as_bytes())
            .unwrap();
    }
}

fn get_meta_commit(repo: &Repository) -> Commit {
    // TODO: wrong commit ? 
    let meta_path = repo.path().join(Path::new("gb/session/meta"));
    let meta_commit = meta_path.join(Path::new("commit"));
    let contents = std::fs::read_to_string(meta_commit.clone())
        .expect(format!("Failed to read {}", meta_commit.to_str().unwrap()).as_str());
    let raw_commit = contents.as_str();
    repo.find_commit(Oid::from_str(raw_commit).unwrap())
        .unwrap()
}

// this function is called when the user modifies a file, it writes starting metadata if not there
// and also touches the last activity timestamp, so we can tell when we are idle
fn write_beginning_meta_files(repo: &Repository) {
    let meta_path = repo.path().join(Path::new("gb/session/meta"));
    // create the parent directory recurisvely if it doesn't exist
    std::fs::create_dir_all(meta_path.clone()).unwrap();

    // check if the file .git/gb/meta/start exists and if not, write the current timestamp into it
    let meta_session_start = meta_path.join(Path::new("session-start"));
    if !meta_session_start.exists() {
        let mut file = File::create(meta_session_start).unwrap();
        file.write_all(chrono::Local::now().timestamp().to_string().as_bytes())
            .unwrap();
    }

    // check if the file .git/gb/session/meta/branch exists and if not, write the current branch name into it
    let meta_branch = meta_path.join(Path::new("branch"));
    if !meta_branch.exists() {
        let mut file = File::create(meta_branch).unwrap();
        let branch = repo.head().unwrap();
        let branch_name = branch.name().unwrap();
        file.write_all(branch_name.as_bytes()).unwrap();
    }

    // check if the file .git/gb/session/meta/commit exists and if not, write the current commit hash into it
    let meta_commit = meta_path.join(Path::new("commit"));
    if !meta_commit.exists() {
        let mut file = File::create(meta_commit).unwrap();
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        file.write_all(commit.id().to_string().as_bytes()).unwrap();
    }

    // ALWAYS write the last time we did this
    let meta_session_last = meta_path.join(Path::new("session-last"));
    let mut file = File::create(meta_session_last).unwrap();
    file.write_all(chrono::Local::now().timestamp().to_string().as_bytes())
        .unwrap();
}
