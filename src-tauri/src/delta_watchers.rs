use crate::crdt::TextDocument;
use crate::projects::Project;
use futures::{
    channel::mpsc::{channel, Receiver},
    SinkExt, StreamExt,
};
use git2::{Commit, Repository};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
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
    // update meta files every time file change is detected
    write_beginning_meta_files(&repo);

    for file_path in files {
        if !file_path.is_file() {
            // only handle file changes
            continue;
        }

        let relative_file_path = Path::new(file_path.strip_prefix(&project.path).unwrap());
        if repo.is_path_ignored(&relative_file_path).unwrap_or(true) {
            // make sure we're not watching ignored files
            continue;
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
        let commit_blob = if let Ok(object) = tree.get_path(Path::new(&relative_file_path)) {
            // if file found, check if delta file exists
            let blob = object.to_object(&repo).unwrap().into_blob().unwrap();
            let contents = String::from_utf8(blob.content().to_vec()).unwrap();
            Some(contents)
        } else {
            None
        };

        // second, get non-flushed file deltas
        let deltas = project.get_file_deltas(Path::new(&relative_file_path));

        // depending on the above, we can create TextDocument
        let mut text_doc = match (commit_blob, deltas) {
            (Some(contents), Some(deltas)) => {
                TextDocument::new(&contents, deltas)
            }
            (Some(contents), None) => {
                TextDocument::new(&contents, vec![])
            }
            (None, Some(deltas)) => {
                TextDocument::from_deltas(deltas)
            }
            (None, None) => {
                TextDocument::from_deltas(vec![])
            }
        };

        // update the TextDocument with the new file contents
        let contents = std::fs::read_to_string(file_path.clone())
            .expect(format!("Failed to read {}", file_path.to_str().unwrap()).as_str());

        if text_doc.update(&contents) {
            // if the file was modified, save the deltas
            let deltas = text_doc.get_deltas();
            project.save_file_deltas(relative_file_path, deltas);
        }
    }
}

// get commit from refs/gitbutler/current or fall back to HEAD
fn get_meta_commit(repo: &Repository) -> Commit {
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
