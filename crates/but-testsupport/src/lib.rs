//! Utilities for testing.
#![deny(rust_2018_idioms, missing_docs)]

use gix::bstr::{BStr, ByteSlice};
use gix::config::tree::Key;
pub use gix_testtools;
use std::path::Path;

/// Choose a slightly more obvious, yet easy to type syntax than a function with 4 parameters.
/// i.e. `hunk_header("-1,10", "+1,10")`.
/// Returns `( (old_start, old_lines), (new_start, new_lines) )`.
pub fn hunk_header(old: &str, new: &str) -> ((u32, u32), (u32, u32)) {
    fn parse_header(hunk_info: &str) -> (u32, u32) {
        let hunk_info = hunk_info.trim_start_matches(['-', '+'].as_slice());
        let parts: Vec<_> = hunk_info.split(',').collect();
        let start = parts[0].parse().unwrap();
        let lines = if parts.len() > 1 {
            parts[1].parse().unwrap()
        } else {
            1
        };
        (start, lines)
    }
    (parse_header(old), parse_header(new))
}

/// While `gix` can't (or can't conveniently) do everything, let's make using `git` easier.
pub fn git(repo: &gix::Repository) -> std::process::Command {
    let mut cmd = std::process::Command::new(gix::path::env::exe_invocation());
    cmd.current_dir(repo.workdir().expect("non-bare"));
    cmd
}

/// Open a repository at `path` suitable for testing which means that:
///
/// * author and committer are configured, as well as a stable time.
/// * it's isolated and won't load environment variables.
/// * an object cache is set for minor speed boost.
pub fn open_repo(path: &Path) -> anyhow::Result<gix::Repository> {
    let mut repo = gix::open_opts(
        path,
        gix::open::Options::isolated()
            .lossy_config(false)
            .config_overrides([
                gix::config::tree::Author::NAME
                    .validated_assignment("Author (Memory Override)".into())?,
                gix::config::tree::Author::EMAIL
                    .validated_assignment("author@example.com".into())?,
                gix::config::tree::Committer::NAME
                    .validated_assignment("Committer (Memory Override)".into())?,
                gix::config::tree::Committer::EMAIL
                    .validated_assignment("committer@example.com".into())?,
                gix::config::tree::gitoxide::Commit::COMMITTER_DATE
                    .validated_assignment("2000-01-01 00:00:00 +0000".into())?,
            ]),
    )?;
    repo.object_cache_size_if_unset(512 * 1024);
    Ok(repo)
}

/// Sets and environment that assures commits are reproducible.
/// This needs the `testing` feature enabled in `but-core` as well to work.
/// This changes the process environment, be aware.
pub fn assure_stable_env() {
    let env = gix_testtools::Env::new()
        // TODO(gix): once everything is ported, the only variable needed here
        //            is CHANGE_ID, and even that could be a global. Call `but_testsupport::open_repo()`
        //            for basic settings.
        .set("GIT_AUTHOR_DATE", "2000-01-01 00:00:00 +0000")
        .set("GIT_AUTHOR_EMAIL", "author@example.com")
        .set("GIT_AUTHOR_NAME", "author (From Env)")
        .set("GIT_COMMITTER_DATE", "2000-01-02 00:00:00 +0000")
        .set("GIT_COMMITTER_EMAIL", "committer@example.com")
        .set("GIT_COMMITTER_NAME", "committer (From Env)")
        .set("CHANGE_ID", "change-id");
    // assure it doesn't get racy.
    std::mem::forget(env);
}

/// Utilities for the [`git()`] command.
pub trait CommandExt {
    /// Run the command successfully or print panic with all available command output.
    fn run(&mut self);
}

impl CommandExt for std::process::Command {
    fn run(&mut self) {
        let out = self.output().expect("Can execute well-known command");
        assert!(
            out.status.success(),
            "{self:?} failed: {}\n\n{}",
            out.stdout.as_bstr(),
            out.stderr.as_bstr()
        );
    }
}

/// Produce a graph of all commits reachable from `refspec`.
pub fn visualize_commit_graph(
    repo: &gix::Repository,
    refspec: impl ToString,
) -> std::io::Result<String> {
    let log = git(repo)
        .args(["log", "--oneline", "--graph", "--decorate"])
        .arg(refspec.to_string())
        .output()?;
    assert!(log.status.success());
    Ok(log.stdout.to_str().expect("no illformed UTF-8").to_string())
}

/// Run a condensed status on `repo`.
pub fn git_status(repo: &gix::Repository) -> std::io::Result<String> {
    let out = git(repo).args(["status", "--porcelain"]).output()?;
    assert!(out.status.success(), "STDERR: {}", out.stderr.as_bstr());
    Ok(out.stdout.to_str().expect("no illformed UTF-8").to_string())
}

/// Display a Git tree in the style of the `tree` CLI program, but include blob contents and usful Git metadata.
pub fn visualize_tree(tree_id: gix::Id<'_>) -> termtree::Tree<String> {
    fn visualize_tree(
        id: gix::Id<'_>,
        name_and_mode: Option<(&BStr, gix::object::tree::EntryMode)>,
    ) -> anyhow::Result<termtree::Tree<String>> {
        fn short_id(id: &gix::hash::oid) -> String {
            id.to_hex_with_len(7).to_string()
        }
        let repo = id.repo;
        let entry_name =
            |id: &gix::hash::oid, name: Option<(&BStr, gix::object::tree::EntryMode)>| -> String {
                match name {
                    None => short_id(id),
                    Some((name, mode)) => {
                        format!(
                            "{name}:{mode}{} {}",
                            short_id(id),
                            match repo.find_blob(id) {
                                Ok(blob) => format!("{:?}", blob.data.as_bstr()),
                                Err(_) => "".into(),
                            },
                            mode = if mode.is_tree() {
                                "".into()
                            } else {
                                format!("{:o}:", mode.0)
                            }
                        )
                    }
                }
            };

        let mut tree = termtree::Tree::new(entry_name(&id, name_and_mode));
        for entry in repo.find_tree(id)?.iter() {
            let entry = entry?;
            if entry.mode().is_tree() {
                tree.push(visualize_tree(
                    entry.id(),
                    Some((entry.filename(), entry.mode())),
                )?);
            } else {
                tree.push(entry_name(
                    entry.oid(),
                    Some((entry.filename(), entry.mode())),
                ));
            }
        }
        Ok(tree)
    }
    visualize_tree(tree_id.object().unwrap().peel_to_tree().unwrap().id(), None).unwrap()
}

/// Visualize a tree on disk with mode information.
/// For convenience, skip `.git` and don't display the root.
///
/// # IMPORTANT: Portability
///
/// * As it's intended for tests, this can't be called on Windows were modes don't exist.
///   Further, be sure to set the `umask` of the process to something explicit, or else it may differ
///   between runs and cause failures.
/// * To avoid umask-specific errors across different systems, which may or may not use it for all operations,
///   we 'normalize' umasks to what Git would track. This normalisation may need adjustments as different systems
///   are encountered.
#[cfg(unix)]
pub fn visualize_disk_tree_skip_dot_git(root: &Path) -> anyhow::Result<termtree::Tree<String>> {
    use std::os::unix::fs::MetadataExt;
    fn normalize_mode(mode: u32) -> u32 {
        match mode {
            0o40777 => 0o40755,
            0o100666 => 0o100644,
            0o100777 => 0o100755,
            0o120777 => 0o120755,
            other => other,
        }
    }
    fn label(p: &Path, md: &std::fs::Metadata) -> String {
        format!(
            "{name}:{mode:o}",
            name = p.file_name().unwrap().to_str().unwrap(),
            mode = normalize_mode(md.mode()),
        )
    }

    fn tree(p: &Path, show_label: bool) -> std::io::Result<termtree::Tree<String>> {
        let mut cur = termtree::Tree::new(if show_label {
            label(p, &p.symlink_metadata()?)
        } else {
            ".".into()
        });

        let mut entries: Vec<_> = std::fs::read_dir(p)?.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let md = entry.metadata()?;
            if md.is_dir() && entry.file_name() != ".git" {
                cur.push(tree(&entry.path(), true)?);
            } else {
                cur.push(termtree::Tree::new(label(&entry.path(), &md)));
            }
        }
        Ok(cur)
    }

    Ok(tree(root, false)?)
}

/// Windows dummy
#[cfg(not(unix))]
pub fn visualize_disk_tree_skip_dot_git(_root: &Path) -> anyhow::Result<termtree::Tree<String>> {
    anyhow::bail!("BUG: must not run on Windows - results won't be desirable");
}
