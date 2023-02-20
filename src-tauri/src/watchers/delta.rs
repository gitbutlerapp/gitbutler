use crate::deltas::{read, write, Delta, TextDocument};
use crate::projects;
use crate::{events, sessions};
use anyhow::{Context, Result};
use git2::Repository;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread;
use std::time::SystemTime;
use std::{collections::HashMap, sync::Mutex};

#[derive(Default)]
pub struct WatcherCollection(Mutex<HashMap<String, RecommendedWatcher>>);

pub struct DeltaWatchers<'a> {
    watchers: &'a WatcherCollection,
}

impl<'a> DeltaWatchers<'a> {
    pub fn new(watchers: &'a WatcherCollection) -> Self {
        Self { watchers }
    }

    pub fn watch(&self, window: tauri::Window, project: projects::Project) -> Result<()> {
        log::info!("Watching deltas for {}", project.path);
        let project_path = Path::new(&project.path);

        let (tx, rx) = channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

        watcher.watch(project_path, RecursiveMode::Recursive)?;

        self.watchers
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
                        match register_file_change(
                            &window,
                            &project,
                            &repo,
                            &event.kind,
                            &relative_file_path,
                        ) {
                            Ok(Some((session, deltas))) => {
                                events::deltas(
                                    &window,
                                    &project,
                                    &session,
                                    &deltas,
                                    &relative_file_path,
                                );
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

    pub fn unwatch(&self, project: projects::Project) -> Result<()> {
        let mut watchers = self.watchers.0.lock().unwrap();
        if let Some(mut watcher) = watchers.remove(&project.path) {
            watcher.unwatch(Path::new(&project.path))?;
        }
        Ok(())
    }
}

// this is what is called when the FS watcher detects a change
// it should figure out delta data (crdt) and update the file at .git/gb/session/deltas/path/to/file
// it also writes the metadata stuff which marks the beginning of a session if a session is not yet started
// returns updated project deltas
fn register_file_change<R: tauri::Runtime>(
    window: &tauri::Window<R>,
    project: &projects::Project,
    repo: &Repository,
    kind: &EventKind,
    relative_file_path: &Path,
) -> Result<Option<(sessions::Session, Vec<Delta>)>, Box<dyn std::error::Error>> {
    if repo.is_path_ignored(&relative_file_path).unwrap_or(true) {
        // make sure we're not watching ignored files
        return Ok(None);
    }

    let file_path = repo.workdir().unwrap().join(relative_file_path);
    let file_contents = match fs::read_to_string(&file_path) {
        Ok(contents) => contents,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                // file doesn't exist, use empty string
                String::new()
            } else {
                // file exists, but content is not utf-8, it's a noop
                // TODO: support binary files
                log::info!("File is not utf-8, ignoring: {:?}", file_path);
                return Ok(None);
            }
        }
    };

    // update meta files every time file change is detected
    let session = write_beginning_meta_files(&window, &project, &repo)?;

    if EventKind::is_modify(&kind) {
        log::info!("File modified: {:?}", file_path);
    } else if EventKind::is_create(&kind) {
        log::info!("File created: {:?}", file_path);
    } else if EventKind::is_remove(&kind) {
        log::info!("File removed: {:?}", file_path);
    }

    // first, we need to check if the file exists in the meta commit
    let latest_contents = get_latest_file_contents(repo, project, relative_file_path)
        .with_context(|| {
            format!(
                "Failed to get latest file contents for {}",
                relative_file_path.display()
            )
        })?;

    // second, get non-flushed file deltas
    let deltas = read(project, relative_file_path).with_context(|| {
        format!(
            "Failed to get current file deltas for {}",
            relative_file_path.display()
        )
    })?;

    // depending on the above, we can create TextDocument suitable for calculating deltas
    let mut text_doc = match (latest_contents, deltas) {
        (Some(latest_contents), Some(deltas)) => TextDocument::new(&latest_contents, deltas),
        (Some(latest_contents), None) => TextDocument::new(&latest_contents, vec![]),
        (None, Some(deltas)) => TextDocument::from_deltas(deltas),
        (None, None) => TextDocument::from_deltas(vec![]),
    };

    if !text_doc.update(&file_contents) {
        return Ok(None);
    }

    // if the file was modified, save the deltas
    let deltas = text_doc.get_deltas();
    write(project, relative_file_path, &deltas)?;
    return Ok(Some((session, deltas)));
}

// returns last commited file contents from refs/gitbutler/current ref
// if it doesn't exists, fallsback to HEAD
// returns None if file doesn't exist in HEAD
// returns None if file is not UTF-8
// TODO: handle binary files
fn get_latest_file_contents(
    repo: &Repository,
    project: &projects::Project,
    relative_file_path: &Path,
) -> Result<Option<String>> {
    let tree_entry = match repo.find_reference(&project.refname()) {
        Ok(reference) => {
            let gitbutler_tree = reference.peel_to_tree()?;
            let gitbutler_tree_path = &Path::new("wd").join(relative_file_path);
            let tree_entry = gitbutler_tree.get_path(gitbutler_tree_path);
            tree_entry
        }
        Err(e) => {
            if e.code() == git2::ErrorCode::NotFound {
                let head = repo.head()?;
                let tree = head.peel_to_tree()?;
                let tree_entry = tree.get_path(relative_file_path);
                tree_entry
            } else {
                Err(e)
            }
        }
    };

    match tree_entry {
        Ok(tree_entry) => {
            // if file found, check if delta file exists
            let blob = tree_entry.to_object(&repo)?.into_blob().unwrap();
            // parse blob as utf-8.
            // if it's not utf8, return None
            let contents = match String::from_utf8(blob.content().to_vec()) {
                Ok(contents) => Some(contents),
                Err(_) => {
                    log::info!("File is not utf-8, ignoring: {:?}", relative_file_path);
                    None
                }
            };

            Ok(contents)
        }
        Err(e) => {
            if e.code() == git2::ErrorCode::NotFound {
                // file not found, return None
                Ok(None)
            } else {
                Err(e.into())
            }
        }
    }
}

// this function is called when the user modifies a file, it writes starting metadata if not there
// and also touches the last activity timestamp, so we can tell when we are idle
fn write_beginning_meta_files<R: tauri::Runtime>(
    window: &tauri::Window<R>,
    project: &projects::Project,
    repo: &Repository,
) -> Result<sessions::Session, Box<dyn std::error::Error>> {
    match sessions::Session::current(repo, project)
        .map_err(|e| format!("Error while getting current session: {}", e.to_string()))?
    {
        Some(mut session) => {
            let now_ts = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            session.meta.last_ts = now_ts;
            sessions::update_session(project, &session)
                .map_err(|e| format!("Error while updating current session: {}", e.to_string()))?;
            events::session(&window, &project, &session);
            Ok(session)
        }
        None => {
            let session = sessions::Session::from_head(repo, project)?;
            events::session(&window, &project, &session);
            Ok(session)
        }
    }
}
