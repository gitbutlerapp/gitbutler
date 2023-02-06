use futures::{
    channel::mpsc::{channel, Receiver},
    SinkExt, StreamExt,
};
use git2::Repository;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use path_absolutize::*;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::projects::Project;

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
            Err(e) => println!("watch error: {:?}", e),
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
    // lol, holy shit rust.
    // this takes the root path and turns it into an absolute path string, so "./" becomes "/User/schacon/Projects/blah"
    let mut pre_path = PathBuf::from(&project.path)
        .absolutize()
        .unwrap()
        .to_path_buf()
        .into_os_string()
        .into_string()
        .unwrap();
    pre_path.push('/');

    // turn the paths into relative paths by stripping the pre_path from each one
    // so /User/schacon/Projects/blah/foo/bar.txt becomes foo/bar.txt
    let files_rel: Vec<String> = files
        .iter()
        .map(|p| {
            p.to_str()
                .unwrap()
                .strip_prefix(&pre_path)
                .unwrap()
                .to_string()
        })
        .collect();

    let entry = files.first().unwrap();
    if !repo.is_path_ignored(entry).unwrap_or(true) {
        let file_path = files_rel.first().unwrap();
        let delta_path_str = String::from(".git/gb/session/deltas/") + file_path;
        let delta_path = Path::new(&delta_path_str);
        let prefix = delta_path.parent().unwrap();
        std::fs::create_dir_all(prefix).unwrap();

        // TODO: Actually write delta data
        if EventKind::is_modify(&kind) {
            log::info!("[{}] File modified: {:?}", project.path, entry);
            // write string "delta" to file at .git/gb/session/deltas/path/to/file
            if entry.is_file() {
                let mut file = File::create(delta_path).unwrap();
                file.write_all(b"delta").unwrap();
            }
        } else if EventKind::is_create(&kind) {
            // not doing anything here right now, because modified will be called right after
            log::info!("[{}] File created: {:?}", project.path, entry);
        } else if EventKind::is_remove(&kind) {
            log::info!("[{}] File removed: {:?}", project.path, entry);
            // i assume we'll still just write the crdt with a delete thing
            if entry.is_file() {
                let mut file = File::create(delta_path).unwrap();
                file.write_all(b"delta-delete").unwrap();
            }
        }

        write_beginning_meta_files(&repo);
    }
}

// this function is called when the user modifies a file, it writes starting metadata if not there
// and also touches the last activity timestamp, so we can tell when we are idle
fn write_beginning_meta_files(repo: &Repository) {
    // check if the file .git/gb/meta/start exists and if not, write the current timestamp into it
    let meta_path_str = String::from(".git/gb/session/meta/session-start");
    let meta_path = Path::new(&meta_path_str);

    // create the parent directory recurisvely if it doesn't exist
    let parent = meta_path.parent().unwrap();
    std::fs::create_dir_all(parent).unwrap();

    if !meta_path.exists() {
        let mut file = File::create(meta_path).unwrap();
        file.write_all(chrono::Local::now().timestamp().to_string().as_bytes())
            .unwrap();
    }

    // check if the file .git/gb/session/meta/branch exists and if not, write the current branch name into it
    let meta_path_str = String::from(".git/gb/session/meta/branch");
    let meta_path = Path::new(&meta_path_str);
    if !meta_path.exists() {
        let mut file = File::create(meta_path).unwrap();
        let branch = repo.head().unwrap();
        let branch_name = branch.name().unwrap();
        file.write_all(branch_name.as_bytes()).unwrap();
    }

    // check if the file .git/gb/session/meta/commit exists and if not, write the current commit hash into it
    let meta_path_str = String::from(".git/gb/session/meta/commit");
    let meta_path = Path::new(&meta_path_str);
    if !meta_path.exists() {
        let mut file = File::create(meta_path).unwrap();
        let commit = repo.head().unwrap().peel_to_commit().unwrap();
        file.write_all(commit.id().to_string().as_bytes()).unwrap();
    }

    // ALWAYS write the last time we did this
    let meta_path_str = String::from(".git/gb/session/meta/session-last");
    let meta_path = Path::new(&meta_path_str);
    let mut file = File::create(meta_path).unwrap();
    file.write_all(chrono::Local::now().timestamp().to_string().as_bytes())
        .unwrap();
}
