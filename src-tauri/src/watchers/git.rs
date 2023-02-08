use crate::{fs, projects::Project};
use filetime::FileTime;
use git2::{IndexTime, Repository};
use sha2::{Digest, Sha256};
use std::{
    fs::{read_to_string, File},
    io::{BufReader, Read},
    os::unix::prelude::MetadataExt,
    path::Path,
    thread,
    time::{Duration, SystemTime},
};

#[derive(Debug)]
pub enum WatchError {
    GitError(git2::Error),
    IOError(std::io::Error),
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchError::GitError(e) => write!(f, "Git error: {}", e),
            WatchError::IOError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl From<git2::Error> for WatchError {
    fn from(error: git2::Error) -> Self {
        Self::GitError(error)
    }
}

impl From<std::io::Error> for WatchError {
    fn from(error: std::io::Error) -> Self {
        Self::IOError(error)
    }
}

const FIVE_MINUTES: u64 = Duration::new(5 * 60, 0).as_secs();
const ONE_HOUR: u64 = Duration::new(60 * 60, 0).as_secs();

pub fn watch(project: Project) -> Result<(), WatchError> {
    let repo = git2::Repository::open(&project.path)?;
    thread::spawn(move || loop {
        match check_for_changes(&repo) {
            Ok(_) => {}
            Err(error) => {
                log::error!(
                    "Error while checking {} for changes: {}",
                    repo.workdir().unwrap().display(),
                    error
                );
            }
        }
        thread::sleep(Duration::from_secs(10));
    });

    Ok(())
}

// main thing called in a loop to check for changes and write our custom commit data
// it will commit only if there are changes and the session is either idle for 5 minutes or is over an hour old
// currently it looks at every file in the wd, but we should probably just look at the ones that have changed when we're certain we can get everything
// - however, it does compare to the git index so we don't actually have to read the contents of every file, so maybe it's not too slow unless in huge repos
// - also only does the file comparison on commit, so it's not too bad
fn check_for_changes(repo: &Repository) -> Result<(), Box<dyn std::error::Error>> {
    if ready_to_commit(repo)? {
        let tree = build_initial_wd_tree(&repo)?;
        let gb_tree = build_gb_tree(tree, &repo)?;

        let commit_oid = write_gb_commit(gb_tree, &repo)?;
        log::debug!(
            "{}: wrote gb commit {}",
            repo.workdir().unwrap().display(),
            commit_oid
        );

        clean_up_session(repo)?;
    }

    Ok(())
    // TODO: try to push the new gb history head to the remote
    // TODO: if we see it is not a FF, pull down the remote, determine order, rewrite the commit line, and push again
}

fn read_timestamp(path: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let raw = read_to_string(path)?;
    let ts = raw.trim().parse::<u64>()?;
    Ok(ts)
}

// make sure that the .git/gb/session directory exists (a session is in progress)
// and that there has been no activity in the last 5 minutes (the session appears to be over)
// and the start was at most an hour ago
fn ready_to_commit(repo: &Repository) -> Result<bool, Box<dyn std::error::Error>> {
    let repo_path = repo.path();
    let last_file = repo_path.join("gb/session/meta/session-last");
    let start_file = repo_path.join("gb/session/meta/session-start");
    if !last_file.exists() {
        log::debug!("{}: no current session", repo_path.display());
        return Ok(false);
    };

    let session_start_ts = read_timestamp(&start_file)?;
    let session_last_ts = read_timestamp(&last_file)?;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u64;

    let elapsed_last = now - session_last_ts;
    let elapsed_start = now - session_start_ts;

    if (elapsed_last > FIVE_MINUTES) || (elapsed_start > ONE_HOUR) {
        Ok(true)
    } else {
        log::debug!(
            "Not ready to commit {} yet. ({} seconds elapsed, {} seconds since start)",
            repo.workdir().unwrap().display(),
            elapsed_last,
            elapsed_start
        );
        Ok(false)
    }
}

// build the initial tree from the working directory, not taking into account the gitbutler metadata
// eventually we might just want to run this once and then update it with the files that are changed over time, but right now we're running it every commit
// it ignores files that are in the .gitignore
fn build_initial_wd_tree(repo: &Repository) -> Result<git2::Oid, Box<dyn std::error::Error>> {
    // create a new in-memory git2 index and open the working one so we can cheat if none of the metadata of an entry has changed
    let wd_index = &mut git2::Index::new()?;
    let repo_index = &mut repo.index()?;

    // add all files in the working directory to the in-memory index, skipping for matching entries in the repo index
    let all_files = fs::list_files(repo.workdir().unwrap())?;
    for file in all_files {
        let file_path = Path::new(&file);
        if !repo.is_path_ignored(&file).unwrap_or(true) {
            add_path(wd_index, repo_index, &file_path, &repo)?;
        }
    }

    // write the in-memory index to the repo
    let tree = wd_index.write_tree_to(&repo)?;
    Ok(tree)
}

// take a file path we see and add it to our in-memory index
// we call this from build_initial_wd_tree, which is smart about using the existing index to avoid rehashing files that haven't changed
// and also looks for large files and puts in a placeholder hash in the LFS format
// TODO: actually upload the file to LFS
fn add_path(
    index: &mut git2::Index,
    repo_index: &mut git2::Index,
    rel_file_path: &Path,
    repo: &Repository,
) -> Result<(), Box<dyn std::error::Error>> {
    let abs_file_path = repo.workdir().unwrap().join(rel_file_path);
    let file_path = Path::new(&abs_file_path);

    let metadata = file_path.metadata()?;
    let mtime = FileTime::from_last_modification_time(&metadata);
    let ctime = FileTime::from_creation_time(&metadata).unwrap();

    // if we find the entry in the index, we can just use it
    match repo_index.get_path(rel_file_path, 0) {
        // if we find the entry and the metadata of the file has not changed, we can just use the existing entry
        Some(entry) => {
            if entry.mtime.seconds() == i32::try_from(mtime.seconds())?
                && entry.mtime.nanoseconds() == u32::try_from(mtime.nanoseconds())?
                && entry.file_size == u32::try_from(metadata.len())?
                && entry.mode == metadata.mode()
            {
                log::debug!("Using existing entry for {}", file_path.display());
                index.add(&entry).unwrap();
                return Ok(());
            }
        }
        None => {
            log::debug!("No entry found for {}", file_path.display());
        }
    };

    // something is different, or not found, so we need to create a new entry

    log::debug!("Adding path: {}", file_path.display());

    // look for files that are bigger than 4GB, which are not supported by git
    // insert a pointer as the blob content instead
    // TODO: size limit should be configurable
    let blob = if metadata.len() > 100_000_000 {
        log::debug!(
            "{}: file too big: {}",
            repo.workdir().unwrap().display(),
            file_path.display()
        );

        // get a sha256 hash of the file first
        let sha = sha256_digest(&file_path)?;

        // put togther a git lfs pointer file: https://github.com/git-lfs/git-lfs/blob/main/docs/spec.md
        let mut lfs_pointer = String::from("version https://git-lfs.github.com/spec/v1\n");
        lfs_pointer.push_str("oid sha256:");
        lfs_pointer.push_str(&sha);
        lfs_pointer.push_str("\n");
        lfs_pointer.push_str("size ");
        lfs_pointer.push_str(&metadata.len().to_string());
        lfs_pointer.push_str("\n");

        // write the file to the .git/lfs/objects directory
        // create the directory recursively if it doesn't exist
        let lfs_objects_dir = repo.path().join("lfs/objects");
        std::fs::create_dir_all(lfs_objects_dir.clone())?;
        let lfs_path = lfs_objects_dir.join(sha);
        std::fs::copy(file_path, lfs_path)?;

        repo.blob(lfs_pointer.as_bytes()).unwrap()
    } else {
        // read the file into a blob, get the object id
        repo.blob_path(&file_path)?
    };

    // create a new IndexEntry from the file metadata
    let new_entry = git2::IndexEntry {
        ctime: IndexTime::new(
            ctime.seconds().try_into().unwrap(),
            ctime.nanoseconds().try_into().unwrap(),
        ),
        mtime: IndexTime::new(
            mtime.seconds().try_into().unwrap(),
            mtime.nanoseconds().try_into().unwrap(),
        ),
        dev: metadata.dev().try_into().unwrap(),
        ino: metadata.ino().try_into().unwrap(),
        mode: metadata.mode(),
        uid: metadata.uid().try_into().unwrap(),
        gid: metadata.gid().try_into().unwrap(),
        file_size: metadata.len().try_into().unwrap(),
        flags: 10, // normal flags for normal file (for the curious: https://git-scm.com/docs/index-format)
        flags_extended: 0, // no extended flags
        path: file_path.to_str().unwrap().to_string().into(),
        id: blob,
    };

    index.add(&new_entry)?;
    Ok(())
}

/// calculates sha256 digest of a large file as lowercase hex string via streaming buffer
/// used to calculate the hash of large files that are not supported by git
fn sha256_digest(path: &Path) -> Result<String, std::io::Error> {
    let input = File::open(path)?;
    let mut reader = BufReader::new(input);

    let digest = {
        let mut hasher = Sha256::new();
        let mut buffer = [0; 1024];
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        hasher.finalize()
    };
    Ok(format!("{:X}", digest))
}

// this builds the tree that we're going to link to from our commit.
// it has two entries, wd and session:
// - wd: the tree that is the working directory recorded at the end of the session
// - session/deltas: the tree that contains the crdt data for each file that changed during the session
// - session/meta: some metadata values like starting time, last touched time, branch, etc
// returns a tree Oid that can be used to create a commit
fn build_gb_tree(
    tree: git2::Oid,
    repo: &Repository,
) -> Result<git2::Oid, Box<dyn std::error::Error>> {
    // create a new, awesome tree with TreeBuilder
    let mut tree_builder = repo.treebuilder(None)?;

    // insert the tree oid as a subdirectory under the name 'wd'
    tree_builder.insert("wd", tree, 0o040000)?;

    // create a new in-memory git2 index and fill it with the contents of .git/gb/session
    let session_index = &mut git2::Index::new()?;

    // add all files in the working directory to the in-memory index, skipping for matching entries in the repo index
    let session_dir = repo.path().join("gb/session");
    for path in fs::list_files(&session_dir)? {
        let file_path = Path::new(&path);
        add_simple_path(&repo, session_index, &file_path)?;
    }

    // write the in-memory index to the repo
    let session_tree = session_index.write_tree_to(&repo).unwrap();

    // insert the session tree oid as a subdirectory under the name 'session'
    tree_builder
        .insert("session", session_tree, 0o040000)
        .unwrap();

    // write the new tree and return the Oid
    let tree = tree_builder.write().unwrap();
    Ok(tree)
}

// this is a helper function for build_gb_tree that takes paths under .git/gb/session and adds them to the in-memory index
fn add_simple_path(
    repo: &Repository,
    index: &mut git2::Index,
    rel_file_path: &Path,
) -> Result<(), git2::Error> {
    let abs_file_path = repo.workdir().unwrap().join(rel_file_path);
    let file_path = Path::new(&abs_file_path);

    log::debug!("Adding path: {}", file_path.display());

    let blob = repo.blob_path(file_path).unwrap();
    let metadata = file_path.metadata().unwrap();
    let mtime = FileTime::from_last_modification_time(&metadata);
    let ctime = FileTime::from_creation_time(&metadata).unwrap();

    // create a new IndexEntry from the file metadata
    let new_entry = git2::IndexEntry {
        ctime: IndexTime::new(
            ctime
                .seconds()
                .try_into()
                .map_err(|_| git2::Error::from_str("ctime seconds out of range"))?,
            ctime
                .nanoseconds()
                .try_into()
                .map_err(|_| git2::Error::from_str("ctime nanoseconds out of range"))?,
        ),
        mtime: IndexTime::new(
            mtime
                .seconds()
                .try_into()
                .map_err(|_| git2::Error::from_str("mtime seconds out of range"))?,
            mtime
                .nanoseconds()
                .try_into()
                .map_err(|_| git2::Error::from_str("mtime nanoseconds out of range"))?,
        ),
        dev: metadata.dev().try_into().unwrap(),
        ino: metadata.ino().try_into().unwrap(),
        mode: metadata.mode(),
        uid: metadata.uid().try_into().unwrap(),
        gid: metadata.gid().try_into().unwrap(),
        file_size: metadata.len().try_into().unwrap(),
        flags: 10, // normal flags for normal file (for the curious: https://git-scm.com/docs/index-format)
        flags_extended: 0, // no extended flags
        path: file_path.to_str().unwrap().into(),
        id: blob,
    };

    index.add(&new_entry)?;

    Ok(())
}

// write a new commit object to the repo
// this is called once we have a tree of deltas, metadata and current wd snapshot
// and either creates or updates the refs/gitbutler/current ref
fn write_gb_commit(gb_tree: git2::Oid, repo: &Repository) -> Result<git2::Oid, git2::Error> {
    // find the Oid of the commit that refs/gitbutler/current points to, none if it doesn't exist
    match repo.revparse_single("refs/gitbutler/current") {
        Ok(obj) => {
            let last_commit = repo.find_commit(obj.id()).unwrap();
            let new_commit = repo.commit(
                Some("refs/gitbutler/current"),
                &repo.signature().unwrap(),        // author
                &repo.signature().unwrap(),        // committer
                "gitbutler check",                 // commit message
                &repo.find_tree(gb_tree).unwrap(), // tree
                &[&last_commit],                   // parents
            )?;
            Ok(new_commit)
        }
        Err(_) => {
            let new_commit = repo.commit(
                Some("refs/gitbutler/current"),
                &repo.signature().unwrap(),        // author
                &repo.signature().unwrap(),        // committer
                "gitbutler check",                 // commit message
                &repo.find_tree(gb_tree).unwrap(), // tree
                &[],                               // parents
            )?;
            Ok(new_commit)
        }
    }
}

// this stops the current session by deleting the .git/gb/session directory
fn clean_up_session(repo: &git2::Repository) -> Result<(), std::io::Error> {
    // delete the .git/gb/session directory
    let session_path = repo.path().join("gb/session");
    std::fs::remove_dir_all(session_path)?;
    Ok(())
}
