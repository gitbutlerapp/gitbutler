use crate::storage;
use anyhow::Result;
use gix::tempfile::{AutoRemove, ContainingDirectory};
use itertools::Itertools;
use std::{io::Write, path::PathBuf};

use crate::projects::Project;

pub struct SnapshotsReference {
    file_path: PathBuf,
}

impl SnapshotsReference {
    pub fn new(project: &Project, target_sha: &str) -> Result<Self> {
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
            let commit = repo.find_commit(git2::Oid::from_str(target_sha)?)?;
            repo.branch("gitbutler/target", &commit, false)?;
        }

        if !reflog_file_path.exists() {
            return Err(anyhow::anyhow!(
                "Could not create gitbutler/target which is needed for undo snapshotting"
            ));
        }

        Ok(Self {
            file_path: reflog_file_path,
        })
    }

    pub fn set_target_ref(&self, sha: &str) -> Result<()> {
        // 0000000000000000000000000000000000000000 82873b54925ab268e9949557f28d070d388e7774 Kiril Videlov <kiril@videlov.com> 1714037434 +0200       branch: Created from 82873b54925ab268e9949557f28d070d388e7774
        let content = std::fs::read_to_string(&self.file_path)?;
        let mut lines = content.lines().collect::<Vec<_>>();
        let mut first_line = lines[0].split_whitespace().collect_vec();
        let len = first_line.len();
        first_line[1] = sha;
        first_line[len - 1] = sha;
        let binding = first_line.join(" ");
        lines[0] = &binding;
        let content = format!("{}\n", lines.join("\n"));
        write(&self.file_path, &content)
    }

    pub fn set_oplog_ref(&self, sha: &str) -> Result<()> {
        // 82873b54925ab268e9949557f28d070d388e7774 7e8eab472636a26611214bebea7d6b79c971fb8b Kiril Videlov <kiril@videlov.com> 1714044124 +0200    reset: moving to 7e8eab472636a26611214bebea7d6b79c971fb8b
        let content = std::fs::read_to_string(&self.file_path)?;
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
        write(&self.file_path, &content)
    }
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
