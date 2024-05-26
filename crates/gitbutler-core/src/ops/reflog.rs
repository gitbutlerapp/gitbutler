use crate::fs::write;
use crate::git;
use anyhow::{Context, Result};
use gix::config::tree::Key;
use std::path::Path;

use crate::virtual_branches::integration::{
    GITBUTLER_INTEGRATION_COMMIT_AUTHOR_EMAIL, GITBUTLER_INTEGRATION_COMMIT_AUTHOR_NAME,
};

/// Sets a reference to the oplog head commit such that snapshots are reachable and will not be garbage collected.
/// We want to achieve 2 things:
///  - The oplog must not be visible in `git log --all` as branch
///  - The oplog tree must not be garbage collected (i.e. it must be reachable)
///
/// This needs to be invoked whenever the target head or the oplog head change.
///
/// How it works:
/// First a reference gitbutler/target is created, pointing to the head of the target (trunk) branch.
/// This is a fake branch that we don't need to care about. If it doesn't exist, it is created.
/// Then in the reflog entry logs/refs/heads/gitbutler/target we pretend that the ref originally pointed to the
/// oplog head commit like so:
///
/// 0000000000000000000000000000000000000000 <target branch head sha>
/// <target branch head sha>                 <oplog head sha>
///
/// The reflog entry is continuously updated to refer to the current target and oplog head commits.
pub(super) fn set_reference_to_oplog(
    worktree_dir: &Path,
    target_head_sha: git::Oid,
    oplog_head_sha: git::Oid,
) -> Result<()> {
    let reflog_file_path = worktree_dir
        .join(".git")
        .join("logs")
        .join("refs")
        .join("heads")
        .join("gitbutler")
        .join("target");

    let mut repo = gix::open_opts(
        worktree_dir,
        // We may override the username as we only write a specific commit log, unrelated to the user.
        gix::open::Options::isolated().config_overrides([
            gix::config::tree::User::NAME
                .validated_assignment(GITBUTLER_INTEGRATION_COMMIT_AUTHOR_NAME.into())?,
            gix::config::tree::User::EMAIL
                .validated_assignment(GITBUTLER_INTEGRATION_COMMIT_AUTHOR_EMAIL.into())?,
        ]),
    )?;
    // The check is here only to avoid unnecessary writes
    if repo.try_find_reference("gitbutler/target")?.is_none() {
        repo.refs.write_reflog = gix::refs::store::WriteReflog::Always;
        repo.reference(
            "refs/heads/gitbutler/target",
            target_head_sha.to_string().parse::<gix::ObjectId>()?,
            gix::refs::transaction::PreviousValue::Any,
            format!("branch: Created from {target_head_sha}"),
        )?;
    }

    let mut content = std::fs::read_to_string(&reflog_file_path)
        .context("A reflog for gitbutler/target which is needed for undo snapshotting")?;
    content = set_target_ref(&content, &target_head_sha.to_string()).with_context(|| {
        format!(
            "Something was wrong with \"{}\"",
            reflog_file_path.display()
        )
    })?;
    content = set_oplog_ref(&content, &oplog_head_sha.to_string())?;
    write(reflog_file_path, content)?;

    Ok(())
}

fn set_target_ref(content: &str, sha: &str) -> Result<String> {
    // 0000000000000000000000000000000000000000 82873b54925ab268e9949557f28d070d388e7774 Kiril Videlov <kiril@videlov.com> 1714037434 +0200\tbranch: Created from 82873b54925ab268e9949557f28d070d388e7774
    let mut lines = gix::refs::file::log::iter::forward(content.as_bytes());
    let mut first_line = lines.next().context("need the creation-line in reflog")??;

    first_line.new_oid = sha.into();
    let message = format!("branch: Created from {sha}");
    first_line.message = message.as_str().into();

    Ok(serialize_line(first_line))
}

fn set_oplog_ref(content: &str, sha: &str) -> Result<String> {
    // 82873b54925ab268e9949557f28d070d388e7774 7e8eab472636a26611214bebea7d6b79c971fb8b Kiril Videlov <kiril@videlov.com> 1714044124 +0200\treset: moving to 7e8eab472636a26611214bebea7d6b79c971fb8b
    let mut lines = gix::refs::file::log::iter::forward(content.as_bytes());
    let first_line = lines.next().context("need the creation-line in reflog")??;

    let new_msg = format!("reset: moving to {}", sha);
    let mut second_line = first_line.clone();
    second_line.previous_oid = first_line.new_oid;
    second_line.new_oid = sha.into();
    second_line.message = new_msg.as_str().into();

    Ok(format!(
        "{}\n{}\n",
        serialize_line(first_line),
        serialize_line(second_line)
    ))
}

fn serialize_line(line: gix::refs::file::log::LineRef<'_>) -> String {
    let mut sig = Vec::new();
    line.signature
        .write_to(&mut sig)
        .expect("write to memory succeeds");

    format!(
        "{} {} {}\t{}",
        line.previous_oid,
        line.new_oid,
        std::str::from_utf8(&sig).expect("no illformed UTF8"),
        line.message
    )
}

#[cfg(test)]
mod set_target_ref {
    use super::{
        git, set_reference_to_oplog, GITBUTLER_INTEGRATION_COMMIT_AUTHOR_EMAIL,
        GITBUTLER_INTEGRATION_COMMIT_AUTHOR_NAME,
    };
    use gix::refs::file::log::LineRef;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;
    use std::str::FromStr;
    use tempfile::tempdir;

    #[test]
    fn reflog_present_but_branch_missing_recreates_branch() -> anyhow::Result<()> {
        let (dir, commit_id) = setup_repo()?;
        let worktree_dir = dir.path();

        let oplog_sha = git::Oid::from_str("0123456789abcdef0123456789abcdef0123456")?;
        set_reference_to_oplog(&worktree_dir, commit_id.into(), oplog_sha).expect("success");

        let loose_ref_file = worktree_dir.join(".git/refs/heads/gitbutler/target");
        std::fs::remove_file(&loose_ref_file)?;

        set_reference_to_oplog(&worktree_dir, commit_id.into(), oplog_sha).expect("success");
        assert!(
            loose_ref_file.is_file(),
            "the file was recreated, just in case there is only a reflog and no branch"
        );
        Ok(())
    }

    #[test]
    fn new_and_update() -> anyhow::Result<()> {
        let (dir, commit_id) = setup_repo()?;
        let worktree_dir = dir.path();

        let log_file_path = worktree_dir.join(".git/logs/refs/heads/gitbutler/target");
        assert!(!log_file_path.exists());

        // Set ref for the first time
        let oplog_sha = git::Oid::from_str("0123456789abcdef0123456789abcdef0123456")?;
        set_reference_to_oplog(&worktree_dir, commit_id.into(), oplog_sha).expect("success");
        assert!(log_file_path.exists());
        let contents = std::fs::read_to_string(&log_file_path)?;
        let lines = reflog_lines(&contents);
        assert_eq!(
            lines.len(),
            2,
            "lines parse and it's exactly two, one for branch creation, another for oplog id"
        );

        let first_line = &lines[0];
        assert_eq!(
            first_line.previous_oid, "0000000000000000000000000000000000000000",
            "start from nothing"
        );
        assert_eq!(
            first_line.new_oid.to_string(),
            commit_id.to_string(),
            "the new hash is the target id"
        );
        let first_line_message = format!("branch: Created from {}", commit_id);
        assert_eq!(first_line.message, first_line_message);
        assert_signature(first_line.signature);

        let second_line = &lines[1];
        assert_eq!(
            second_line.previous_oid.to_string(),
            commit_id.to_string(),
            "second entry starts where the first left off"
        );
        assert_eq!(second_line.new_oid.to_string(), oplog_sha.to_string());
        let line2_message = format!("reset: moving to {oplog_sha}");
        assert_eq!(second_line.message, line2_message);
        assert_signature(second_line.signature);

        // Update the oplog head only
        let another_oplog_sha = git::Oid::from_str("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")?;
        set_reference_to_oplog(&worktree_dir, commit_id.into(), another_oplog_sha)
            .expect("success");

        let contents = std::fs::read_to_string(&log_file_path)?;
        let lines: Vec<_> = reflog_lines(&contents);
        assert_eq!(lines.len(), 2);
        let first_line = &lines[0];
        assert_eq!(
            format!("{} {}", first_line.previous_oid, first_line.new_oid),
            format!("0000000000000000000000000000000000000000 {}", commit_id)
        );
        assert_eq!(first_line.message, first_line_message);
        assert_signature(first_line.signature);

        let second_line = &lines[1];
        assert_eq!(
            format!("{} {}", second_line.previous_oid, second_line.new_oid),
            format!("{} {}", commit_id, another_oplog_sha)
        );
        let second_line_message = format!("reset: moving to {another_oplog_sha}");
        assert_eq!(second_line.message, second_line_message);
        assert_signature(second_line.signature);

        // Update the target head only
        let new_target = git::Oid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")?;
        set_reference_to_oplog(&worktree_dir, new_target, another_oplog_sha).expect("success");

        let contents = std::fs::read_to_string(&log_file_path)?;
        let lines: Vec<_> = reflog_lines(&contents);
        assert_eq!(lines.len(), 2);
        let first_line = &lines[0];
        assert_eq!(
            format!("{} {}", first_line.previous_oid, first_line.new_oid),
            format!("0000000000000000000000000000000000000000 {}", new_target)
        );
        let line1_message = format!("branch: Created from {new_target}");
        assert_eq!(first_line.message, line1_message);
        assert_signature(first_line.signature);

        let second_line = &lines[1];
        assert_eq!(
            format!("{} {}", second_line.previous_oid, second_line.new_oid),
            format!("{} {}", new_target, another_oplog_sha)
        );
        assert_eq!(second_line.message, second_line_message);
        assert_signature(second_line.signature);

        Ok(())
    }

    fn reflog_lines(contents: &str) -> Vec<LineRef<'_>> {
        gix::refs::file::log::iter::forward(contents.as_bytes())
            .map(Result::unwrap)
            .collect::<Vec<_>>()
    }

    fn assert_signature(sig: gix::actor::SignatureRef<'_>) {
        assert_eq!(sig.name, GITBUTLER_INTEGRATION_COMMIT_AUTHOR_NAME);
        assert_eq!(sig.email, GITBUTLER_INTEGRATION_COMMIT_AUTHOR_EMAIL);
    }

    fn setup_repo() -> anyhow::Result<(tempfile::TempDir, git2::Oid)> {
        let dir = tempdir()?;
        let repo = git2::Repository::init(dir.path())?;
        let file_path = dir.path().join("foo.txt");
        std::fs::write(file_path, "test")?;
        let mut index = repo.index()?;
        index.add_path(&PathBuf::from("foo.txt"))?;
        let oid = index.write_tree()?;
        let name = "Your Name";
        let email = "your.email@example.com";
        let signature = git2::Signature::now(name, email)?;
        let commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "initial commit",
            &repo.find_tree(oid)?,
            &[],
        )?;
        Ok((dir, commit_id))
    }
}
