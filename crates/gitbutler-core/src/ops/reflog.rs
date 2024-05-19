use crate::fs::write;
use anyhow::Result;
use itertools::Itertools;
use std::path::PathBuf;

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
    write(file_path, content)
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
    write(file_path, content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_set_target_ref() {
        let (dir, commit_id) = setup_repo();
        let project = Project {
            path: dir.path().to_path_buf(),
            ..Default::default()
        };

        let log_file_path = dir
            .path()
            .join(".git")
            .join("logs")
            .join("refs")
            .join("heads")
            .join("gitbutler")
            .join("target");
        assert!(!log_file_path.exists());

        // Set ref for the first time
        assert!(set_reference_to_oplog(&project, &commit_id.to_string(), "oplog_sha").is_ok());
        assert!(log_file_path.exists());
        let log_file = std::fs::read_to_string(&log_file_path).unwrap();
        let log_lines = log_file.lines().collect::<Vec<_>>();
        assert_eq!(log_lines.len(), 2);
        assert!(log_lines[0].starts_with(&format!(
            "0000000000000000000000000000000000000000 {}",
            commit_id
        )));
        assert!(log_lines[0].ends_with(&format!("branch: Created from {}", commit_id)));
        assert!(log_lines[1].starts_with(&format!("{} {}", commit_id, "oplog_sha")));
        assert!(log_lines[1].ends_with("reset: moving to oplog_sha"));

        // Update the oplog head only
        assert!(
            set_reference_to_oplog(&project, &commit_id.to_string(), "another_oplog_sha").is_ok()
        );
        let log_file = std::fs::read_to_string(&log_file_path).unwrap();
        let log_lines = log_file.lines().collect::<Vec<_>>();
        assert_eq!(log_lines.len(), 2);
        assert!(log_lines[0].starts_with(&format!(
            "0000000000000000000000000000000000000000 {}",
            commit_id
        )));
        assert!(log_lines[0].ends_with(&format!("branch: Created from {}", commit_id)));
        println!("{:?}", log_lines[1]);
        assert!(log_lines[1].starts_with(&format!("{} {}", commit_id, "another_oplog_sha")));
        assert!(log_lines[1].ends_with("reset: moving to another_oplog_sha"));

        // Update the target head only
        assert!(set_reference_to_oplog(&project, "new_target", "another_oplog_sha").is_ok());
        let log_file = std::fs::read_to_string(&log_file_path).unwrap();
        let log_lines = log_file.lines().collect::<Vec<_>>();
        assert_eq!(log_lines.len(), 2);
        assert!(log_lines[0].starts_with(&format!(
            "0000000000000000000000000000000000000000 {}",
            "new_target"
        )));
        assert!(log_lines[0].ends_with(&format!("branch: Created from {}", "new_target")));
        println!("{:?}", log_lines[1]);
        assert!(log_lines[1].starts_with(&format!("{} {}", "new_target", "another_oplog_sha")));
        assert!(log_lines[1].ends_with("reset: moving to another_oplog_sha"));
    }

    fn setup_repo() -> (tempfile::TempDir, git2::Oid) {
        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let file_path = dir.path().join("foo.txt");
        std::fs::write(file_path, "test").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(&PathBuf::from("foo.txt")).unwrap();
        let oid = index.write_tree().unwrap();
        let name = "Your Name";
        let email = "your.email@example.com";
        let signature = git2::Signature::now(name, email).unwrap();
        let commit_id = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "initial commit",
                &repo.find_tree(oid).unwrap(),
                &[],
            )
            .unwrap();
        (dir, commit_id)
    }
}
