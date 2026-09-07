//! Tests for [`but_workspace::init::set_target_ref_and_init_project()`], the metadata-only
//! replacement for the legacy `set_base_branch()`.

use bstr::ByteSlice;
use but_core::git_config::{edit_config, set_config_value};
use but_core::ref_metadata::ProjectMeta;
use but_testsupport::visualize_commit_graph_all;
use gix::refs::transaction::PreviousValue;
use snapbox::str;

use crate::utils::writable_scenario;

fn scenario() -> (
    gix::Repository,
    but_testsupport::gix_testtools::tempfile::TempDir,
) {
    writable_scenario("init-project-with-origin")
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

/// The stored project metadata, naming the target commit by its message so the summary
/// reads as history, or `[missing]` if no such object exists.
fn stored_meta_summary(repo: &gix::Repository) -> anyhow::Result<String> {
    let meta = ProjectMeta::resolve(repo)?;
    let target_commit = meta.target_commit_id.map_or("<unset>".to_string(), |id| {
        repo.find_commit(id)
            .map_or("[missing]".to_string(), |commit| {
                commit.message_raw_sloppy().trim().as_bstr().to_string()
            })
    });
    Ok(format!(
        "target_ref={}; target_commit_id={target_commit}; push_remote={}",
        meta.target_ref
            .as_ref()
            .map_or("<unset>".to_string(), ToString::to_string),
        meta.push_remote.as_deref().unwrap_or("<unset>")
    ))
}

fn exclude_decoration(repo: &gix::Repository) -> String {
    repo.config_snapshot()
        .string("log.excludeDecoration")
        .map_or("<unset>".to_string(), |value| value.to_string())
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

fn inferred_target(repo: &gix::Repository) -> anyhow::Result<String> {
    Ok(but_workspace::init::infer_default_target_ref(repo)?
        .map_or("<none>".to_string(), |name| name.to_string()))
}

#[test]
fn infers_symbolic_remote_head_first() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    create_remote_branch(&repo, "refs/remotes/origin/trunk")?;
    set_remote_head(&repo, "refs/remotes/origin/trunk")?;

    // A valid symbolic remote HEAD has the highest priority.
    snapbox::assert_data_eq!(inferred_target(&repo)?, str!["refs/remotes/origin/trunk"]);
    Ok(())
}

#[test]
fn malformed_remote_head_falls_back_to_main() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    create_remote_branch(&repo, "refs/remotes/fork/trunk")?;
    set_remote_head(&repo, "refs/remotes/fork/trunk")?;

    // A symbolic HEAD pointing at another remote must not change the selected remote.
    snapbox::assert_data_eq!(inferred_target(&repo)?, str!["refs/remotes/origin/main"]);
    Ok(())
}

#[test]
fn infers_main_without_symbolic_remote_head() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    // main is the first fallback when remote HEAD is absent.
    snapbox::assert_data_eq!(inferred_target(&repo)?, str!["refs/remotes/origin/main"]);
    Ok(())
}

#[test]
fn infers_master_when_main_is_absent() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    repo.find_reference("refs/remotes/origin/main")?.delete()?;
    create_remote_branch(&repo, "refs/remotes/origin/master")?;

    // master is used only after remote HEAD and main.
    snapbox::assert_data_eq!(inferred_target(&repo)?, str!["refs/remotes/origin/master"]);
    Ok(())
}

#[test]
fn returns_none_without_a_candidate_branch() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    repo.find_reference("refs/remotes/origin/main")?.delete()?;

    // A default remote without HEAD, main, or master has no inferred target.
    snapbox::assert_data_eq!(inferred_target(&repo)?, str!["<none>"]);
    Ok(())
}

#[test]
fn returns_none_without_an_unambiguous_default_remote() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("two-non-origin-remotes");

    // Two non-origin remotes have no implicit default, and target inference requires one.
    snapbox::assert_data_eq!(
        repo.remote_default_name(gix::remote::Direction::Push)
            .map_or("<none>".to_string(), |name| name.to_string()),
        str!["<none>"]
    );
    snapbox::assert_data_eq!(inferred_target(&repo)?, str!["<none>"]);
    Ok(())
}

/// Create an empty commit on top of `parent` without updating any reference.
fn empty_commit_on_top(
    repo: &gix::Repository,
    parent: gix::ObjectId,
    message: &str,
) -> anyhow::Result<gix::ObjectId> {
    let tree = repo.find_commit(parent)?.tree_id()?.detach();
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
    Ok(repo.write_object(&commit)?.detach())
}

#[test]
fn fresh_init_sets_target_and_keeps_current_branch() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );

    set_target_ref(&repo, "refs/remotes/origin/main", None)?;

    // With no stored id, the merge-base is used - here HEAD and the target share their
    // only commit.
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str!["target_ref=refs/remotes/origin/main; target_commit_id=M; push_remote=<unset>"]
    );

    // Re-open to observe ref and configuration changes.
    let repo = but_testsupport::open_repo(repo.workdir().expect("fixture has a worktree"))?;
    // The user stays on their current branch, and no workspace reference is created.
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* 85efbe4 (HEAD -> main, origin/main) M

"#]]
    );
    // Initialization hides GitButler refs from log decorations, like the legacy path did.
    snapbox::assert_data_eq!(exclude_decoration(&repo), str!["refs/gitbutler"]);
    Ok(())
}

#[test]
fn resetting_the_same_ref_keeps_the_target_commit_id() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    set_target_ref(&repo, "refs/remotes/origin/main", None)?;
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str!["target_ref=refs/remotes/origin/main; target_commit_id=M; push_remote=<unset>"]
    );

    // Advance both the local branch and the remote-tracking ref so a recomputed
    // merge-base would differ from the stored one.
    let new_commit = empty_commit_on_top(&repo, repo.head_id()?.detach(), "advance")?;
    repo.reference("refs/heads/main", new_commit, PreviousValue::Any, "test")?;
    repo.reference(
        "refs/remotes/origin/main",
        new_commit,
        PreviousValue::Any,
        "test",
    )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* becc0a5 (HEAD -> main, origin/main) advance
* 85efbe4 M

"#]]
    );

    set_target_ref(&repo, "refs/remotes/origin/main", None)?;
    // An existing target commit id is never overwritten.
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str!["target_ref=refs/remotes/origin/main; target_commit_id=M; push_remote=<unset>"]
    );
    Ok(())
}

#[test]
fn changing_the_target_ref_preserves_the_target_commit_id() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    set_target_ref(&repo, "refs/remotes/origin/main", None)?;
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str!["target_ref=refs/remotes/origin/main; target_commit_id=M; push_remote=<unset>"]
    );

    // Advance the local branch and create the new target at the new commit so a
    // recomputed merge-base with the new target would differ from the stored one.
    let new_commit = empty_commit_on_top(&repo, repo.head_id()?.detach(), "advance")?;
    repo.reference("refs/heads/main", new_commit, PreviousValue::Any, "test")?;
    repo.reference(
        "refs/remotes/origin/other",
        new_commit,
        PreviousValue::Any,
        "test",
    )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* becc0a5 (HEAD -> main, origin/other) advance
* 85efbe4 (origin/main) M

"#]]
    );

    set_target_ref(&repo, "refs/remotes/origin/other", None)?;
    // The stored id is kept verbatim when it remains reachable from the new target.
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str!["target_ref=refs/remotes/origin/other; target_commit_id=M; push_remote=<unset>"]
    );
    Ok(())
}

#[test]
fn changing_to_a_diverged_target_recomputes_the_target_commit_id() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    let root = repo.head_id()?.detach();
    let old_target = empty_commit_on_top(&repo, root, "old target")?;
    let head = empty_commit_on_top(&repo, old_target, "head")?;
    repo.reference(
        "refs/remotes/origin/main",
        old_target,
        PreviousValue::Any,
        "test",
    )?;
    repo.reference("refs/heads/main", head, PreviousValue::Any, "test")?;
    set_target_ref(&repo, "refs/remotes/origin/main", None)?;
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str![
            "target_ref=refs/remotes/origin/main; target_commit_id=old target; push_remote=<unset>"
        ]
    );

    let diverged_target = empty_commit_on_top(&repo, root, "diverged target")?;
    repo.reference(
        "refs/remotes/origin/other",
        diverged_target,
        PreviousValue::Any,
        "test",
    )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* 3749a52 (HEAD -> main) head
* 643b145 (origin/main) old target
| * f49a288 (origin/other) diverged target
|/  
* 85efbe4 M

"#]]
    );

    set_target_ref(&repo, "refs/remotes/origin/other", None)?;
    // The stored target commit must be reachable from the replacement target, so it is
    // recomputed as the merge-base with the new target: the root.
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str!["target_ref=refs/remotes/origin/other; target_commit_id=M; push_remote=<unset>"]
    );
    Ok(())
}

#[test]
fn fills_missing_target_commit_id_from_existing_target_ref() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    let target_ref = "refs/remotes/origin/main";

    // Write the partially migrated state - target ref present, commit id missing -
    // directly to the local Git configuration, as a metadata write would already
    // repair it. The repair in `set_target_ref_and_init_project()` is what has to
    // fill the missing commit id.
    edit_config(Some(&repo), gix::config::Source::Local, |config| {
        set_config_value(config, "gitbutler.project.targetRef", target_ref)?;
        Ok(())
    })?;
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str!["target_ref=refs/remotes/origin/main; target_commit_id=<unset>; push_remote=<unset>"]
    );

    // Advance only the remote-tracking ref, leaving `HEAD` behind, so the target tip
    // (which migration repair fills in) differs from `merge_base(HEAD, target)`.
    let old_tip = repo.find_reference(target_ref)?.peel_to_commit()?.id;
    let new_target_tip = empty_commit_on_top(&repo, old_tip, "advance target")?;
    repo.reference(target_ref, new_target_tip, PreviousValue::Any, "test")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        str![[r#"
* 0f42eb0 (origin/main) advance target
* 85efbe4 (HEAD -> main) M

"#]]
    );

    set_target_ref(&repo, target_ref, None)?;
    // A missing id is filled from the stored target ref's tip after validating its history.
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str![
            "target_ref=refs/remotes/origin/main; target_commit_id=advance target; push_remote=<unset>"
        ]
    );
    Ok(())
}

#[test]
fn push_remote_is_set_and_preserved_when_omitted() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();

    // 'fork' deliberately differs from the target's own remote so preservation
    // can't be confused with defaulting to the target's remote.
    set_target_ref(&repo, "refs/remotes/origin/main", Some("fork"))?;
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str!["target_ref=refs/remotes/origin/main; target_commit_id=M; push_remote=fork"]
    );

    // Unlike the legacy `set_base_branch()`, omitting the push remote keeps it.
    set_target_ref(&repo, "refs/remotes/origin/main", None)?;
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str!["target_ref=refs/remotes/origin/main; target_commit_id=M; push_remote=fork"]
    );
    Ok(())
}

#[test]
fn push_remote_changes_without_changing_target() -> anyhow::Result<()> {
    let (repo, _tmp) = scenario();
    set_target_ref(&repo, "refs/remotes/origin/main", None)?;
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str!["target_ref=refs/remotes/origin/main; target_commit_id=M; push_remote=<unset>"]
    );

    set_push_remote(&repo, "fork")?;
    snapbox::assert_data_eq!(
        stored_meta_summary(&repo)?,
        str!["target_ref=refs/remotes/origin/main; target_commit_id=M; push_remote=fork"]
    );
    Ok(())
}

mod error {
    use super::*;
    use but_error::AnyhowContextExt;

    /// The error's classification code, followed by its full message.
    fn classified(err: &anyhow::Error) -> String {
        format!(
            "{}: {err:#}",
            err.custom_context()
                .map_or("<no code>".to_string(), |ctx| ctx.code.to_string())
        )
    }

    #[test]
    fn unrelated_target_is_an_actionable_precondition_failure() -> anyhow::Result<()> {
        let (repo, _tmp) = scenario();
        repo.commit(
            "refs/remotes/origin/unrelated",
            "unrelated root",
            repo.object_hash().empty_tree(),
            std::iter::empty::<gix::ObjectId>(),
        )?;

        let err = set_target_ref(&repo, "refs/remotes/origin/unrelated", None).unwrap_err();

        // An unrelated target is a recoverable selection problem, and the error tells
        // onboarding how the user can recover.
        snapbox::assert_data_eq!(
            classified(&err),
            str![
                "PreconditionFailed: The selected target has no common history with HEAD. Fetch more history or choose another branch."
            ]
        );
        // Rejecting an unrelated target neither writes project metadata nor finishes
        // project initialization.
        snapbox::assert_data_eq!(
            stored_meta_summary(&repo)?,
            str!["target_ref=<unset>; target_commit_id=<unset>; push_remote=<unset>"]
        );
        snapbox::assert_data_eq!(exclude_decoration(&repo), str!["<unset>"]);
        Ok(())
    }

    #[test]
    fn stale_commit_without_target_ref_does_not_bypass_validation() -> anyhow::Result<()> {
        let (repo, _tmp) = scenario();
        ProjectMeta {
            target_ref: None,
            target_commit_id: Some(repo.head_id()?.detach()),
            push_remote: None,
        }
        .persist(&repo)?;
        repo.commit(
            "refs/remotes/origin/unrelated",
            "unrelated root",
            repo.object_hash().empty_tree(),
            std::iter::empty::<gix::ObjectId>(),
        )?;

        let err = set_target_ref(&repo, "refs/remotes/origin/unrelated", None).unwrap_err();

        // An orphaned target id must not authorize an unrelated target.
        snapbox::assert_data_eq!(
            classified(&err),
            str![
                "PreconditionFailed: The selected target has no common history with HEAD. Fetch more history or choose another branch."
            ]
        );
        // Rejecting the target preserves the incomplete metadata for recovery.
        snapbox::assert_data_eq!(
            stored_meta_summary(&repo)?,
            str!["target_ref=<unset>; target_commit_id=M; push_remote=<unset>"]
        );
        Ok(())
    }

    #[test]
    fn existing_target_does_not_bypass_unrelated_target_validation() -> anyhow::Result<()> {
        let (repo, _tmp) = scenario();
        set_target_ref(&repo, "refs/remotes/origin/main", None)?;
        repo.commit(
            "refs/remotes/origin/unrelated",
            "unrelated root",
            repo.object_hash().empty_tree(),
            std::iter::empty::<gix::ObjectId>(),
        )?;

        let err = set_target_ref(&repo, "refs/remotes/origin/unrelated", None).unwrap_err();

        // An existing target pair must not authorize an unrelated replacement.
        snapbox::assert_data_eq!(
            classified(&err),
            str![
                "PreconditionFailed: The selected target has no common history with HEAD. Fetch more history or choose another branch."
            ]
        );
        // Rejecting the replacement target preserves existing metadata.
        snapbox::assert_data_eq!(
            stored_meta_summary(&repo)?,
            str!["target_ref=refs/remotes/origin/main; target_commit_id=M; push_remote=<unset>"]
        );
        Ok(())
    }

    #[test]
    fn missing_remote_branch() -> anyhow::Result<()> {
        let (repo, _tmp) = scenario();
        let err = set_target_ref(&repo, "refs/remotes/origin/missing", None).unwrap_err();
        snapbox::assert_data_eq!(
            format!("{err:#}"),
            str!["remote branch 'refs/remotes/origin/missing' not found"]
        );
        Ok(())
    }

    #[test]
    fn missing_stored_target_commit_is_recomputed() -> anyhow::Result<()> {
        let (repo, _tmp) = scenario();
        ProjectMeta {
            target_ref: Some("refs/remotes/origin/main".try_into()?),
            target_commit_id: Some(gix::ObjectId::from_hex(
                b"1111111111111111111111111111111111111111",
            )?),
            push_remote: None,
        }
        .persist(&repo)?;
        snapbox::assert_data_eq!(
            stored_meta_summary(&repo)?,
            str![
                "target_ref=refs/remotes/origin/main; target_commit_id=[missing]; push_remote=<unset>"
            ]
        );

        set_target_ref(&repo, "refs/remotes/origin/main", None)?;

        // The missing stored object is replaced by the validated merge-base of HEAD and
        // the target, the fixture's only commit.
        snapbox::assert_data_eq!(
            stored_meta_summary(&repo)?,
            str!["target_ref=refs/remotes/origin/main; target_commit_id=M; push_remote=<unset>"]
        );
        Ok(())
    }

    #[test]
    fn local_branch_rejected() -> anyhow::Result<()> {
        let (repo, _tmp) = scenario();
        let err = set_target_ref(&repo, "refs/heads/main", None).unwrap_err();
        snapbox::assert_data_eq!(
            format!("{err:#}"),
            str!["target ref 'refs/heads/main' must be a remote tracking branch"]
        );
        Ok(())
    }

    #[test]
    fn unknown_push_remote() -> anyhow::Result<()> {
        let (repo, _tmp) = scenario();
        let err = set_target_ref(&repo, "refs/remotes/origin/main", Some("nope")).unwrap_err();
        snapbox::assert_data_eq!(
            format!("{err:#}"),
            str![[r#"failed to find remote nope: The remote named "nope" did not exist"#]]
        );
        Ok(())
    }

    #[test]
    fn standalone_unknown_push_remote_does_not_change_metadata() -> anyhow::Result<()> {
        let (repo, _tmp) = scenario();
        set_target_ref(&repo, "refs/remotes/origin/main", Some("fork"))?;

        let err = set_push_remote(&repo, "nope").unwrap_err();
        snapbox::assert_data_eq!(
            format!("{err:#}"),
            str![[r#"failed to find remote nope: The remote named "nope" did not exist"#]]
        );
        snapbox::assert_data_eq!(
            stored_meta_summary(&repo)?,
            str!["target_ref=refs/remotes/origin/main; target_commit_id=M; push_remote=fork"]
        );
        Ok(())
    }

    #[test]
    fn standalone_push_remote_requires_target() -> anyhow::Result<()> {
        let (repo, _tmp) = scenario();

        let err = set_push_remote(&repo, "fork").unwrap_err();
        snapbox::assert_data_eq!(
            format!("{err:#}"),
            str![
                "cannot set push remote without a default target: DefaultTargetNotFound: there is no default target"
            ]
        );
        Ok(())
    }

    #[test]
    fn remote_without_fetch_url_rejected() -> anyhow::Result<()> {
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
        })?;
        repo.reference(
            "refs/remotes/pushonly/main",
            repo.head_id()?.detach(),
            PreviousValue::Any,
            "test",
        )?;

        let repo = but_testsupport::open_repo(repo.workdir().expect("fixture has a worktree"))?;
        let err = set_target_ref(&repo, "refs/remotes/pushonly/main", None).unwrap_err();
        snapbox::assert_data_eq!(
            format!("{err:#}"),
            str!["failed to get remote url for 'pushonly'"]
        );
        Ok(())
    }
}
