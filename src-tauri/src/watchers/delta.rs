use crate::deltas::{self, Delta, TextDocument};
use crate::projects;
use crate::{events, sessions};
use anyhow::{Context, Result};
use git2;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub async fn on_file_change(
    sender: &tokio::sync::mpsc::Sender<events::Event>,
    fslock: Arc<tokio::sync::Mutex<fslock::LockFile>>,
    project: &projects::Project,
    deltas_storage: &deltas::Store,
    relative_file_path: &Path,
) -> Result<()> {
    let repo = git2::Repository::open(&project.path).expect(
        format!(
            "{}: failed to open repo at \"{}\"",
            project.id, project.path
        )
        .as_str(),
    );

    if repo.is_path_ignored(&relative_file_path).unwrap_or(true) {
        // make sure we're not watching ignored files
        return Ok(());
    }
    log::info!(
        "{}: {} changed",
        project.id,
        relative_file_path.to_str().unwrap()
    );

    let mut fslock = fslock.lock().await;
    log::debug!("{}: locking", project.id);
    fslock.lock().unwrap();
    log::debug!("{}: locked", project.id);

    let change = match register_file_change(&project, &repo, &deltas_storage, &relative_file_path) {
        Ok(change) => change,
        Err(e) => {
            log::error!("{}: failed to register file change: {:#}", project.id, e);
            None
        }
    };

    log::debug!("{}: unlocking", project.id);
    fslock.unlock().unwrap();
    log::debug!("{}: unlocked", project.id);

    if let Some((session, deltas)) = change {
        if let Err(e) = sender
            .send(events::Event::session(&project, &session))
            .await
        {
            log::error!("{}: failed to send session event: {:#}", project.id, e);
        }

        if let Err(e) = sender
            .send(events::Event::detlas(
                &project,
                &session,
                &deltas,
                &relative_file_path,
            ))
            .await
        {
            log::error!("{}: failed to send deltas event: {:#}", project.id, e);
        }
    }
    Ok(())
}

// this is what is called when the FS watcher detects a change
// it should figure out delta data (crdt) and update the file at .git/gb/session/deltas/path/to/file
// returns current project session and calculated deltas, if any.
pub(crate) fn register_file_change(
    project: &projects::Project,
    repo: &git2::Repository,
    deltas_storage: &deltas::Store,
    relative_file_path: &Path,
) -> Result<Option<(sessions::Session, Vec<Delta>)>> {
    let file_path = repo.workdir().unwrap().join(relative_file_path);
    let current_file_contents = match fs::read_to_string(&file_path) {
        Ok(contents) => contents,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                // file doesn't exist, use empty string
                String::new()
            } else {
                // file exists, but content is not utf-8, it's a noop
                // TODO: support binary files
                log::info!(
                    "{}: \"{}\" is not utf-8, ignoring",
                    project.id,
                    file_path.display()
                );
                return Ok(None);
            }
        }
    };

    // first, get latest file contens to compare with
    let latest_contents = get_latest_file_contents(&repo, project, relative_file_path)
        .with_context(|| {
            format!(
                "failed to get latest file contents for {}",
                relative_file_path.display()
            )
        })?;

    // second, get non-flushed file deltas
    let deltas = deltas_storage.read(relative_file_path).with_context(|| {
        format!(
            "failed to get current file deltas for {}",
            relative_file_path.display()
        )
    })?;

    // depending on the above, we can create TextDocument suitable for calculating _new_ deltas
    let mut text_doc = match (latest_contents, deltas) {
        (Some(latest_contents), Some(deltas)) => TextDocument::new(Some(&latest_contents), deltas)?,
        (Some(latest_contents), None) => TextDocument::new(Some(&latest_contents), vec![])?,
        (None, Some(deltas)) => TextDocument::new(None, deltas)?,
        (None, None) => TextDocument::new(None, vec![])?,
    };

    if !text_doc.update(&current_file_contents)? {
        log::debug!(
            "{}: \"{}\" no new deltas, ignoring",
            project.id,
            relative_file_path.display()
        );
        return Ok(None);
    }

    // if the file was modified, save the deltas
    let deltas = text_doc.get_deltas();
    let session = deltas_storage
        .write(relative_file_path, &deltas)
        .with_context(|| {
            format!(
                "failed to write file deltas for {}",
                relative_file_path.display()
            )
        })?;

    // save file contents corresponding to the deltas
    fs::create_dir_all(project.wd_path().join(relative_file_path).parent().unwrap())?;
    fs::write(
        project.wd_path().join(relative_file_path),
        current_file_contents,
    )
    .with_context(|| {
        format!(
            "failed to write file contents to {}",
            project.wd_path().join(relative_file_path).display()
        )
    })?;

    Ok(Some((session, deltas)))
}

// returns last commited file contents from refs/gitbutler/current ref
// if ref doesn't exists, returns file contents from the HEAD repository commit
// returns None if file is not found in either of trees
// returns None if file is not UTF-8
// TODO: handle binary files
fn get_latest_file_contents(
    repo: &git2::Repository,
    project: &projects::Project,
    relative_file_path: &Path,
) -> Result<Option<String>> {
    let tree_entry = match repo.find_reference(&project.refname()) {
        Ok(reference) => {
            let gitbutler_tree_path = &Path::new("wd").join(relative_file_path);
            // "wd/<file_path>" contents from gitbutler HEAD
            reference.peel_to_tree()?.get_path(gitbutler_tree_path)
        }
        Err(e) => {
            if e.code() == git2::ErrorCode::NotFound {
                // "<file_path>" contents from repository HEAD
                repo.head()?.peel_to_tree()?.get_path(relative_file_path)
            } else {
                Err(e)
            }
        }
    };

    match tree_entry {
        Err(e) => {
            if e.code() == git2::ErrorCode::NotFound {
                // file not found in the chosen tree, return None
                Ok(None)
            } else {
                Err(e.into())
            }
        }
        Ok(tree_entry) => {
            let blob = tree_entry.to_object(&repo)?.into_blob().expect(&format!(
                "{}: failed to get blob for {}",
                project.id,
                relative_file_path.display()
            ));

            let text_content = match String::from_utf8(blob.content().to_vec()) {
                Ok(contents) => Some(contents),
                Err(_) => {
                    log::info!(
                        "{}: \"{}\" is not utf-8, ignoring",
                        project.id,
                        relative_file_path.display()
                    );
                    None
                }
            };

            Ok(text_content)
        }
    }
}
