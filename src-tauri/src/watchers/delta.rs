use crate::deltas::{get_current_file_deltas, save_current_file_deltas, Delta, TextDocument};
use crate::projects;
use crate::{butler, events, sessions};
use anyhow::Result;
use git2::Repository;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
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
    let contents = get_latest_file_contents(repo, relative_file_path)?;
    // second, get non-flushed file deltas
    let deltas = get_current_file_deltas(repo, relative_file_path)?;

    // depending on the above, we can create TextDocument suitable for calculating deltas
    let mut text_doc = match (contents, deltas) {
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
    save_current_file_deltas(repo, relative_file_path, &deltas)?;
    return Ok(Some((session, deltas)));
}

// returns last commited file contents from refs/gitbutler/current ref
// if it doesn't exists, fallsback to HEAD
// returns None if file doesn't exist in HEAD
fn get_latest_file_contents(
    repo: &Repository,
    relative_file_path: &Path,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    match repo.revparse_single(format!("refs/{}/current", butler::refname()).as_str()) {
        Ok(object) => {
            // refs/gitbutler/current exists, return file contents from wd dir
            let gitbutler_head = repo.find_commit(object.id())?;
            let gitbutler_tree = gitbutler_head.tree()?;
            // files are stored in the wd tree inside gitbutler trees.
            let gitbutler_tree_path = &Path::new("wd").join(relative_file_path);
            if let Ok(tree_entry) = gitbutler_tree.get_path(gitbutler_tree_path) {
                // if file found, check if delta file exists
                let blob = tree_entry.to_object(&repo)?.into_blob().unwrap();
                let contents = String::from_utf8(blob.content().to_vec())?;
                Ok(Some(contents))
            } else {
                Ok(None)
            }
        }
        Err(_) => {
            // refs/gitbutler/current doesn't exist, return file contents from HEAD
            let head = repo.head()?;
            let tree = head.peel_to_tree()?;
            if let Ok(tree_entry) = tree.get_path(relative_file_path) {
                // if file found, check if delta file exists
                let blob = tree_entry.to_object(&repo)?.into_blob().unwrap();
                let contents = String::from_utf8(blob.content().to_vec())?;
                Ok(Some(contents))
            } else {
                Ok(None)
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
    match sessions::Session::current(repo)
        .map_err(|e| format!("Error while getting current session: {}", e.to_string()))?
    {
        Some(mut session) => {
            let now_ts = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            session.meta.last_ts = now_ts;
            sessions::update_session(repo, &session)
                .map_err(|e| format!("Error while updating current session: {}", e.to_string()))?;
            events::session(&window, &project, &session);
            Ok(session)
        }
        None => {
            let session = sessions::Session::from_head(repo)?;
            events::session(&window, &project, &session);
            Ok(session)
        }
    }
}
