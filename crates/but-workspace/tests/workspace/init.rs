//! Tests for [`but_workspace::init::set_target_ref_and_init_project()`], the metadata-only
//! replacement for the legacy `set_base_branch()`.

use but_core::git_config::{edit_config, set_config_value};
use but_core::ref_metadata::ProjectMeta;
use but_meta::VirtualBranchesTomlMetadata;
use gix::refs::transaction::PreviousValue;

use crate::utils::writable_scenario;

fn scenario() -> (
    gix::Repository,
    VirtualBranchesTomlMetadata,
    but_testsupport::gix_testtools::tempfile::TempDir,
) {
    let (repo, tmp) = writable_scenario("init-project-with-origin");
    let meta = VirtualBranchesTomlMetadata::from_path(repo.path().join("virtual-branches.toml"))
        .expect("toml metadata opens in fixture repositories");
    (repo, meta, tmp)
}

fn set_target_ref(
    repo: &gix::Repository,
    meta: &mut VirtualBranchesTomlMetadata,
    target_ref: &str,
    push_remote: Option<&str>,
) -> anyhow::Result<()> {
    let target_ref: gix::refs::FullName = target_ref.try_into()?;
    but_workspace::init::set_target_ref_and_init_project(
        repo,
        meta,
        target_ref.as_ref(),
        push_remote.map(ToOwned::to_owned),
    )
}

fn stored_meta(repo: &gix::Repository, meta: &VirtualBranchesTomlMetadata) -> ProjectMeta {
    ProjectMeta::resolve(repo, meta).expect("project metadata is readable")
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
    let (repo, mut meta, _tmp) = scenario();
    let head_name_before = repo.head_name().unwrap().unwrap();
    let head_commit = repo.head_id().unwrap().detach();

    set_target_ref(&repo, &mut meta, "refs/remotes/origin/main", None).unwrap();

    let project_meta = stored_meta(&repo, &meta);
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
    let (repo, mut meta, _tmp) = scenario();
    set_target_ref(&repo, &mut meta, "refs/remotes/origin/main", None).unwrap();
    let initial_target_id = stored_meta(&repo, &meta).target_commit_id.unwrap();

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

    set_target_ref(&repo, &mut meta, "refs/remotes/origin/main", None).unwrap();
    assert_eq!(
        stored_meta(&repo, &meta).target_commit_id,
        Some(initial_target_id),
        "an existing target commit id is never overwritten"
    );
}

#[test]
fn changing_the_target_ref_preserves_the_target_commit_id() {
    let (repo, mut meta, _tmp) = scenario();
    set_target_ref(&repo, &mut meta, "refs/remotes/origin/main", None).unwrap();
    let initial_target_id = stored_meta(&repo, &meta).target_commit_id.unwrap();

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

    set_target_ref(&repo, &mut meta, "refs/remotes/origin/other", None).unwrap();

    let project_meta = stored_meta(&repo, &meta);
    assert_eq!(
        project_meta.target_ref.map(|name| name.to_string()),
        Some("refs/remotes/origin/other".to_string())
    );
    assert_eq!(
        project_meta.target_commit_id,
        Some(initial_target_id),
        "the stored id is kept verbatim even for a different target branch"
    );
}

#[test]
fn fills_missing_target_commit_id_from_existing_target_ref() {
    let (repo, mut meta, _tmp) = scenario();
    let target_ref = "refs/remotes/origin/main";

    // Write the partially migrated state - target ref present, commit id missing -
    // directly to the local Git configuration, as a metadata write would already
    // repair it. The repair in `set_target_ref_and_init_project()` is what has to
    // fill the missing commit id.
    edit_config(Some(&repo), gix::config::Source::Local, |config| {
        set_config_value(config, "gitbutler.project.targetRef", target_ref)?;
        set_config_value(config, "gitbutler.project.portedMeta", "true")?;
        Ok(())
    })
    .unwrap();

    // Advance only the remote-tracking ref, leaving `HEAD` behind, so the target tip
    // (which migration repair fills in) differs from `merge_base(HEAD, target)`
    // (which the fresh-init fallback would compute).
    let old_tip = repo
        .find_reference(target_ref)
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .id;
    let new_target_tip = empty_commit_on_top(&repo, old_tip, "advance target");
    repo.reference(target_ref, new_target_tip, PreviousValue::Any, "test")
        .unwrap();

    set_target_ref(&repo, &mut meta, target_ref, None).unwrap();

    assert_eq!(
        stored_meta(&repo, &meta).target_commit_id,
        Some(new_target_tip),
        "a missing id is filled from the stored target ref's tip, not the merge-base"
    );
}

#[test]
fn push_remote_is_set_and_preserved_when_omitted() {
    let (repo, mut meta, _tmp) = scenario();

    // 'fork' deliberately differs from the target's own remote so preservation
    // can't be confused with defaulting to the target's remote.
    set_target_ref(&repo, &mut meta, "refs/remotes/origin/main", Some("fork")).unwrap();
    assert_eq!(
        stored_meta(&repo, &meta).push_remote.as_deref(),
        Some("fork")
    );

    // Unlike the legacy `set_base_branch()`, omitting the push remote keeps it.
    set_target_ref(&repo, &mut meta, "refs/remotes/origin/main", None).unwrap();
    assert_eq!(
        stored_meta(&repo, &meta).push_remote.as_deref(),
        Some("fork"),
        "the existing push remote is preserved, not replaced by the target's remote"
    );
}

mod error {
    use super::*;

    #[test]
    fn missing_remote_branch() {
        let (repo, mut meta, _tmp) = scenario();
        assert_eq!(
            set_target_ref(&repo, &mut meta, "refs/remotes/origin/missing", None)
                .unwrap_err()
                .to_string(),
            "remote branch 'refs/remotes/origin/missing' not found"
        );
    }

    #[test]
    fn local_branch_rejected() {
        let (repo, mut meta, _tmp) = scenario();
        assert_eq!(
            set_target_ref(&repo, &mut meta, "refs/heads/main", None)
                .unwrap_err()
                .to_string(),
            "target ref 'refs/heads/main' must be a remote tracking branch"
        );
    }

    #[test]
    fn unknown_push_remote() {
        let (repo, mut meta, _tmp) = scenario();
        assert_eq!(
            set_target_ref(&repo, &mut meta, "refs/remotes/origin/main", Some("nope"))
                .unwrap_err()
                .to_string(),
            "failed to find remote nope"
        );
    }

    #[test]
    fn remote_without_fetch_url_rejected() {
        let (repo, mut meta, _tmp) = scenario();

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
            set_target_ref(&repo, &mut meta, "refs/remotes/pushonly/main", None)
                .unwrap_err()
                .to_string(),
            "failed to get remote url for 'pushonly'"
        );
    }
}
