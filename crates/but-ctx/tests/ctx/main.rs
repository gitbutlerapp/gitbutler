use std::{
    fmt::Write as _,
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
use snapbox::ToDebug as _;

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
    let (repo, _tmp) = but_testsupport::writable_scenario("unborn-empty");
    let ctx = Context::from_repo_for_testing(repo)?;

    let sync = ctx.to_sync();
    let restored = sync.into_thread_local();
    assert_eq!(ctx.project_data_dir(), restored.project_data_dir());
    Ok(())
}

#[test]
fn discover_with_app_channel_uses_requested_project_data_dir() -> anyhow::Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("storage-path-per-channel");
    let workdir = repo.workdir().expect("fixture is non-bare");
    let project_data_dir = |channel: AppChannel| -> anyhow::Result<String> {
        let ctx = Context::discover_with_app_channel(workdir, channel)?;
        let project_data_dir = ctx.project_data_dir();
        Ok(project_data_dir
            .strip_prefix(&ctx.gitdir)?
            .display()
            .to_string())
    };

    but_testsupport::isolated_app_data_dir(|| {
        snapbox::assert_data_eq!(
            project_data_dir(AppChannel::Nightly)?,
            snapbox::str!["gitbutler-nightly"]
        );
        snapbox::assert_data_eq!(
            project_data_dir(AppChannel::Dev)?,
            snapbox::str!["gitbutler-dev"]
        );
        Ok(())
    })
}

#[test]
fn set_project_meta_persists_git_config() -> anyhow::Result<()> {
    let (_tmp, repo, target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo_for_testing(repo)?;
    let project_meta = project_meta(target_commit_id, "refs/remotes/origin/main", "fork")?;

    snapbox::assert_data_eq!(
        project_meta_summary(ctx.project_meta()?),
        snapbox::str!["target_ref=<unset>; target_commit_id=<unset>; push_remote=<unset>"]
    );

    ctx.set_project_meta(project_meta.clone())?;
    snapbox::assert_data_eq!(
        project_meta_summary(ctx.project_meta()?),
        snapbox::str![
            "target_ref=refs/remotes/origin/main; target_commit_id=d5d98f5; push_remote=fork"
        ]
    );

    let changed = ProjectMeta {
        push_remote: Some("another-fork".into()),
        ..project_meta
    };
    ctx.set_project_meta(changed)?;
    snapbox::assert_data_eq!(
        project_meta_summary(ctx.project_meta()?),
        snapbox::str![
            "target_ref=refs/remotes/origin/main; target_commit_id=d5d98f5; push_remote=another-fork"
        ]
    );
    Ok(())
}

#[test]
fn set_project_meta_fills_missing_target_commit_id_from_target_ref() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-base")?;
    let ctx = Context::from_repo_for_testing(repo)?;

    ctx.set_project_meta(ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: None,
        push_remote: Some("fork".into()),
    })?;

    // Migration fills a missing target commit from the target ref tip, which the
    // fixture points at its only commit.
    snapbox::assert_data_eq!(
        project_meta_summary(ctx.project_meta()?),
        snapbox::str![
            "target_ref=refs/remotes/origin/main; target_commit_id=d5d98f5; push_remote=fork"
        ]
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

    // An existing stable target must not move to the current ref tip.
    snapbox::assert_data_eq!(
        project_meta_summary(ctx.project_meta()?),
        snapbox::str![
            "target_ref=refs/remotes/origin/main; target_commit_id=1111111; push_remote=<unset>"
        ]
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

    snapbox::assert_data_eq!(
        project_meta_summary(ctx.project_meta()?),
        snapbox::str!["target_ref=<unset>; target_commit_id=<unset>; push_remote=fork"]
    );
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
    snapbox::assert_data_eq!(
        project_meta_summary(ctx.project_meta()?),
        snapbox::str!["target_ref=<unset>; target_commit_id=<unset>; push_remote=<unset>"]
    );

    // Write through an independent handle, like another process would.
    let other_ctx = Context::from_repo_for_testing(open_repo(&ctx.gitdir)?)?;
    other_ctx.set_project_meta(project_meta(
        target_commit_id,
        "refs/remotes/origin/main",
        "fork",
    )?)?;

    // A long-lived context observes target changes made elsewhere.
    snapbox::assert_data_eq!(
        project_meta_summary(ctx.project_meta()?),
        snapbox::str![
            "target_ref=refs/remotes/origin/main; target_commit_id=d5d98f5; push_remote=fork"
        ]
    );
    Ok(())
}

#[test]
fn context_creation_ports_legacy_toml_before_cleanup() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-toml")?;
    let ctx = Context::from_repo_for_testing(repo)?;

    snapbox::assert_data_eq!(
        project_meta_summary(ctx.project_meta()?),
        snapbox::str![
            "target_ref=refs/remotes/origin/main; target_commit_id=d5d98f5; push_remote=fork"
        ]
    );

    fs::write(
        ctx.project_data_dir().join("virtual_branches.toml"),
        "[branches]\n",
    )?;
    let reopened = Context::from_repo_for_testing(open_repo(&ctx.gitdir)?)?;
    snapbox::assert_data_eq!(
        project_meta_summary(reopened.project_meta()?),
        snapbox::str![
            "target_ref=refs/remotes/origin/main; target_commit_id=d5d98f5; push_remote=fork"
        ]
    );
    Ok(())
}

#[test]
fn concurrent_context_creation_ports_legacy_toml() -> anyhow::Result<()> {
    let (_tmp, repo, _target_commit_id) = run_fixture("project-meta-toml")?;
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
    // Legacy project metadata is ported.
    snapbox::assert_data_eq!(
        project_meta_summary(ProjectMeta::resolve(&repo)?),
        snapbox::str![
            "target_ref=refs/remotes/origin/main; target_commit_id=d5d98f5; push_remote=fork"
        ]
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
            "target_ref=refs/remotes/upstream/trunk; target_commit_id=d5d98f5; push_remote=origin"
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
            "target_ref=refs/remotes/upstream/trunk; target_commit_id=d5d98f5; push_remote=origin"
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
        project_meta
            .target_commit_id
            .map_or("<unset>".into(), |id| id.to_hex_with_len(7).to_string()),
        project_meta.push_remote.as_deref().unwrap_or("<unset>")
    )
}

#[test]
fn worktree_adoption_archives_preexisting_worktrees() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-seeding");
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;

    // The first-ever read adopts, archiving all pre-existing worktrees.
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str!["[]"]);
    snapbox::assert_data_eq!(
        db_state(&ctx)?,
        snapbox::str![[r#"adopted: true, rows: [("wt-a", true), ("wt-b", true)]"#]]
    );

    but_testsupport::invoke_bash(
        "git worktree add -b feat-c wt-c
         git -C wt-c commit --allow-empty -m C1",
        &*ctx.repo.get()?,
    );
    // A worktree created after adoption is active by default, archived ones are
    // returned with their flag set rather than filtered out, and the path is the
    // checkout, not the admin dir under `.git/worktrees/`.
    snapbox::assert_data_eq!(
        worktree_entries(&ctx)?,
        snapbox::str![[r#"
wt-a archived=true path=wt-a
wt-b archived=true path=wt-b
wt-c archived=false path=wt-c

"#]]
    );
    // The checked-out branch is resolved for ref-first graph seeding, at the
    // worktree's own HEAD which has advanced past the main one.
    snapbox::assert_data_eq!(
        ctx.worktree_head("wt-c".into())?.to_debug(),
        snapbox::str![[r#"
Some(
    WorktreeHead {
        ref_name: Some(
            FullName(
                "refs/heads/feat-c",
            ),
        ),
        id: Sha1(339e6f33492cc35912cd386fe831b8123a3fa1d0),
    },
)

"#]]
    );

    {
        let mut db = ctx.db.get_cache_mut()?;
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: b"wt-a".to_vec(),
            archived: false,
        })?;
    }
    // Unarchiving makes a pre-existing worktree visible again.
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str![[r#"["wt-a", "wt-c"]"#]]);
    Ok(())
}

fn active_names(ctx: &Context) -> anyhow::Result<String> {
    let names: Vec<_> = ctx
        .worktrees_with_state()?
        .into_iter()
        .filter(|wt| !wt.archived)
        .map(|wt| wt.name.to_string())
        .collect();
    Ok(format!("{names:?}"))
}

fn worktree_entries(ctx: &Context) -> anyhow::Result<String> {
    let workdir = gix::path::realpath(ctx.workdir_or_gitdir()?)?;
    let mut out = String::new();
    for wt in ctx.worktrees_with_state()? {
        let path = gix::path::realpath(&wt.path)?;
        let path = path.strip_prefix(&workdir).unwrap_or(path.as_path());
        writeln!(
            out,
            "{} archived={} path={}",
            wt.name,
            wt.archived,
            path.display()
        )?;
    }
    Ok(out)
}

fn db_state(ctx: &Context) -> anyhow::Result<String> {
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
}

fn workspace_graph(ctx: &Context) -> anyhow::Result<String> {
    let (_guard, _repo, ws, _db) = ctx.workspace_and_db()?;
    Ok(graph_tree(&ws.graph).to_string())
}

#[test]
fn setting_archived_state_before_the_first_read_survives_adoption() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-seeding");
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;

    // Un-archiving is the very first worktree operation this project ever sees, so
    // adoption still has to run. If it ran afterwards it would archive everything
    // on disk and quietly undo this.
    ctx.set_worktree_archived("wt-a".into(), false)?;

    // The explicit request outlives the adoption it triggered.
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str![[r#"["wt-a"]"#]]);
    snapbox::assert_data_eq!(
        db_state(&ctx)?,
        snapbox::str![[r#"adopted: true, rows: [("wt-a", false), ("wt-b", true)]"#]]
    );
    Ok(())
}

#[test]
fn worktree_manipulation_flag_gates_worktree_state() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-seeding");

    // Flag off: nothing is returned and no adoption side-effects happen.
    let ctx = Context::from_repo_for_testing(repo.clone())?;
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str!["[]"]);
    snapbox::assert_data_eq!(db_state(&ctx)?, snapbox::str!["adopted: false, rows: []"]);

    // Flag on: the first read adopts, archiving the pre-existing worktrees.
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str!["[]"]);
    snapbox::assert_data_eq!(
        db_state(&ctx)?,
        snapbox::str![[r#"adopted: true, rows: [("wt-a", true), ("wt-b", true)]"#]]
    );
    Ok(())
}

#[test]
fn worktree_adoption_with_zero_worktrees_is_persisted() -> anyhow::Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("no-worktrees");
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;

    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str!["[]"]);
    // The explicit marker keeps the adoption alive even though it archived nothing.
    snapbox::assert_data_eq!(db_state(&ctx)?, snapbox::str!["adopted: true, rows: []"]);

    // So a project's first worktrees are active instead of being swept into a
    // re-run of adoption, detached ones included.
    but_testsupport::invoke_bash(
        "git worktree add -b feat wt-new
         git worktree add --detach wt-detached",
        &*ctx.repo.get()?,
    );
    snapbox::assert_data_eq!(
        active_names(&ctx)?,
        snapbox::str![[r#"["wt-detached", "wt-new"]"#]]
    );
    // A detached HEAD still resolves to its commit, without a ref name.
    snapbox::assert_data_eq!(
        ctx.worktree_head("wt-detached".into())?.to_debug(),
        snapbox::str![[r#"
Some(
    WorktreeHead {
        ref_name: None,
        id: Sha1(85efbe4d5a663bff0ed8fb5fbc38a72be0592f55),
    },
)

"#]]
    );
    Ok(())
}

#[test]
fn pruned_worktrees_are_adopted_but_not_returned() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktrees-without-heads");
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;

    // A deleted checkout directory makes `wt-gone` prunable: it is not returned at
    // all, yet it was still archived at adoption.
    snapbox::assert_data_eq!(
        worktree_entries(&ctx)?,
        snapbox::str![[r#"
wt-unborn archived=true path=wt-unborn
wt-ws archived=true path=wt-ws

"#]]
    );
    snapbox::assert_data_eq!(
        db_state(&ctx)?,
        snapbox::str![[
            r#"adopted: true, rows: [("wt-gone", true), ("wt-unborn", true), ("wt-ws", true)]"#
        ]]
    );

    {
        let mut db = ctx.db.get_cache_mut()?;
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: b"wt-gone".to_vec(),
            archived: false,
        })?;
    }
    // Even unarchived, a pruned checkout is excluded - it is unusable, not merely hidden.
    snapbox::assert_data_eq!(
        worktree_entries(&ctx)?,
        snapbox::str![[r#"
wt-unborn archived=true path=wt-unborn
wt-ws archived=true path=wt-ws

"#]]
    );
    snapbox::assert_data_eq!(
        db_state(&ctx)?,
        snapbox::str![[
            r#"adopted: true, rows: [("wt-gone", false), ("wt-unborn", true), ("wt-ws", true)]"#
        ]]
    );
    Ok(())
}

#[test]
fn rows_of_worktrees_git_forgot_are_pruned_on_read() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-seeding");
    let workdir = repo.workdir().expect("fixture is non-bare").to_owned();
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str!["[]"]);
    snapbox::assert_data_eq!(
        db_state(&ctx)?,
        snapbox::str![[r#"adopted: true, rows: [("wt-a", true), ("wt-b", true)]"#]]
    );

    // A deleted checkout leaves the worktree prunable, but git still knows it and
    // so does the database; a removed worktree is forgotten by both.
    std::fs::remove_dir_all(workdir.join("wt-a"))?;
    but_testsupport::invoke_bash("git worktree remove wt-b", &*ctx.repo.get()?);
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str!["[]"]);
    snapbox::assert_data_eq!(
        db_state(&ctx)?,
        snapbox::str![[r#"adopted: true, rows: [("wt-a", true)]"#]]
    );

    but_testsupport::invoke_bash(
        "git worktree prune
         git worktree add wt-b feat-b",
        &*ctx.repo.get()?,
    );
    // A worktree created under a forgotten name starts out active.
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str![[r#"["wt-b"]"#]]);
    snapbox::assert_data_eq!(db_state(&ctx)?, snapbox::str!["adopted: true, rows: []"]);
    Ok(())
}

#[test]
fn unborn_head_worktrees_enumerate_but_resolve_no_head() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktrees-without-heads");
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    // The unborn worktree was archived at adoption like any other.
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str!["[]"]);
    {
        let mut db = ctx.db.get_cache_mut()?;
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: b"wt-unborn".to_vec(),
            archived: false,
        })?;
    }
    // The worktree itself enumerates - its checkout exists on disk.
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str![[r#"["wt-unborn"]"#]]);
    // But an unborn HEAD has no commit to resolve - consumers skip it, not an error.
    snapbox::assert_data_eq!(
        ctx.worktree_head("wt-unborn".into())?.to_debug(),
        snapbox::str![[r#"
None

"#]]
    );
    Ok(())
}

#[test]
fn workspace_ref_worktrees_enumerate_but_never_resolve_or_seed() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktrees-without-heads");
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    // A workspace-ref worktree is still adopted like any other.
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str!["[]"]);
    snapbox::assert_data_eq!(
        db_state(&ctx)?,
        snapbox::str![[
            r#"adopted: true, rows: [("wt-gone", true), ("wt-unborn", true), ("wt-ws", true)]"#
        ]]
    );
    {
        let mut db = ctx.db.get_cache_mut()?;
        db.worktree_meta_mut().upsert(but_db::WorktreeMeta {
            name: b"wt-ws".to_vec(),
            archived: false,
        })?;
    }
    // The worktree itself enumerates - its checkout exists on disk.
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str![[r#"["wt-ws"]"#]]);
    // But a worktree on the workspace ref never resolves a head - GitButler
    // manages that ref itself.
    snapbox::assert_data_eq!(
        ctx.worktree_head("wt-ws".into())?.to_debug(),
        snapbox::str![[r#"
None

"#]]
    );
    // And so it is never seeded into graph traversal.
    let (_guard, _repo, ws, _db) = ctx.workspace_and_db()?;
    snapbox::assert_data_eq!(
        ws.graph.worktree_tips.to_debug(),
        snapbox::str![[r#"
[]

"#]]
    );
    Ok(())
}

#[test]
fn worktree_state_is_unreachable_from_linked_worktree_contexts() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-seeding");
    let wt_a = repo.workdir().expect("fixture is non-bare").join("wt-a");

    // A context opened inside a linked worktree stores its database in the
    // worktree's private git dir - adoption and archived state written there
    // would silently diverge from the main worktree's database.
    let mut ctx = Context::from_repo_for_testing(open_repo(&wt_a)?)?;
    // With the flag off even a linked-worktree context returns nothing, without
    // erroring, and workspace building is unaffected.
    snapbox::assert_data_eq!(active_names(&ctx)?, snapbox::str!["[]"]);
    snapbox::assert_data_eq!(
        workspace_graph(&ctx)?,
        snapbox::str![[r#"

└── 👉►:0[0]:feat-a[📁wt-a@repo]
    └── ·7076dee (⌂)
        └── ►:1[1]:main[🌳]
            └── 🏁·85efbe4 (⌂)

"#]]
    );
    ctx.settings.feature_flags.worktree_manipulation = true;
    // Linked-worktree contexts are refused, not given their own state.
    snapbox::assert_data_eq!(
        ctx.worktrees_with_state().unwrap_err().to_string(),
        snapbox::str![
            "worktree state must be read from the main worktree - a linked-worktree context has its own database, letting adoption and archived state diverge"
        ]
    );
    // A fresh context bypasses the workspace cached by the flag-off call above.
    // Workspace building inherits the refusal - seeding must not read diverging state.
    let mut ctx = Context::from_repo_for_testing(open_repo(&wt_a)?)?;
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
    let (repo, _tmp) = writable_scenario_slow("bare-clone-worktree");
    let wt_bare = repo.workdir().expect("fixture is non-bare").join("wt-bare");
    let mut ctx = Context::from_repo_for_testing(open_repo(&wt_bare)?)?;
    ctx.settings.feature_flags.worktree_manipulation = true;
    snapbox::assert_data_eq!(
        ctx.worktrees_with_state().unwrap_err().to_string(),
        snapbox::str![
            "worktree state must be read from the main worktree - a linked-worktree context has its own database, letting adoption and archived state diverge"
        ]
    );
    Ok(())
}

#[test]
fn workspace_from_head_seeds_active_worktree_tips() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-seeding");

    // Flag off: no tips are seeded and the database is untouched.
    let ctx = Context::from_repo_for_testing(repo.clone())?;
    snapbox::assert_data_eq!(
        workspace_graph(&ctx)?,
        snapbox::str![[r#"

└── 👉►:0[0]:main[🌳]
    └── 🏁·85efbe4 (⌂)

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
    └── 🏁·85efbe4 (⌂)

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
    {
        let (_guard, _repo, ws, _db) = ctx.workspace_and_db()?;
        assert_eq!(
            ws.graph
                .worktree_tips
                .iter()
                .map(|tip| tip.name.to_string())
                .collect::<Vec<_>>(),
            ["wt-b"],
            "the graph preserves the stable name of each active worktree"
        );
    }
    // The worktree branch never owns its commit: it is an empty segment
    // forking onto the anonymous commit-owning segment below.
    snapbox::assert_data_eq!(
        workspace_graph(&ctx)?,
        snapbox::str![[r#"

└── ►:2[0]:feat-b[📁wt-b]
    └── ►:1[1]:anon:
        └── ·7d7d38f (⌂)
            └── 👉►:0[2]:main[🌳@repo]
                └── 🏁·85efbe4 (⌂)

"#]]
    );
    Ok(())
}
