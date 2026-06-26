//! Integration coverage for the in-process `grit-lib` transport backend, using
//! a `file://` remote so no network or authentication is involved.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A self-cleaning temporary directory under the system temp dir.
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "gitbutler-grit-test-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("create temp dir");
        Self(base)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

/// Create a source repository with a single commit on `main` containing
/// `hello.txt`, and return its path.
fn make_source(root: &Path) -> PathBuf {
    let src = root.join("source");
    std::fs::create_dir_all(&src).unwrap();
    git(&src, &["init", "-q", "-b", "main"]);
    std::fs::write(src.join("hello.txt"), b"hello from grit\n").unwrap();
    std::fs::create_dir_all(src.join("nested")).unwrap();
    std::fs::write(src.join("nested/file.txt"), b"nested\n").unwrap();
    git(&src, &["add", "."]);
    git(&src, &["commit", "-q", "-m", "initial"]);
    src
}

fn file_url(path: &Path) -> String {
    format!("file://{}", path.display())
}

// cargo test --package gitbutler-git --test git grit_clone_local
#[test]
fn grit_clone_local() {
    let tmp = TempDir::new("clone");
    let src = make_source(tmp.path());
    let dest = tmp.path().join("dest");

    gitbutler_git::grit::clone(&file_url(&src), &dest).expect("grit clone");

    // The working tree was checked out.
    assert_eq!(
        std::fs::read_to_string(dest.join("hello.txt")).unwrap(),
        "hello from grit\n"
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("nested/file.txt")).unwrap(),
        "nested\n"
    );

    // HEAD points at the remote's default branch and an index was written.
    let head = std::fs::read_to_string(dest.join(".git/HEAD")).unwrap();
    assert_eq!(head.trim(), "ref: refs/heads/main");
    assert!(dest.join(".git/index").exists(), "index was written");

    // The clone is a valid repository with a clean status (via real git).
    let status = Command::new("git")
        .current_dir(&dest)
        .args(["status", "--porcelain"])
        .output()
        .expect("git status");
    assert!(status.status.success());
    assert!(
        status.stdout.is_empty(),
        "expected clean status, got: {}",
        String::from_utf8_lossy(&status.stdout)
    );
}

// cargo test --package gitbutler-git --test git grit_fetch_local
#[test]
fn grit_fetch_local() {
    let tmp = TempDir::new("fetch");
    let src = make_source(tmp.path());

    // A destination repo with `origin` pointing at the source via file://.
    let dest = tmp.path().join("dest");
    std::fs::create_dir_all(&dest).unwrap();
    git(&dest, &["init", "-q", "-b", "main"]);
    git(&dest, &["remote", "add", "origin", &file_url(&src)]);

    let repo = gix::open(&dest).expect("open dest");
    gitbutler_git::grit::fetch(&repo, "origin").expect("grit fetch");

    // The remote-tracking ref now exists and matches the source's `main`.
    let src_main = Command::new("git")
        .current_dir(&src)
        .args(["rev-parse", "main"])
        .output()
        .unwrap();
    let src_oid = String::from_utf8_lossy(&src_main.stdout).trim().to_owned();

    let tracked = Command::new("git")
        .current_dir(&dest)
        .args(["rev-parse", "refs/remotes/origin/main"])
        .output()
        .unwrap();
    assert!(tracked.status.success(), "tracking ref should exist");
    assert_eq!(String::from_utf8_lossy(&tracked.stdout).trim(), src_oid);
}
