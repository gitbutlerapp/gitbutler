use crate::storage;
use anyhow::Result;
use gix::tempfile::{AutoRemove, ContainingDirectory};
use itertools::Itertools;
use std::{io::Write, path::PathBuf};

use crate::projects::Project;

/// Sets a reference to the oplog head commit such that snapshots are reachable and will not be garbage collected.
/// We want to achieve 2 things:
///  - The oplog must not be visible in `git log --all` as branch
///  - The oplog tree must not be garbage collected (i.e. it must be reachable)
///
/// This needs to be invoked whenever the target head or the oplog head change.
///
/// How it works:
/// First a reference gitbutler/target is created, pointing to the head of the target (trunk) branch. This is a fake branch that we don't need to care about. If it doesn't exist, it is created.
/// Then in the reflog entry logs/refs/heads/gitbutler/target we pretend that the the ref originally pointed to the oplog head commit like so:
///
/// 0000000000000000000000000000000000000000 <target branch head sha>
/// <target branch head sha>                 <oplog head sha>
///
/// The reflog entry is continuously updated to refer to the current target and oplog head commits.
pub fn set_reference_to_oplog(
    project: &Project,
    target_head_sha: &str,
    oplog_head_sha: &str,
) -> Result<()> {
    let repo_path = project.path.as_path();
    let reflog_file_path = repo_path
        .join(".git")
        .join("logs")
        .join("refs")
        .join("heads")
        .join("gitbutler")
        .join("target");

    if !reflog_file_path.exists() {
        let repo = git2::Repository::init(repo_path)?;
        let commit = repo.find_commit(git2::Oid::from_str(target_head_sha)?)?;
        repo.branch("gitbutler/target", &commit, false)?;
    }

    if !reflog_file_path.exists() {
        return Err(anyhow::anyhow!(
            "Could not create gitbutler/target which is needed for undo snapshotting"
        ));
    }

    set_target_ref(&reflog_file_path, target_head_sha)?;
    set_oplog_ref(&reflog_file_path, oplog_head_sha)?;

    Ok(())
}

fn set_target_ref(file_path: &PathBuf, sha: &str) -> Result<()> {
    // 0000000000000000000000000000000000000000 82873b54925ab268e9949557f28d070d388e7774 Kiril Videlov <kiril@videlov.com> 1714037434 +0200       branch: Created from 82873b54925ab268e9949557f28d070d388e7774
    let content = std::fs::read_to_string(file_path)?;
    let mut lines = content.lines().collect::<Vec<_>>();
    let mut first_line = lines[0].split_whitespace().collect_vec();
    let len = first_line.len();
    first_line[1] = sha;
    first_line[len - 1] = sha;
    let binding = first_line.join(" ");
    lines[0] = &binding;
    let content = format!("{}\n", lines.join("\n"));
    write(file_path, &content)
}

fn set_oplog_ref(file_path: &PathBuf, sha: &str) -> Result<()> {
    // 82873b54925ab268e9949557f28d070d388e7774 7e8eab472636a26611214bebea7d6b79c971fb8b Kiril Videlov <kiril@videlov.com> 1714044124 +0200    reset: moving to 7e8eab472636a26611214bebea7d6b79c971fb8b
    let content = std::fs::read_to_string(file_path)?;
    let first_line = content.lines().collect::<Vec<_>>().remove(0);

    let target_ref = first_line.split_whitespace().collect_vec()[1];
    let the_rest = first_line.split_whitespace().collect_vec()[2..].join(" ");
    let the_rest = the_rest.replace("branch", "   reset");
    let mut the_rest_split = the_rest.split(':').collect_vec();
    let new_msg = format!(" moving to {}", sha);
    the_rest_split[1] = &new_msg;
    let the_rest = the_rest_split.join(":");

    let second_line = [target_ref, sha, &the_rest].join(" ");

    let content = format!("{}\n", [first_line, &second_line].join("\n"));
    write(file_path, &content)
}

fn write(file_path: &PathBuf, content: &str) -> Result<()> {
    let mut temp_file = gix::tempfile::new(
        file_path.parent().unwrap(),
        ContainingDirectory::Exists,
        AutoRemove::Tempfile,
    )?;
    temp_file.write_all(content.as_bytes())?;
    storage::persist_tempfile(temp_file, file_path)?;

    Ok(())
}
