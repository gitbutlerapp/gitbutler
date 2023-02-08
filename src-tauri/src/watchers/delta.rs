use crate::butler::{get_file_deltas, save_file_deltas};
use crate::crdt::{Delta, TextDocument};
use crate::projects::Project;
use git2::{Commit, Repository};
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::thread;
use std::{collections::HashMap, fs::File, sync::Mutex};
use std::{io::Write, sync::mpsc::channel};
use tauri::{Runtime, Window};

#[derive(Default)]
pub struct WatcherCollection(Mutex<HashMap<String, RecommendedWatcher>>);

#[derive(Debug)]
pub enum UnwatchError {
    UnwatchError(notify::Error),
}

impl From<notify::Error> for UnwatchError {
    fn from(error: notify::Error) -> Self {
        UnwatchError::UnwatchError(error)
    }
}

pub fn unwatch(watchers: &WatcherCollection, project: Project) -> Result<(), UnwatchError> {
    let mut watchers = watchers.0.lock().unwrap();
    if let Some(mut watcher) = watchers.remove(&project.path) {
        watcher.unwatch(Path::new(&project.path))?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DeltasEvent {
    file_path: String,
    deltas: Vec<Delta>,
}

#[derive(Debug)]
pub enum WatchError {
    GitError(git2::Error),
    WatchError(notify::Error),
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchError::GitError(e) => write!(f, "Git error: {}", e),
            WatchError::WatchError(e) => write!(f, "Watch error: {}", e),
        }
    }
}

impl From<notify::Error> for WatchError {
    fn from(error: notify::Error) -> Self {
        WatchError::WatchError(error)
    }
}

impl From<git2::Error> for WatchError {
    fn from(error: git2::Error) -> Self {
        WatchError::GitError(error)
    }
}

pub fn watch<R: Runtime>(
    window: Window<R>,
    watchers: &WatcherCollection,
    project: Project,
) -> Result<(), WatchError> {
    log::info!("Watching deltas for {}", project.path);
    let project_path = Path::new(&project.path);

    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    watcher.watch(project_path, RecursiveMode::Recursive)?;
    watchers
        .0
        .lock()
        .unwrap()
        .insert(project.path.clone(), watcher);

    let repo = Repository::open(project_path);
    thread::spawn(move || {
        if repo.is_err() {
            log::error!("failed to open git repo: {:?}", repo.err());
            return;
        }
        let repo = repo.unwrap();

        while let Ok(event) = rx.recv() {
            if let Ok(event) = event {
                for file_path in event.paths {
                    let relative_file_path =
                        file_path.strip_prefix(repo.workdir().unwrap()).unwrap();
                    match register_file_change(&repo, &event.kind, &relative_file_path) {
                        Ok(Some(deltas)) => {
                            let event_name = format!("deltas://{}", project.id);

                            log::info!("Emitting event: {}", event_name);
                            match window.emit(
                                &event_name,
                                &DeltasEvent {
                                    deltas,
                                    file_path: relative_file_path.to_str().unwrap().to_string(),
                                },
                            ) {
                                Ok(_) => {}
                                Err(e) => log::error!("Error: {:?}", e),
                            };
                        }
                        Ok(None) => {}
                        Err(e) => log::error!("Error: {:?}", e),
                    }
                }
            } else {
                log::error!("Error: {:?}", event);
            }
        }
    });

    Ok(())
}

// this is what is called when the FS watcher detects a change
// it should figure out delta data (crdt) and update the file at .git/gb/session/deltas/path/to/file
// it also writes the metadata stuff which marks the beginning of a session if a session is not yet started
// returns updated project deltas
fn register_file_change(
    repo: &Repository,
    kind: &EventKind,
    file_path: &Path,
) -> Result<Option<Vec<Delta>>, Box<dyn std::error::Error>> {
    // update meta files every time file change is detected
    write_beginning_meta_files(&repo)?;

    if !file_path.is_file() {
        // only handle file changes
        return Ok(None);
    }

    if repo.is_path_ignored(&file_path).unwrap_or(true) {
        // make sure we're not watching ignored files
        return Ok(None);
    }

    if EventKind::is_modify(&kind) {
        log::info!("File modified: {:?}", file_path);
    } else if EventKind::is_create(&kind) {
        log::info!("File created: {:?}", file_path);
    } else if EventKind::is_remove(&kind) {
        log::info!("File removed: {:?}", file_path);
    }

    // first, we need to check if the file exists in the meta commit
    let meta_commit = get_meta_commit(&repo);
    let tree = meta_commit.tree().unwrap();
    let commit_blob = if let Ok(object) = tree.get_path(file_path) {
        // if file found, check if delta file exists
        let blob = object.to_object(&repo).unwrap().into_blob().unwrap();
        let contents = String::from_utf8(blob.content().to_vec()).unwrap();
        Some(contents)
    } else {
        None
    };

    // second, get non-flushed file deltas
    let deltas = get_file_deltas(&repo.workdir().unwrap(), file_path)?;

    // depending on the above, we can create TextDocument
    let mut text_doc = match (commit_blob, deltas) {
        (Some(contents), Some(deltas)) => TextDocument::new(&contents, deltas),
        (Some(contents), None) => TextDocument::new(&contents, vec![]),
        (None, Some(deltas)) => TextDocument::from_deltas(deltas),
        (None, None) => TextDocument::from_deltas(vec![]),
    };

    // update the TextDocument with the new file contents
    let contents = std::fs::read_to_string(file_path.clone())?;

    if !text_doc.update(&contents) {
        return Ok(None);
    }

    // if the file was modified, save the deltas
    let deltas = text_doc.get_deltas();
    save_file_deltas(repo, file_path, &deltas)?;
    return Ok(Some(deltas));
}

// get commit from refs/gitbutler/current or fall back to HEAD
// TODO: make this private as soon as possible
pub fn get_meta_commit(repo: &Repository) -> Commit {
    match repo.revparse_single("refs/gitbutler/current") {
        Ok(object) => repo.find_commit(object.id()).unwrap(),
        Err(_) => {
            let head = repo.head().unwrap();
            repo.find_commit(head.target().unwrap()).unwrap()
        }
    }
}

// this function is called when the user modifies a file, it writes starting metadata if not there
// and also touches the last activity timestamp, so we can tell when we are idle
fn write_beginning_meta_files(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
    let meta_path = repo.path().join(Path::new("gb/session/meta"));
    // create the parent directory recurisvely if it doesn't exist
    std::fs::create_dir_all(meta_path.clone())?;

    // check if the file .git/gb/meta/start exists and if not, write the current timestamp into it
    let meta_session_start = meta_path.join(Path::new("session-start"));
    if !meta_session_start.exists() {
        let mut file = File::create(meta_session_start)?;
        file.write_all(chrono::Local::now().timestamp().to_string().as_bytes())?;
    }

    // check if the file .git/gb/session/meta/branch exists and if not, write the current branch name into it
    let meta_branch = meta_path.join(Path::new("branch"));
    if !meta_branch.exists() {
        let mut file = File::create(meta_branch)?;
        let branch = repo.head()?;
        let branch_name = branch.name().unwrap();
        file.write_all(branch_name.as_bytes())?;
    }

    // check if the file .git/gb/session/meta/commit exists and if not, write the current commit hash into it
    let meta_commit = meta_path.join(Path::new("commit"));
    if !meta_commit.exists() {
        let mut file = File::create(meta_commit)?;
        let commit = repo.head().unwrap().peel_to_commit()?;
        file.write_all(commit.id().to_string().as_bytes())?;
    }

    // ALWAYS write the last time we did this
    let meta_session_last = meta_path.join(Path::new("session-last"));
    let mut file = File::create(meta_session_last)?;
    file.write_all(chrono::Local::now().timestamp().to_string().as_bytes())?;

    Ok(())
}
