use crate::deltas::{read, write, Delta, TextDocument};
use crate::projects;
use crate::{events, sessions};
use anyhow::{Context, Result};
use git2;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::mpsc;

pub struct DeltaWatchers {
    watchers: HashMap<String, RecommendedWatcher>,
}

fn is_interesting_event(kind: &notify::EventKind) -> Option<String> {
    match kind {
        notify::EventKind::Create(notify::event::CreateKind::File) => {
            Some("file created".to_string())
        }
        notify::EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
            Some("file modified".to_string())
        }
        notify::EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
            Some("file renamed".to_string())
        }
        notify::EventKind::Remove(notify::event::RemoveKind::File) => {
            Some("file removed".to_string())
        }
        _ => None,
    }
}

impl DeltaWatchers {
    pub fn new() -> Self {
        Self {
            watchers: Default::default(),
        }
    }

    pub fn watch(
        &mut self,
        sender: mpsc::Sender<events::Event>,
        project: projects::Project,
    ) -> Result<()> {
        log::info!("Watching deltas for {}", project.path);
        let project_path = Path::new(&project.path);

        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

        watcher.watch(project_path, RecursiveMode::Recursive)?;

        self.watchers.insert(project.path.clone(), watcher);

        tauri::async_runtime::spawn_blocking(move || {
            while let Ok(event) = rx.recv() {
                match event {
                    Ok(notify_event) => {
                        for file_path in notify_event.paths {
                            let relative_file_path =
                                file_path.strip_prefix(project.path.clone()).unwrap();
                            let repo = git2::Repository::open(&project.path).expect(
                                format!("failed to open repo at {}", project.path).as_str(),
                            );

                            if repo.is_path_ignored(&relative_file_path).unwrap_or(true) {
                                // make sure we're not watching ignored files
                                continue;
                            }

                            if let Some(kind_string) = is_interesting_event(&notify_event.kind) {
                                log::info!(
                                    "{}: \"{}\" {}",
                                    project.id,
                                    relative_file_path.display(),
                                    kind_string
                                );
                            } else {
                                continue;
                            }

                            match register_file_change(&project, &repo, &relative_file_path) {
                                Ok(Some((session, deltas))) => {
                                    if let Err(e) =
                                        sender.send(events::Event::session(&project, &session))
                                    {
                                        log::error!("filed to send session event: {:#}", e)
                                    }

                                    if let Err(e) = sender.send(events::Event::detlas(
                                        &project,
                                        &session,
                                        &deltas,
                                        &relative_file_path,
                                    )) {
                                        log::error!("failed to send deltas event: {:#}", e)
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => log::error!("failed to register file change: {:#}", e),
                            }
                        }
                    }
                    Err(e) => log::error!("notify event error: {:#}", e),
                }
            }
        });

        Ok(())
    }

    pub fn unwatch(&mut self, project: projects::Project) -> Result<()> {
        if let Some(mut watcher) = self.watchers.remove(&project.path) {
            watcher.unwatch(Path::new(&project.path))?;
        }
        Ok(())
    }
}

// this is what is called when the FS watcher detects a change
// it should figure out delta data (crdt) and update the file at .git/gb/session/deltas/path/to/file
// it also writes the metadata stuff which marks the beginning of a session if a session is not yet started
// returns updated project deltas and sessions to which they belong
pub(crate) fn register_file_change(
    project: &projects::Project,
    repo: &git2::Repository,
    relative_file_path: &Path,
) -> Result<Option<(sessions::Session, Vec<Delta>)>> {
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

    // first, we need to check if the file exists in the meta commit
    let latest_contents = get_latest_file_contents(&repo, project, relative_file_path)
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
        (Some(latest_contents), Some(deltas)) => TextDocument::new(&latest_contents, deltas)?,
        (Some(latest_contents), None) => TextDocument::new(&latest_contents, vec![])?,
        (None, Some(deltas)) => TextDocument::from_deltas(deltas)?,
        (None, None) => TextDocument::from_deltas(vec![])?,
    };

    if !text_doc.update(&file_contents)? {
        return Ok(None);
    } else {
        // if the file was modified, save the deltas
        let deltas = text_doc.get_deltas();
        let session = write(&repo, project, relative_file_path, &deltas)?;
        Ok(Some((session, deltas)))
    }
}

// returns last commited file contents from refs/gitbutler/current ref
// if it doesn't exists, fallsback to HEAD
// returns None if file doesn't exist in HEAD
// returns None if file is not UTF-8
// TODO: handle binary files
fn get_latest_file_contents(
    repo: &git2::Repository,
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
