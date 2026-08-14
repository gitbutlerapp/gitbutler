use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
};

use but_core::ref_metadata::ProjectMeta;
use but_ctx::{Context, ProjectHandle};
use but_path::AppChannel;
use but_testsupport::{
    CommandExt as _, git, gix_testtools::tempfile::TempDir, graph_tree, open_repo,
    writable_scenario_slow,
};

#[test]
fn new_from_project_handle_uses_repo_gitdir() -> anyhow::Result<()> {
    but_testsupport::isolated_app_data_dir(|| {
        // Keep this fixture private and writable while Context construction migrates project
        // metadata into local Git config.
        let tmp = TempDir::new_in(".")?;
        gix::init(tmp.path())?;
        let repo = open_repo(tmp.path().strip_prefix(std::env::current_dir()?)?)?;
        let worktree = repo.workdir().expect("fixture is non-bare").to_owned();

        assert!(repo.path().is_relative());
        for input in [
            repo.git_dir().to_owned(),
            repo.workdir().expect("non-bare").to_owned(),
        ] {
            let handle = ProjectHandle::from_path(&input)?;
            let ctx = Context::new_from_project_handle(handle)?;

            let expected_gitdir = gix::path::realpath(ctx.repo.get()?.path())?;
            let expected_worktree = gix::path::realpath(&worktree)?;
            assert_eq!(
                ctx.gitdir, expected_gitdir,
                "the Git dir is the realpath, so ProjectHandles can be worktrees or git directories"
            );
            assert_ne!(ctx.gitdir, repo.path(), "even though we didn't pass it");
            assert_eq!(
                ctx.workdir()?.as_deref(),
                Some(expected_worktree.as_path()),
                "real-pathiness translates to the worktree"
            );
        }

        let ctx = Context::from_repo_for_testing(repo.clone())?;
        assert_eq!(
            ctx.gitdir,
            repo.path(),
            "When creating a context from a repo directly, it will not alter the stored path though."
        );
        Ok(())
    })
}

#[test]
fn new_from_project_handle_keeps_repo_cached() -> anyhow::Result<()> {
    but_testsupport::isolated_app_data_dir(|| {
        // Keep this fixture private and writable while Context construction migrates project
        // metadata into local Git config.
        let (repo, _tmp) = but_testsupport::writable_scenario("unborn-empty");
        let handle = ProjectHandle::from_path(repo.git_dir())?;
        let ctx = Context::new_from_project_handle(handle)?;

        assert!(
            ctx.repo.get_opt().is_some(),
            "the repository used during construction should be kept in context"
        );
        assert!(ctx.to_sync().repo.is_some());
        Ok(())
    })
}

#[test]
fn project_data_dir_comes_from_git_config() -> anyhow::Result<()> {
    let repo_dir = TempDir::new()?;
    let repo = gix::init(repo_dir.path())?;
    let key = but_project_handle::storage_path_config_key().to_owned();
    git(&repo)
        .args(["config", "--local", key.as_str(), "gitbutler-custom"])
        .run();
    let repo = open_repo(repo_dir.path())?;

    let ctx = Context::from_repo_for_testing(repo)?;
    assert_eq!(ctx.project_data_dir(), ctx.gitdir.join("gitbutler-custom"));

    let db = ctx.db.get_cache()?;
    assert!(
        ctx.project_data_dir().join("but.sqlite").exists(),
        "database should be created in configured project-data directory"
    );

    let project_cache_path = ctx.project_data_dir().join("but_cache.sqlite");
    assert!(
        !project_cache_path.exists(),
        "cache database isn't present initially"
    );

    let _cache = db.cache.get()?;
    assert!(
        project_cache_path.exists(),
        "cache database should be created after first access alongside the main database in configured project-data directory"
    );
    Ok(())
}

#[test]
fn sync_context_preserves_project_data_dir() -> anyhow::Result<()> {
    let repo_dir = TempDir::new()?;
    gix::init(repo_dir.path())?;
    let repo = open_repo(repo_dir.path())?;
    let ctx = Context::from_repo_for_testing(repo)?;

    let sync = ctx.to_sync();
    let restored = sync.into_thread_local();
    assert_eq!(ctx.project_data_dir(), restored.project_data_dir());
    Ok(())
}

#[test]
fn discover_with_app_channel_uses_requested_project_data_dir() -> anyhow::Result<()> {
    let repo_dir = TempDir::new()?;
    let repo = gix::init(repo_dir.path())?;
    let nightly_key =
        but_project_handle::storage_path_config_key_for_app_channel(AppChannel::Nightly);
    let dev_key = but_project_handle::storage_path_config_key_for_app_channel(AppChannel::Dev);
    git(&repo)
        .args(["config", "--local", nightly_key, "gitbutler-nightly"])
        .run();
    git(&repo)
        .args(["config", "--local", dev_key, "gitbutler-dev"])
        .run();

    but_testsupport::isolated_app_data_dir(|| {
        let nightly_ctx = Context::discover_with_app_channel(repo_dir.path(), AppChannel::Nightly)?;
        assert_eq!(
            nightly_ctx.project_data_dir(),
            nightly_ctx.gitdir.join("gitbutler-nightly")
        );

        let dev_ctx = Context::discover_with_app_channel(repo_dir.path(), AppChannel::Dev)?;
        assert_eq!(
            dev_ctx.project_data_dir(),
            dev_ctx.gitdir.join("gitbutler-dev")
        );
        Ok(())
    })
}

#[test]
fn set_project_meta_persists_git_config() -> anyhow::Result<()> {
    let (_tmp, repo, target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo_for_testing(repo)?;
    let project_meta = project_meta(target_commit_id, "refs/remotes/origin/main", "fork")?;

    assert_eq!(ctx.project_meta()?, ProjectMeta::default());

    ctx.set_project_meta(project_meta.clone())?;
    assert_eq!(ctx.project_meta()?, project_meta);

    let changed = ProjectMeta {
        push_remote: Some("another-fork".into()),
        ..project_meta
    };
    ctx.set_project_meta(changed.clone())?;
    assert_eq!(ctx.project_meta()?, changed);
    Ok(())
}

#[test]
fn set_project_meta_fills_missing_target_commit_id_from_target_ref() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-base")?;
    let expected_target_id = {
        let mut target_ref = repo.find_reference("refs/remotes/origin/main")?;
        target_ref.peel_to_commit()?.id
    };
    let ctx = Context::from_repo_for_testing(repo)?;

    ctx.set_project_meta(ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: None,
        push_remote: Some("fork".into()),
    })?;

    assert_eq!(
        ctx.project_meta()?.target_commit_id,
        Some(expected_target_id),
        "migration should fill a missing target commit from the target ref tip"
    );
    Ok(())
}

#[test]
fn set_project_meta_preserves_existing_target_commit_id() -> anyhow::Result<()> {
    let (_tmp, repo, target_ref_tip) = run_fixture("project-meta-base")?;
    let stable_target = gix::ObjectId::from_hex(b"1111111111111111111111111111111111111111")?;
    assert_ne!(
        stable_target, target_ref_tip,
        "the fixture must detect repair"
    );
    let ctx = Context::from_repo_for_testing(repo)?;

    ctx.set_project_meta(ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(stable_target),
        push_remote: None,
    })?;

    assert_eq!(
        ctx.project_meta()?.target_commit_id,
        Some(stable_target),
        "an existing stable target must not move to the current ref tip"
    );
    Ok(())
}

#[test]
fn set_project_meta_clears_missing_target_ref() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo_for_testing(repo)?;

    ctx.set_project_meta(ProjectMeta {
        target_ref: Some("refs/remotes/origin/missing".try_into()?),
        target_commit_id: None,
        push_remote: Some("fork".into()),
    })?;

    assert_eq!(ctx.project_meta()?.target_ref, None);
    Ok(())
}

#[test]
fn project_meta_defaults_when_config_and_toml_are_unset() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo_for_testing(repo)?;

    let actual = ctx.project_meta()?;
    snapbox::assert_data_eq!(
        project_meta_summary(actual),
        snapbox::str!["target_ref=<unset>; target_commit_id=<unset>; push_remote=<unset>"]
    );
    Ok(())
}

#[test]
fn project_meta_observes_changes_made_through_other_repository_handles() -> anyhow::Result<()> {
    let (_tmp, repo, target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo_for_testing(repo)?;
    assert_eq!(ctx.project_meta()?.target_ref, None);

    // Write through an independent handle, like another process would.
    let other_ctx = Context::from_repo_for_testing(open_repo(&ctx.gitdir)?)?;
    other_ctx.set_project_meta(project_meta(
        target_commit_id,
        "refs/remotes/origin/main",
        "fork",
    )?)?;

    assert_eq!(
        ctx.project_meta()?.target_ref.map(|name| name.to_string()),
        Some("refs/remotes/origin/main".to_string()),
        "a long-lived context observes target changes made elsewhere"
    );
    Ok(())
}

#[test]
fn context_creation_ports_legacy_toml_before_cleanup() -> anyhow::Result<()> {
    let (_tmp, repo, target_commit_id) = run_fixture("project-meta-toml")?;
    let ctx = Context::from_repo_for_testing(repo)?;
    let expected = project_meta(target_commit_id, "refs/remotes/origin/main", "fork")?;

    assert_eq!(ctx.project_meta()?, expected);

    fs::write(
        ctx.project_data_dir().join("virtual_branches.toml"),
        "[branches]\n",
    )?;
    let reopened = Context::from_repo_for_testing(open_repo(&ctx.gitdir)?)?;
    assert_eq!(reopened.project_meta()?, expected);
    Ok(())
}

#[test]
fn concurrent_context_creation_ports_legacy_toml() -> anyhow::Result<()> {
    let (_tmp, repo, target_commit_id) = run_fixture("project-meta-toml")?;
    let gitdir = repo.git_dir().to_owned();
    let barrier = Arc::new(Barrier::new(9));
    let repos = (0..8)
        .map(|_| open_repo(&gitdir))
        .collect::<Result<Vec<_>, _>>()?;

    let threads = repos
        .into_iter()
        .map(|repo| {
            let barrier = barrier.clone();
            thread::spawn(move || -> anyhow::Result<()> {
                barrier.wait();
                Context::from_repo_for_testing(repo)?;
                Ok(())
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();

    for thread in threads {
        thread.join().expect("context creation does not panic")?;
    }
    assert_eq!(
        ProjectMeta::resolve(&repo)?,
        project_meta(target_commit_id, "refs/remotes/origin/main", "fork")?,
        "legacy project metadata is ported"
    );
    Ok(())
}

#[test]
fn context_creation_preserves_unmarked_project_config() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-ported")?;
    but_core::git_config::edit_repo_config(&repo, gix::config::Source::Local, |config| {
        but_core::git_config::remove_config_value(config, "gitbutler.project.portedMeta")
    })?;
    fs::write(
        repo.git_dir().join("gitbutler/virtual_branches.toml"),
        "invalid legacy metadata",
    )?;

    let ctx = Context::from_repo_for_testing(repo)?;
    snapbox::assert_data_eq!(
        project_meta_summary(ctx.project_meta()?),
        snapbox::str![
            "target_ref=refs/remotes/upstream/trunk; target_commit_id=[OID]; push_remote=origin"
        ]
    );
    Ok(())
}

#[test]
fn project_meta_reads_git_config_and_ignores_stale_toml() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-ported")?;
    let ctx = Context::from_repo_for_testing(repo)?;

    let actual = ctx.project_meta()?;
    snapbox::assert_data_eq!(
        project_meta_summary(actual),
        snapbox::str![
            "target_ref=refs/remotes/upstream/trunk; target_commit_id=[OID]; push_remote=origin"
        ]
    );
    Ok(())
}

fn run_fixture(name: &str) -> anyhow::Result<(TempDir, gix::Repository, gix::ObjectId)> {
    let (repo, repo_dir) = but_testsupport::writable_scenario(name);
    let target_commit_id = repo.rev_parse_single("HEAD")?.detach();
    Ok((repo_dir, repo, target_commit_id))
}

fn project_meta(
    target_commit_id: gix::ObjectId,
    target_ref: &str,
    push_remote: &str,
) -> anyhow::Result<ProjectMeta> {
    Ok(ProjectMeta {
        target_ref: Some(target_ref.try_into()?),
        target_commit_id: Some(target_commit_id),
        push_remote: Some(push_remote.to_owned()),
    })
}

fn project_meta_summary(project_meta: ProjectMeta) -> String {
    format!(
        "target_ref={}; target_commit_id={}; push_remote={}",
        project_meta
            .target_ref
            .as_ref()
            .map_or("<unset>".into(), ToString::to_string),
        project_meta.target_commit_id.map_or("<unset>", |_| "[OID]"),
        project_meta.push_remote.as_deref().unwrap_or("<unset>")
    )
}

#[test]
fn worktree_adoption_archives_preexisting_worktrees() -> anyhow::Result<()> {
    let root = TempDir::new()?;
    gix::init(root.path().join("main"))?;
    let repo = open_repo(&root.path().join("main"))?;
    but_testsupport::invoke_bash(
        "git commit --allow-empty -m M
         git worktree add -b feat-a ../wt-a
         git worktree add -b feat-b ../wt-b",
        &repo,
    );
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;

    assert_eq!(
        active_names(&ctx)?,
        Vec::<String>::new(),
        "the first-ever read adopts, archiving all pre-existing worktrees"
    );

    but_testsupport::invoke_bash(
        "git worktree add -b feat-c ../wt-c
         git -C ../wt-c commit --allow-empty -m C1",
        &*ctx.repo.get()?,
    );
    let active = ctx.active_worktrees()?;
    assert_eq!(
        active
            .iter()
            .map(|wt| wt.name.to_string())
            .collect::<Vec<_>>(),
        ["wt-c"],
        "worktrees created after adoption are active by default"
    );
    let head = ctx
        .worktree_head("wt-c".into())?
        .expect("an active worktree on a born branch resolves a head");
    assert_eq!(
        head.ref_name
            .as_ref()
            .map(|name| name.as_bstr().to_string()),
        Some("refs/heads/feat-c".into()),
        "the checked-out branch is resolved for ref-first graph seeding"
    );
    assert_eq!(
        head.id,
        ctx.repo.get()?.rev_parse_single("feat-c")?.detach(),
        "the head is the worktree's own HEAD, which has advanced past the main one"
    );
    assert_eq!(
        active[0].path.canonicalize()?,
        root.path().join("wt-c").canonicalize()?,
        "the checkout path is reported, not the admin dir under .git/worktrees/"
    );
    assert_eq!(
        ctx.worktrees_with_state()?
            .iter()
            .map(|wt| (wt.name.to_string(), wt.archived))
            .collect::<Vec<_>>(),
        [
            ("wt-a".to_string(), true),
            ("wt-b".to_string(), true),
            ("wt-c".to_string(), false)
        ],
        "archived entries are returned with their flag set, not filtered out"
    );

    {
        let mut db = ctx.db.get_cache_mut()?;
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: b"wt-a".to_vec(),
            archived: false,
        })?;
    }
    assert_eq!(
        active_names(&ctx)?,
        ["wt-a", "wt-c"],
        "unarchiving makes a pre-existing worktree visible again"
    );
    Ok(())
}

fn active_names(ctx: &Context) -> anyhow::Result<Vec<String>> {
    Ok(ctx
        .active_worktrees()?
        .into_iter()
        .map(|wt| wt.name.to_string())
        .collect())
}

#[test]
fn setting_archived_state_before_the_first_read_survives_adoption() -> anyhow::Result<()> {
    let root = TempDir::new()?;
    gix::init(root.path().join("main"))?;
    let repo = open_repo(&root.path().join("main"))?;
    but_testsupport::invoke_bash(
        "git commit --allow-empty -m M
         git worktree add -b feat-a ../wt-a",
        &repo,
    );
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;

    // Un-archiving is the very first worktree operation this project ever sees, so
    // adoption still has to run. If it ran afterwards it would archive everything
    // on disk and quietly undo this.
    ctx.set_worktree_archived("wt-a".into(), false)?;

    assert_eq!(
        active_names(&ctx)?,
        ["wt-a"],
        "the explicit request outlives the adoption it triggered"
    );
    Ok(())
}

#[test]
fn worktree_manipulation_flag_gates_worktree_state() -> anyhow::Result<()> {
    let root = TempDir::new()?;
    gix::init(root.path().join("main"))?;
    let repo = open_repo(&root.path().join("main"))?;
    but_testsupport::invoke_bash(
        "git commit --allow-empty -m M
         git worktree add -b feat-a ../wt-a",
        &repo,
    );

    // Flag off: nothing is returned and no adoption side-effects happen.
    let ctx = Context::from_repo_for_testing(repo.clone())?;
    assert_eq!(active_names(&ctx)?, Vec::<String>::new());
    {
        let db = ctx.db.get_cache_mut()?;
        assert!(
            !db.worktree_meta().adoption_ran()?,
            "flag off must not adopt"
        );
        assert_eq!(
            db.worktree_meta().list()?.len(),
            0,
            "flag off must not touch the worktree_meta table"
        );
    }

    // Flag on: the first read adopts (archives) the pre-existing worktree.
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    assert_eq!(
        active_names(&ctx)?,
        Vec::<String>::new(),
        "pre-existing worktrees are archived at adoption"
    );
    let db = ctx.db.get_cache_mut()?;
    assert!(db.worktree_meta().adoption_ran()?);
    assert_eq!(
        db.worktree_meta().list()?.len(),
        1,
        "adoption records the pre-existing worktree as archived"
    );
    Ok(())
}

#[test]
fn worktree_adoption_with_zero_worktrees_is_persisted() -> anyhow::Result<()> {
    let root = TempDir::new()?;
    gix::init(root.path().join("main"))?;
    let repo = open_repo(&root.path().join("main"))?;
    but_testsupport::invoke_bash("git commit --allow-empty -m M", &repo);
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;

    assert_eq!(active_names(&ctx)?, Vec::<String>::new());

    // The explicit marker keeps the adoption alive even though it archived
    // nothing, so a project's first worktree is active instead of being swept
    // into a re-run of adoption.
    but_testsupport::invoke_bash(
        "git worktree add -b feat ../wt-new
         git worktree add --detach ../wt-detached",
        &*ctx.repo.get()?,
    );
    assert_eq!(
        active_names(&ctx)?,
        ["wt-detached", "wt-new"],
        "worktrees created after adoption are active, detached ones included"
    );
    let detached = ctx
        .worktree_head("wt-detached".into())?
        .expect("a detached HEAD still resolves to its commit");
    assert_eq!(
        detached.ref_name, None,
        "a detached worktree resolves without a ref name"
    );
    Ok(())
}

#[test]
fn pruned_worktrees_are_adopted_but_not_returned() -> anyhow::Result<()> {
    let root = TempDir::new()?;
    gix::init(root.path().join("main"))?;
    let repo = open_repo(&root.path().join("main"))?;
    but_testsupport::invoke_bash(
        "git commit --allow-empty -m M
         git worktree add -b feat ../wt-gone",
        &repo,
    );
    std::fs::remove_dir_all(root.path().join("wt-gone"))?;

    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    assert_eq!(
        active_names(&ctx)?,
        Vec::<String>::new(),
        "a deleted checkout directory makes the worktree prunable, not active"
    );
    {
        let mut db = ctx.db.get_cache_mut()?;
        assert_eq!(
            db.worktree_meta().get(b"wt-gone")?.map(|row| row.archived),
            Some(true),
            "the unusable worktree was still archived at adoption"
        );
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: b"wt-gone".to_vec(),
            archived: false,
        })?;
    }
    assert_eq!(
        active_names(&ctx)?,
        Vec::<String>::new(),
        "even unarchived, a pruned checkout is excluded - it is unusable, not merely hidden"
    );
    Ok(())
}

#[test]
fn unborn_head_worktrees_enumerate_but_resolve_no_head() -> anyhow::Result<()> {
    let root = TempDir::new()?;
    gix::init(root.path().join("main"))?;
    let repo = open_repo(&root.path().join("main"))?;
    but_testsupport::invoke_bash(
        "git commit --allow-empty -m M
         git worktree add -b feat ../wt-unborn
         git -C ../wt-unborn symbolic-ref HEAD refs/heads/never-born",
        &repo,
    );

    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    assert_eq!(
        active_names(&ctx)?,
        Vec::<String>::new(),
        "the unborn worktree was archived at adoption like any other"
    );
    {
        let mut db = ctx.db.get_cache_mut()?;
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: b"wt-unborn".to_vec(),
            archived: false,
        })?;
    }
    assert_eq!(
        active_names(&ctx)?,
        ["wt-unborn"],
        "the worktree itself enumerates - its checkout exists on disk"
    );
    assert!(
        ctx.worktree_head("wt-unborn".into())?.is_none(),
        "but an unborn HEAD has no commit to resolve - consumers skip it, not an error"
    );
    Ok(())
}

#[test]
fn workspace_ref_worktrees_enumerate_but_never_resolve_or_seed() -> anyhow::Result<()> {
    let root = TempDir::new()?;
    gix::init(root.path().join("main"))?;
    let repo = open_repo(&root.path().join("main"))?;
    but_testsupport::invoke_bash(
        "git commit --allow-empty -m M
         git branch gitbutler/workspace
         git worktree add ../wt-ws gitbutler/workspace",
        &repo,
    );

    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    assert_eq!(active_names(&ctx)?, Vec::<String>::new());
    {
        let mut db = ctx.db.get_cache_mut()?;
        assert_eq!(
            db.worktree_meta().get(b"wt-ws")?.map(|row| row.archived),
            Some(true),
            "a workspace-ref worktree is still adopted like any other"
        );
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: b"wt-ws".to_vec(),
            archived: false,
        })?;
    }
    assert_eq!(
        active_names(&ctx)?,
        ["wt-ws"],
        "the worktree itself enumerates - its checkout exists on disk"
    );
    assert!(
        ctx.worktree_head("wt-ws".into())?.is_none(),
        "but a worktree on the workspace ref never resolves a head - \
         GitButler manages that ref itself"
    );
    assert_eq!(
        ctx.graph_options(Default::default())?.worktree_tips.len(),
        0,
        "and so it is never seeded into graph traversal"
    );
    Ok(())
}

#[test]
fn worktree_state_is_unreachable_from_linked_worktree_contexts() -> anyhow::Result<()> {
    let root = TempDir::new()?;
    gix::init(root.path().join("main"))?;
    let repo = open_repo(&root.path().join("main"))?;
    but_testsupport::invoke_bash(
        "git commit --allow-empty -m M
         git worktree add -b feat-a ../wt-a",
        &repo,
    );

    // A context opened inside a linked worktree stores its database in the
    // worktree's private git dir - adoption and archived state written there
    // would silently diverge from the main worktree's database.
    let mut ctx = Context::from_repo_for_testing(open_repo(&root.path().join("wt-a"))?)?;
    assert_eq!(
        active_names(&ctx)?,
        Vec::<String>::new(),
        "with the flag off even a linked-worktree context returns nothing, without erroring"
    );
    assert!(
        ctx.workspace_and_db().is_ok(),
        "with the flag off, workspace building in a linked worktree is unaffected"
    );
    ctx.settings.feature_flags.worktree_manipulation = true;
    assert!(
        ctx.worktrees_with_state()
            .unwrap_err()
            .to_string()
            .contains("main worktree"),
        "linked-worktree contexts must be refused, not given their own state"
    );
    // A fresh context bypasses the workspace cached by the flag-off call above.
    // Workspace building inherits the refusal - seeding must not read diverging state.
    let mut ctx = Context::from_repo_for_testing(open_repo(&root.path().join("wt-a"))?)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    snapbox::assert_data_eq!(
        ctx.workspace_and_db()
            .err()
            .map(|err| err.to_string())
            .unwrap_or_default(),
        snapbox::str![
            "worktree state must be read from the main worktree - a linked-worktree context has its own database, letting adoption and archived state diverge"
        ]
    );

    // The same holds for worktrees of a bare repository, whose git dirs contain
    // no `.git` path component for kind heuristics to latch onto.
    but_testsupport::invoke_bash_at_dir(
        "git clone --bare main bare.git
         git -C bare.git worktree add -b feat-bare ../wt-bare",
        root.path(),
    );
    let mut ctx = Context::from_repo_for_testing(open_repo(&root.path().join("wt-bare"))?)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    assert!(
        ctx.worktrees_with_state()
            .unwrap_err()
            .to_string()
            .contains("main worktree"),
        "a worktree of a bare repository is still a linked-worktree context"
    );
    Ok(())
}

#[test]
fn workspace_from_head_seeds_active_worktree_tips() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-seeding");
    let workspace_graph = |ctx: &Context| -> anyhow::Result<String> {
        let (_guard, _repo, ws, _db) = ctx.workspace_and_db()?;
        Ok(graph_tree(&ws.graph).to_string())
    };
    let db_state = |ctx: &Context| -> anyhow::Result<String> {
        let db = ctx.db.get_cache_mut()?;
        Ok(format!(
            "adopted: {}, rows: {:?}",
            db.worktree_meta().adoption_ran()?,
            db.worktree_meta()
                .list()?
                .into_iter()
                .map(|row| (
                    String::from_utf8_lossy(&row.name).into_owned(),
                    row.archived
                ))
                .collect::<Vec<_>>()
        ))
    };

    // Flag off: no tips are seeded and the database is untouched.
    let ctx = Context::from_repo_for_testing(repo.clone())?;
    snapbox::assert_data_eq!(
        workspace_graph(&ctx)?,
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    └── 🏁·85efbe4 (⌂|1)

"#]]
    );
    snapbox::assert_data_eq!(db_state(&ctx)?, snapbox::str!["adopted: false, rows: []"]);

    // Flag on: the first workspace build adopts, archiving the pre-existing
    // worktrees - archived worktrees are not seeded.
    let mut ctx = Context::from_repo_for_testing(repo.clone())?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    snapbox::assert_data_eq!(
        workspace_graph(&ctx)?,
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    └── 🏁·85efbe4 (⌂|1)

"#]]
    );
    snapbox::assert_data_eq!(
        db_state(&ctx)?,
        snapbox::str![[r#"adopted: true, rows: [("wt-a", true), ("wt-b", true)]"#]]
    );

    // Unarchiving makes a worktree active - like one created after adoption -
    // and only then is it seeded; the fresh context bypasses the per-context
    // workspace cache.
    {
        let mut db = ctx.db.get_cache_mut()?;
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: b"wt-b".to_vec(),
            archived: false,
        })?;
    }
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    let options = ctx.graph_options(Default::default())?;
    assert_eq!(
        options
            .worktree_tips
            .iter()
            .map(|tip| tip.name.to_string())
            .collect::<Vec<_>>(),
        ["wt-b"],
        "graph options preserve the stable name of each active worktree"
    );
    snapbox::assert_data_eq!(
        workspace_graph(&ctx)?,
        snapbox::str![[r#"

└── ►:1[0]:feat-b[📁wt-b]
    └── ·7d7d38f (⌂)
        └── 👉►:0[1]:main[🌳@repo]
            └── 🏁·85efbe4 (⌂|1)

"#]]
    );
    Ok(())
}
