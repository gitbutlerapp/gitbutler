//! Tests for [`but_workspace::init::set_target_ref_and_init_project()`], the metadata-only
//! replacement for the legacy `set_base_branch()`.

use but_core::git_config::{edit_config, set_config_value};
use but_core::ref_metadata::ProjectMeta;
use gix::refs::transaction::PreviousValue;

use crate::utils::writable_scenario;

fn scenario() -> (
    gix::Repository,
    but_testsupport::gix_testtools::tempfile::TempDir,
) {
    let (repo, tmp) = writable_scenario("init-project-with-origin");
    (repo, tmp)
}

fn set_target_ref(
    repo: &gix::Repository,
    target_ref: &str,
    push_remote: Option<&str>,
) -> anyhow::Result<()> {
    let target_ref: gix::refs::FullName = target_ref.try_into()?;
    but_workspace::init::set_target_ref_and_init_project(
        repo,
        target_ref.as_ref(),
        push_remote.map(ToOwned::to_owned),
    )
}

fn set_push_remote(repo: &gix::Repository, push_remote: &str) -> anyhow::Result<()> {
    but_workspace::init::set_push_remote(repo, push_remote.to_owned())
}

fn stored_meta(repo: &gix::Repository) -> ProjectMeta {
    ProjectMeta::resolve(repo).expect("project metadata is readable")
}

fn create_remote_branch(repo: &gix::Repository, name: &str) -> anyhow::Result<()> {
    repo.reference(
        name,
        repo.head_id()?.detach(),
        PreviousValue::Any,
        "test remote branch",
    )?;
    Ok(())
}

fn set_remote_head(repo: &gix::Repository, target: &str) -> anyhow::Result<()> {
    repo.edit_reference(gix::refs::transaction::RefEdit {
        change: gix::refs::transaction::Change::Update {
            log: gix::refs::transaction::LogChange {
                mode: gix::refs::transaction::RefLog::AndReference,
                force_create_reflog: false,
                message: "test remote HEAD".into(),
            },
            expected: PreviousValue::Any,
            new: gix::refs::Target::Symbolic(target.try_into()?),
        },
        name: "refs/remotes/origin/HEAD".try_into()?,
        deref: false,
    })?;
    Ok(())
}

fn inferred_target(repo: &gix::Repository) -> anyhow::Result<Option<String>> {
    Ok(but_workspace::init::infer_default_target_ref(repo)?.map(|name| name.to_string()))
}

#[test]
fn infers_symbolic_remote_head_first() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    create_remote_branch(&repo, "refs/remotes/origin/trunk")?;
    set_remote_head(&repo, "refs/remotes/origin/trunk")?;

    assert_eq!(
        inferred_target(&repo)?,
        Some("refs/remotes/origin/trunk".into()),
        "a valid symbolic remote HEAD has the highest priority"
    );
    Ok(())
}

#[test]
fn malformed_remote_head_falls_back_to_main() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    create_remote_branch(&repo, "refs/remotes/fork/trunk")?;
    set_remote_head(&repo, "refs/remotes/fork/trunk")?;

    assert_eq!(
        inferred_target(&repo)?,
        Some("refs/remotes/origin/main".into()),
        "a symbolic HEAD pointing at another remote must not change the selected remote"
    );
    Ok(())
}

#[test]
fn infers_main_without_symbolic_remote_head() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    assert_eq!(
        inferred_target(&repo)?,
        Some("refs/remotes/origin/main".into()),
        "main is the first fallback when remote HEAD is absent"
    );
    Ok(())
}

#[test]
fn infers_master_when_main_is_absent() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    repo.find_reference("refs/remotes/origin/main")?.delete()?;
    create_remote_branch(&repo, "refs/remotes/origin/master")?;

    assert_eq!(
        inferred_target(&repo)?,
        Some("refs/remotes/origin/master".into()),
        "master is used only after remote HEAD and main"
    );
    Ok(())
}

#[test]
fn returns_none_without_a_candidate_branch() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    repo.find_reference("refs/remotes/origin/main")?.delete()?;

    assert_eq!(
        inferred_target(&repo)?,
        None,
        "a default remote without HEAD, main, or master has no inferred target"
    );
    Ok(())
}

#[test]
fn returns_none_without_an_unambiguous_default_remote() -> anyhow::Result<()> {
    let tmp = but_testsupport::gix_testtools::tempfile::TempDir::new()?;
    let repo = gix::init(tmp.path())?;
    edit_config(Some(&repo), gix::config::Source::Local, |config| {
        set_config_value(
            config,
            "remote.upstream.url",
            "https://example.com/upstream",
        )?;
        set_config_value(config, "remote.fork.url", "https://example.com/fork")?;
        Ok(())
    })?;
    let repo = but_testsupport::open_repo(tmp.path())?;

    assert_eq!(
        repo.remote_default_name(gix::remote::Direction::Push),
        None,
        "two non-origin remotes have no implicit default"
    );
    assert_eq!(
        inferred_target(&repo)?,
        None,
        "target inference requires a default remote"
    );
    Ok(())
}

/// Create an empty commit on top of `parent` without updating any reference.
fn empty_commit_on_top(
    repo: &gix::Repository,
    parent: gix::ObjectId,
    message: &str,
) -> gix::ObjectId {
    let tree = repo
        .find_commit(parent)
        .expect("parent commit exists")
        .tree_id()
        .expect("commit has a tree")
        .detach();
    let signature = gix::actor::Signature {
        name: "test".into(),
        email: "test@example.com".into(),
        time: gix::date::Time::new(1675176957, 0),
    };
    let commit = gix::objs::Commit {
        tree,
        parents: [parent].into(),
        author: signature.clone(),
        committer: signature,
        encoding: None,
        message: message.into(),
        extra_headers: Vec::new(),
    };
    repo.write_object(&commit)
        .expect("commit can be written")
        .detach()
}

#[test]
fn fresh_init_sets_target_and_keeps_current_branch() {
    let (repo, _tmp) = scenario();
    let head_name_before = repo.head_name().unwrap().unwrap();
    let head_commit = repo.head_id().unwrap().detach();

    set_target_ref(&repo, "refs/remotes/origin/main", None).unwrap();

    let project_meta = stored_meta(&repo);
    assert_eq!(
        project_meta.target_ref.map(|name| name.to_string()),
        Some("refs/remotes/origin/main".to_string())
    );
    assert_eq!(
        project_meta.target_commit_id,
        Some(head_commit),
        "with no stored id, the merge-base is used - here HEAD and the target share their only commit"
    );

    // Re-open to observe ref and configuration changes.
    let repo = but_testsupport::open_repo(repo.workdir().expect("fixture has a worktree")).unwrap();
    assert_eq!(
        repo.head_name().unwrap().unwrap(),
        head_name_before,
        "the user stays on their current branch"
    );
    assert!(
        repo.try_find_reference("refs/heads/gitbutler/workspace")
            .unwrap()
            .is_none(),
        "no workspace reference is created"
    );
    assert_eq!(
        repo.config_snapshot()
            .string("log.excludeDecoration")
            .map(|value| value.to_string()),
        Some("refs/gitbutler".to_string()),
        "initialization hides GitButler refs from log decorations, like the legacy path did"
    );
}

#[test]
fn resetting_the_same_ref_keeps_the_target_commit_id() {
    let (repo, _tmp) = scenario();
    set_target_ref(&repo, "refs/remotes/origin/main", None).unwrap();
    let initial_target_id = stored_meta(&repo).target_commit_id.unwrap();

    // Advance both the local branch and the remote-tracking ref so a recomputed
    // merge-base would differ from the stored one.
    let new_commit = empty_commit_on_top(&repo, repo.head_id().unwrap().detach(), "advance");
    repo.reference("refs/heads/main", new_commit, PreviousValue::Any, "test")
        .unwrap();
    repo.reference(
        "refs/remotes/origin/main",
        new_commit,
        PreviousValue::Any,
        "test",
    )
    .unwrap();

    set_target_ref(&repo, "refs/remotes/origin/main", None).unwrap();
    assert_eq!(
        stored_meta(&repo).target_commit_id,
        Some(initial_target_id),
        "an existing target commit id is never overwritten"
    );
}

#[test]
fn changing_the_target_ref_preserves_the_target_commit_id() {
    let (repo, _tmp) = scenario();
    set_target_ref(&repo, "refs/remotes/origin/main", None).unwrap();
    let initial_target_id = stored_meta(&repo).target_commit_id.unwrap();

    // Advance the local branch and create the new target at the new commit so a
    // recomputed merge-base with the new target would differ from the stored one.
    let new_commit = empty_commit_on_top(&repo, repo.head_id().unwrap().detach(), "advance");
    repo.reference("refs/heads/main", new_commit, PreviousValue::Any, "test")
        .unwrap();
    repo.reference(
        "refs/remotes/origin/other",
        new_commit,
        PreviousValue::Any,
        "test",
    )
    .unwrap();

    set_target_ref(&repo, "refs/remotes/origin/other", None).unwrap();

    let project_meta = stored_meta(&repo);
    assert_eq!(
        project_meta.target_ref.map(|name| name.to_string()),
        Some("refs/remotes/origin/other".to_string())
    );
    assert_eq!(
        project_meta.target_commit_id,
        Some(initial_target_id),
        "the stored id is kept verbatim when it remains reachable from the new target"
    );
}

#[test]
fn changing_to_a_diverged_target_recomputes_the_target_commit_id() {
    let (repo, _tmp) = scenario();
    let root = repo.head_id().unwrap().detach();
    let old_target = empty_commit_on_top(&repo, root, "old target");
    let head = empty_commit_on_top(&repo, old_target, "head");
    repo.reference(
        "refs/remotes/origin/main",
        old_target,
        PreviousValue::Any,
        "test",
    )
    .unwrap();
    repo.reference("refs/heads/main", head, PreviousValue::Any, "test")
        .unwrap();
    set_target_ref(&repo, "refs/remotes/origin/main", None).unwrap();

    let diverged_target = empty_commit_on_top(&repo, root, "diverged target");
    repo.reference(
        "refs/remotes/origin/other",
        diverged_target,
        PreviousValue::Any,
        "test",
    )
    .unwrap();
    set_target_ref(&repo, "refs/remotes/origin/other", None).unwrap();

    let project_meta = stored_meta(&repo);
    assert_eq!(
        project_meta.target_commit_id,
        Some(root),
        "the stored target commit must be reachable from the replacement target"
    );
}

#[test]
fn fills_missing_target_commit_id_from_existing_target_ref() {
    let (repo, _tmp) = scenario();
    let target_ref = "refs/remotes/origin/main";

    // Write the partially migrated state - target ref present, commit id missing -
    // directly to the local Git configuration, as a metadata write would already
    // repair it. The repair in `set_target_ref_and_init_project()` is what has to
    // fill the missing commit id.
    edit_config(Some(&repo), gix::config::Source::Local, |config| {
        set_config_value(config, "gitbutler.project.targetRef", target_ref)?;
        Ok(())
    })
    .unwrap();

    // Advance only the remote-tracking ref, leaving `HEAD` behind, so the target tip
    // (which migration repair fills in) differs from `merge_base(HEAD, target)`.
    let old_tip = repo
        .find_reference(target_ref)
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .id;
    let new_target_tip = empty_commit_on_top(&repo, old_tip, "advance target");
    repo.reference(target_ref, new_target_tip, PreviousValue::Any, "test")
        .unwrap();

    set_target_ref(&repo, target_ref, None).unwrap();

    assert_eq!(
        stored_meta(&repo).target_commit_id,
        Some(new_target_tip),
        "a missing id is filled from the stored target ref's tip after validating its history"
    );
}

#[test]
fn push_remote_is_set_and_preserved_when_omitted() {
    let (repo, _tmp) = scenario();

    // 'fork' deliberately differs from the target's own remote so preservation
    // can't be confused with defaulting to the target's remote.
    set_target_ref(&repo, "refs/remotes/origin/main", Some("fork")).unwrap();
    assert_eq!(stored_meta(&repo).push_remote.as_deref(), Some("fork"));

    // Unlike the legacy `set_base_branch()`, omitting the push remote keeps it.
    set_target_ref(&repo, "refs/remotes/origin/main", None).unwrap();
    assert_eq!(
        stored_meta(&repo).push_remote.as_deref(),
        Some("fork"),
        "the existing push remote is preserved, not replaced by the target's remote"
    );
}

#[test]
fn push_remote_changes_without_changing_target() {
    let (repo, _tmp) = scenario();
    set_target_ref(&repo, "refs/remotes/origin/main", None).unwrap();
    let before = stored_meta(&repo);

    set_push_remote(&repo, "fork").unwrap();

    let after = stored_meta(&repo);
    assert_eq!(after.target_ref, before.target_ref);
    assert_eq!(after.target_commit_id, before.target_commit_id);
    assert_eq!(after.push_remote.as_deref(), Some("fork"));
}

mod error {
    use super::*;
    use but_error::{AnyhowContextExt, Code};

    #[test]
    fn unrelated_target_is_an_actionable_precondition_failure() {
        let (repo, _tmp) = scenario();
        let meta_before = stored_meta(&repo);
        let exclude_decoration_before = repo
            .config_snapshot()
            .string("log.excludeDecoration")
            .map(|value| value.to_owned());
        repo.commit(
            "refs/remotes/origin/unrelated",
            "unrelated root",
            repo.object_hash().empty_tree(),
            std::iter::empty::<gix::ObjectId>(),
        )
        .unwrap();

        let err = set_target_ref(&repo, "refs/remotes/origin/unrelated", None).unwrap_err();

        assert_eq!(
            err.custom_context().map(|ctx| ctx.code),
            Some(Code::PreconditionFailed),
            "an unrelated target is a recoverable selection problem"
        );
        assert!(
            err.to_string()
                .contains("Fetch more history or choose another branch"),
            "the error tells onboarding how the user can recover"
        );
        assert_eq!(
            stored_meta(&repo),
            meta_before,
            "rejecting an unrelated target must not write project metadata"
        );
        assert_eq!(
            repo.config_snapshot()
                .string("log.excludeDecoration")
                .map(|value| value.to_owned()),
            exclude_decoration_before,
            "rejecting an unrelated target must not finish project initialization"
        );
    }

    #[test]
    fn stale_commit_without_target_ref_does_not_bypass_validation() {
        let (repo, _tmp) = scenario();
        let meta_before = ProjectMeta {
            target_ref: None,
            target_commit_id: Some(repo.head_id().unwrap().detach()),
            push_remote: None,
        };
        meta_before.persist(&repo).unwrap();
        repo.commit(
            "refs/remotes/origin/unrelated",
            "unrelated root",
            repo.object_hash().empty_tree(),
            std::iter::empty::<gix::ObjectId>(),
        )
        .unwrap();

        let err = set_target_ref(&repo, "refs/remotes/origin/unrelated", None).unwrap_err();

        assert_eq!(
            err.custom_context().map(|ctx| ctx.code),
            Some(Code::PreconditionFailed),
            "an orphaned target id must not authorize an unrelated target"
        );
        assert_eq!(
            stored_meta(&repo),
            meta_before,
            "rejecting the target must preserve the incomplete metadata for recovery"
        );
    }

    #[test]
    fn existing_target_does_not_bypass_unrelated_target_validation() {
        let (repo, _tmp) = scenario();
        set_target_ref(&repo, "refs/remotes/origin/main", None).unwrap();
        let meta_before = stored_meta(&repo);
        repo.commit(
            "refs/remotes/origin/unrelated",
            "unrelated root",
            repo.object_hash().empty_tree(),
            std::iter::empty::<gix::ObjectId>(),
        )
        .unwrap();

        let err = set_target_ref(&repo, "refs/remotes/origin/unrelated", None).unwrap_err();

        assert_eq!(
            err.custom_context().map(|ctx| ctx.code),
            Some(Code::PreconditionFailed),
            "an existing target pair must not authorize an unrelated replacement"
        );
        assert_eq!(
            stored_meta(&repo),
            meta_before,
            "rejecting the replacement target must preserve existing metadata"
        );
    }

    #[test]
    fn missing_remote_branch() {
        let (repo, _tmp) = scenario();
        assert_eq!(
            set_target_ref(&repo, "refs/remotes/origin/missing", None)
                .unwrap_err()
                .to_string(),
            "remote branch 'refs/remotes/origin/missing' not found"
        );
    }

    #[test]
    fn missing_stored_target_commit_is_recomputed() {
        let (repo, _tmp) = scenario();
        let missing = gix::ObjectId::from_hex(b"1111111111111111111111111111111111111111").unwrap();
        ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into().unwrap()),
            target_commit_id: Some(missing),
            push_remote: None,
        }
        .persist(&repo)
        .unwrap();

        set_target_ref(&repo, "refs/remotes/origin/main", None).unwrap();

        let stored = stored_meta(&repo).target_commit_id.unwrap();
        assert_ne!(
            stored, missing,
            "the missing stored object must not be retained"
        );
        assert_eq!(
            stored,
            repo.merge_base(
                repo.head_id().unwrap().detach(),
                repo.find_reference("refs/remotes/origin/main")
                    .unwrap()
                    .peel_to_commit()
                    .unwrap()
                    .id
            )
            .unwrap()
            .detach(),
            "the missing stored object is replaced by the validated merge-base"
        );
    }

    #[test]
    fn local_branch_rejected() {
        let (repo, _tmp) = scenario();
        assert_eq!(
            set_target_ref(&repo, "refs/heads/main", None)
                .unwrap_err()
                .to_string(),
            "target ref 'refs/heads/main' must be a remote tracking branch"
        );
    }

    #[test]
    fn unknown_push_remote() {
        let (repo, _tmp) = scenario();
        assert_eq!(
            set_target_ref(&repo, "refs/remotes/origin/main", Some("nope"))
                .unwrap_err()
                .to_string(),
            "failed to find remote nope"
        );
    }

    #[test]
    fn standalone_unknown_push_remote_does_not_change_metadata() {
        let (repo, _tmp) = scenario();
        set_target_ref(&repo, "refs/remotes/origin/main", Some("fork")).unwrap();
        let before = stored_meta(&repo);

        assert_eq!(
            set_push_remote(&repo, "nope").unwrap_err().to_string(),
            "failed to find remote nope"
        );
        assert_eq!(stored_meta(&repo), before);
    }

    #[test]
    fn standalone_push_remote_requires_target() {
        let (repo, _tmp) = scenario();

        assert_eq!(
            set_push_remote(&repo, "fork").unwrap_err().to_string(),
            "cannot set push remote without a default target"
        );
    }

    #[test]
    fn remote_without_fetch_url_rejected() {
        let (repo, _tmp) = scenario();

        // A remote that exists (has a push URL and refspecs) but has no fetch URL.
        // Accepting it would break every later base-branch read, which derives the
        // fetch URL on demand.
        edit_config(Some(&repo), gix::config::Source::Local, |config| {
            set_config_value(config, "remote.pushonly.pushUrl", "./remote.git")?;
            set_config_value(
                config,
                "remote.pushonly.fetch",
                "+refs/heads/*:refs/remotes/pushonly/*",
            )?;
            Ok(())
        })
        .unwrap();
        let head_id = repo.head_id().unwrap().detach();
        repo.reference(
            "refs/remotes/pushonly/main",
            head_id,
            PreviousValue::Any,
            "test",
        )
        .unwrap();

        let repo =
            but_testsupport::open_repo(repo.workdir().expect("fixture has a worktree")).unwrap();
        assert_eq!(
            set_target_ref(&repo, "refs/remotes/pushonly/main", None)
                .unwrap_err()
                .to_string(),
            "failed to get remote url for 'pushonly'"
        );
    }
}
