use std::path::Path;

use anyhow::{Context, Result};
use gitbutler_fs::write;
use gitbutler_repo::{GITBUTLER_COMMIT_AUTHOR_EMAIL, GITBUTLER_COMMIT_AUTHOR_NAME};
use gix::config::tree::Key;

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
/// 0000000000000000000000000000000000000000 <target branch head>
/// <target branch head>                     <oplog head>
///
/// The reflog entry is continuously updated to refer to the current target and oplog head commits.
pub(super) fn set_reference_to_oplog(
    worktree_dir: &Path,
    target_commit_id: git2::Oid,
    oplog_commit_id: git2::Oid,
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
        gix::open::Options::isolated().config_overrides({
            let sig = standard_signature();
            [
                gix::config::tree::User::NAME.validated_assignment(sig.name)?,
                gix::config::tree::User::EMAIL.validated_assignment(sig.email)?,
            ]
        }),
    )?;
    // The check is here only to avoid unnecessary writes
    if repo.try_find_reference("gitbutler/target")?.is_none() {
        repo.refs.write_reflog = gix::refs::store::WriteReflog::Always;
        let target_commit_hex = target_commit_id.to_string();
        repo.reference(
            "refs/heads/gitbutler/target",
            target_commit_hex.parse::<gix::ObjectId>()?,
            gix::refs::transaction::PreviousValue::Any,
            branch_creation_message(&target_commit_hex),
        )?;
    }

    let mut content = match std::fs::read_to_string(&reflog_file_path) {
        Ok(c) => c,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    content = set_target_ref(&content, &target_commit_id.to_string()).with_context(|| {
        format!(
            "Something was wrong with oplog reflog file at \"{}\"",
            reflog_file_path.display()
        )
    })?;
    content = set_oplog_ref(&content, &oplog_commit_id.to_string())?;
    write(reflog_file_path, content)?;

    Ok(())
}

fn branch_creation_message(commit_id_hex: &str) -> String {
    format!("branch: Created from {commit_id_hex}")
}

fn standard_signature() -> gix::actor::SignatureRef<'static> {
    gix::actor::SignatureRef {
        name: GITBUTLER_COMMIT_AUTHOR_NAME.into(),
        email: GITBUTLER_COMMIT_AUTHOR_EMAIL.into(),
        time: gix::date::Time::now_local_or_utc(),
    }
}

fn set_target_ref(reflog_content: &str, target_commit_id_hex: &str) -> Result<String> {
    // 0000000000000000000000000000000000000000 82873b54925ab268e9949557f28d070d388e7774 Kiril Videlov <kiril@videlov.com> 1714037434 +0200\tbranch: Created from 82873b54925ab268e9949557f28d070d388e7774
    let mut lines = gix::refs::file::log::iter::forward(reflog_content.as_bytes());
    let message = branch_creation_message(target_commit_id_hex);
    let expected_first_line = gix::refs::file::log::LineRef {
        previous_oid: "0000000000000000000000000000000000000000".into(),
        new_oid: target_commit_id_hex.into(),
        signature: standard_signature(),
        message: message.as_str().into(),
    };
    let mut first_line = lines
        .next()
        .unwrap_or(Ok(expected_first_line))
        .unwrap_or(expected_first_line);

    first_line.new_oid = target_commit_id_hex.into();
    let message = format!("branch: Created from {target_commit_id_hex}");
    first_line.message = message.as_str().into();

    Ok(serialize_line(first_line))
}

fn set_oplog_ref(reflog_content: &str, oplog_commit_id_hex: &str) -> Result<String> {
    // 82873b54925ab268e9949557f28d070d388e7774 7e8eab472636a26611214bebea7d6b79c971fb8b Kiril Videlov <kiril@videlov.com> 1714044124 +0200\treset: moving to 7e8eab472636a26611214bebea7d6b79c971fb8b
    let mut lines = gix::refs::file::log::iter::forward(reflog_content.as_bytes());
    let first_line = lines.next().context("need the creation-line in reflog")??;

    let new_msg = format!("reset: moving to {}", oplog_commit_id_hex);
    let second_line = gix::refs::file::log::LineRef {
        previous_oid: first_line.new_oid,
        new_oid: oplog_commit_id_hex.into(),
        message: new_msg.as_str().into(),
        ..first_line
    };

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
    use std::path::PathBuf;

    use gix::refs::file::log::LineRef;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    use super::{
        set_reference_to_oplog, GITBUTLER_COMMIT_AUTHOR_EMAIL, GITBUTLER_COMMIT_AUTHOR_NAME,
    };

    #[test]
    fn reflog_present_but_empty() -> anyhow::Result<()> {
        let (dir, commit_id) = setup_repo()?;
        let worktree_dir = dir.path();

        let oplog = git2::Oid::from_str("0123456789abcdef0123456789abcdef0123456")?;
        set_reference_to_oplog(worktree_dir, commit_id, oplog).expect("success");

        let log_file_path = worktree_dir.join(".git/logs/refs/heads/gitbutler/target");
        std::fs::write(&log_file_path, [])?;

        set_reference_to_oplog(worktree_dir, commit_id, oplog).expect("success");

        let contents = std::fs::read_to_string(&log_file_path)?;
        assert_eq!(reflog_lines(&contents).len(), 2);

        let contents = std::fs::read_to_string(&log_file_path)?;
        let lines = reflog_lines(&contents);
        assert_signature(lines[0].signature);
        Ok(())
    }

    #[test]
    fn reflog_present_but_broken() -> anyhow::Result<()> {
        let (dir, commit_id) = setup_repo()?;
        let worktree_dir = dir.path();

        let oplog = git2::Oid::from_str("0123456789abcdef0123456789abcdef0123456")?;
        set_reference_to_oplog(worktree_dir, commit_id, oplog).expect("success");

        let log_file_path = worktree_dir.join(".git/logs/refs/heads/gitbutler/target");
        std::fs::write(&log_file_path, b"a gobbled mess that is no reflog")?;

        set_reference_to_oplog(worktree_dir, commit_id, oplog).expect("success");

        let contents = std::fs::read_to_string(&log_file_path)?;
        assert_eq!(reflog_lines(&contents).len(), 2);
        Ok(())
    }

    #[test]
    fn reflog_present_but_branch_is_missing() -> anyhow::Result<()> {
        let (dir, commit_id) = setup_repo()?;
        let worktree_dir = dir.path();

        let oplog = git2::Oid::from_str("0123456789abcdef0123456789abcdef0123456")?;
        set_reference_to_oplog(worktree_dir, commit_id, oplog).expect("success");

        let loose_ref_path = worktree_dir.join(".git/refs/heads/gitbutler/target");
        std::fs::remove_file(&loose_ref_path)?;

        set_reference_to_oplog(worktree_dir, commit_id, oplog).expect("success");
        assert!(
            loose_ref_path.is_file(),
            "the file was recreated, just in case there is only a reflog and no branch"
        );
        Ok(())
    }

    #[test]
    fn branch_present_but_reflog_is_missing() -> anyhow::Result<()> {
        let (dir, commit_id) = setup_repo()?;
        let worktree_dir = dir.path();

        let oplog = git2::Oid::from_str("0123456789abcdef0123456789abcdef0123456")?;
        set_reference_to_oplog(worktree_dir, commit_id, oplog).expect("success");

        let log_file_path = worktree_dir.join(".git/logs/refs/heads/gitbutler/target");
        std::fs::remove_file(&log_file_path)?;

        set_reference_to_oplog(worktree_dir, commit_id, oplog)
            .expect("missing reflog files are recreated");
        assert!(log_file_path.is_file(), "the file was recreated");

        let contents = std::fs::read_to_string(&log_file_path)?;
        let lines = reflog_lines(&contents);
        assert_signature(lines[0].signature);
        Ok(())
    }

    #[test]
    fn new_and_update() -> anyhow::Result<()> {
        let (dir, commit_id) = setup_repo()?;
        let commit_id_hex = commit_id.to_string();
        let commit_id_hex: &gix::bstr::BStr = commit_id_hex.as_str().into();
        let worktree_dir = dir.path();

        let log_file_path = worktree_dir.join(".git/logs/refs/heads/gitbutler/target");
        assert!(!log_file_path.exists());

        // Set ref for the first time
        let oplog_hex = "0123456789abcdef0123456789abcdef01234567";
        let oplog = git2::Oid::from_str(oplog_hex)?;
        set_reference_to_oplog(worktree_dir, commit_id, oplog).expect("success");
        assert!(log_file_path.exists());
        let contents = std::fs::read_to_string(&log_file_path)?;
        let lines = reflog_lines(&contents);
        assert_eq!(
            lines.len(),
            2,
            "lines parse and it's exactly two, one for branch creation, another for oplog id"
        );

        let first_line = lines[0];
        assert_signature(first_line.signature);
        let first_line_message = format!("branch: Created from {}", commit_id);
        let expected_line = gix::refs::file::log::LineRef {
            previous_oid: "0000000000000000000000000000000000000000".into(),
            new_oid: commit_id_hex,
            signature: first_line.signature,
            message: first_line_message.as_str().into(),
        };
        assert_eq!(first_line, expected_line);

        let second_line = lines[1];
        let second_line_message = format!("reset: moving to {oplog}");
        let expected_line = gix::refs::file::log::LineRef {
            previous_oid: commit_id_hex,
            new_oid: oplog_hex.into(),
            signature: first_line.signature,
            message: second_line_message.as_str().into(),
        };
        assert_eq!(second_line, expected_line);

        // Update the oplog head only
        let another_oplog_hex = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let another_oplog = git2::Oid::from_str(another_oplog_hex)?;
        set_reference_to_oplog(worktree_dir, commit_id, another_oplog).expect("success");

        let contents = std::fs::read_to_string(&log_file_path)?;
        let lines: Vec<_> = reflog_lines(&contents);
        assert_eq!(lines.len(), 2);

        let first_line = lines[0];
        assert_signature(first_line.signature);
        let expected_line = gix::refs::file::log::LineRef {
            previous_oid: "0000000000000000000000000000000000000000".into(),
            new_oid: commit_id_hex,
            signature: first_line.signature,
            message: first_line_message.as_str().into(),
        };
        assert_eq!(first_line, expected_line);

        let second_line = lines[1];
        let second_line_message = format!("reset: moving to {another_oplog}");
        let expected_line = gix::refs::file::log::LineRef {
            previous_oid: commit_id_hex,
            new_oid: another_oplog_hex.into(),
            signature: first_line.signature,
            message: second_line_message.as_str().into(),
        };
        assert_eq!(second_line, expected_line);

        // Update the target head only
        let new_target_hex = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let new_target = git2::Oid::from_str(new_target_hex)?;
        set_reference_to_oplog(worktree_dir, new_target, another_oplog).expect("success");

        let contents = std::fs::read_to_string(&log_file_path)?;
        let lines: Vec<_> = reflog_lines(&contents);
        assert_eq!(lines.len(), 2);

        let first_line = lines[0];
        assert_signature(first_line.signature);
        let first_line_message = format!("branch: Created from {new_target}");
        let expected_line = gix::refs::file::log::LineRef {
            previous_oid: "0000000000000000000000000000000000000000".into(),
            new_oid: new_target_hex.into(),
            signature: first_line.signature,
            message: first_line_message.as_str().into(),
        };
        assert_eq!(first_line, expected_line);

        let second_line = lines[1];
        assert_signature(second_line.signature);
        let expected_line = gix::refs::file::log::LineRef {
            previous_oid: new_target_hex.into(),
            new_oid: another_oplog_hex.into(),
            signature: first_line.signature,
            message: second_line_message.as_str().into(),
        };
        assert_eq!(second_line, expected_line);

        Ok(())
    }

    fn reflog_lines(contents: &str) -> Vec<LineRef<'_>> {
        gix::refs::file::log::iter::forward(contents.as_bytes())
            .map(Result::unwrap)
            .collect::<Vec<_>>()
    }

    fn assert_signature(sig: gix::actor::SignatureRef<'_>) {
        assert_eq!(sig.name, GITBUTLER_COMMIT_AUTHOR_NAME);
        assert_eq!(sig.email, GITBUTLER_COMMIT_AUTHOR_EMAIL);
        assert_ne!(
            sig.time.seconds, 0,
            "we don't accidentally use the default time as it would caues GC as well"
        );
    }

    fn setup_repo() -> anyhow::Result<(tempfile::TempDir, git2::Oid)> {
        let dir = tempdir()?;
        let repo = git2::Repository::init(dir.path())?;
        let file_path = dir.path().join("foo.txt");
        std::fs::write(file_path, "test")?;
        let mut index = repo.index()?;
        index.add_path(&PathBuf::from("foo.txt"))?;
        let oid = index.write_tree()?;
        let name = "Your name";
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
