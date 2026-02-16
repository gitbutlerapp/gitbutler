use bstr::ByteSlice;
use but_testsupport::{git, invoke_bash_at_dir};
use but_worktrees::{
    integrate::{WorktreeIntegrationStatus, worktree_integrate, worktree_integration_status},
    new::worktree_new,
};

use crate::util::test_ctx;

#[test]
fn create_unrelated_change_and_reintroduce() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-branches")?;
    let mut ctx = test_ctx.ctx;

    let mut guard = ctx.exclusive_worktree_access();

    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
    let a = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;

    invoke_bash_at_dir(
        r#"echo "foo" > qux.txt && git add . && git commit -am "added qux!""#,
        &a.created.path,
    );

    assert_eq!(
        worktree_integration_status(
            &mut ctx,
            guard.read_permission(),
            &a.created.id,
            feature_a_name.as_ref()
        )?,
        WorktreeIntegrationStatus::Integratable {
            cherry_pick_conflicts: false,
            commits_above_conflict: false,
            working_dir_conflicts: false
        },
        "We should be able to integrate the unrelated change back into the original reference"
    );
    assert_eq!(
        worktree_integration_status(
            &mut ctx,
            guard.read_permission(),
            &a.created.id,
            feature_b_name.as_ref()
        )?,
        WorktreeIntegrationStatus::Integratable {
            cherry_pick_conflicts: false,
            commits_above_conflict: false,
            working_dir_conflicts: false
        },
        "We should also be able to integrate the unrelated change back into the above reference"
    );

    worktree_integrate(
        &mut ctx,
        guard.write_permission(),
        &a.created.id,
        feature_a_name.as_ref(),
    )
    .expect("it works");

    let repo = ctx.repo.get()?;
    insta::assert_snapshot!(but_testsupport::visualize_tree(repo.head_tree_id()?), @r#"
    c5bb3ff
    ├── bar.txt:100644:91c021a "feature-b line 2\n"
    ├── file.txt:100644:f2376e2 "initial content\n"
    ├── foo.txt:100644:bf8cf71 "feature-b line 1\n"
    └── qux.txt:100644:257cc56 "foo\n"
    "#);

    // cannot show hashes as these aren't controllable yet.
    // TODO: when making this 'modern', ensure we fully isolate it.
    let unstable_log = git(&repo)
        .args(["log", "--graph", "--pretty=format:%s %d"])
        .output()?
        .stdout
        .as_bstr()
        .to_owned();
    insta::assert_snapshot!(unstable_log, @r"
    * GitButler Workspace Commit  (HEAD -> gitbutler/workspace)
    * feature-b: add line 2  (feature-b)
    * feature-b: add line 1 
    * Integrated worktree  (feature-a)
    * feature-a: add line 2 
    * feature-a: add line 1 
    * init  (origin/main, main)
    ");

    Ok(())
}

#[test]
fn causes_conflicts_above() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-branches")?;
    let mut ctx = test_ctx.ctx;

    let mut guard = ctx.exclusive_worktree_access();

    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
    let a = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;

    invoke_bash_at_dir(
        r#"echo "foo" > foo.txt && git add . && git commit -am "added conflicts above!""#,
        &a.created.path,
    );

    assert_eq!(
        worktree_integration_status(
            &mut ctx,
            guard.read_permission(),
            &a.created.id,
            feature_a_name.as_ref()
        )?,
        WorktreeIntegrationStatus::Integratable {
            cherry_pick_conflicts: false,
            commits_above_conflict: true,
            working_dir_conflicts: false
        },
        "When integrating into feature-a, it should cause the commits above which touch foo.txt to conflict"
    );
    assert_eq!(
        worktree_integration_status(
            &mut ctx,
            guard.read_permission(),
            &a.created.id,
            feature_b_name.as_ref()
        )?,
        WorktreeIntegrationStatus::Integratable {
            cherry_pick_conflicts: true,
            commits_above_conflict: false,
            working_dir_conflicts: false
        },
        "When integrating into feature-b, the resulting commit should end up conflicted"
    );

    worktree_integrate(
        &mut ctx,
        guard.write_permission(),
        &a.created.id,
        feature_a_name.as_ref(),
    )
    .expect("it works");

    let repo = ctx.repo.get()?;
    insta::assert_snapshot!(but_testsupport::visualize_tree(repo.head_tree_id()?), @r#"
    762a113
    ├── bar.txt:100644:91c021a "feature-b line 2\n"
    ├── file.txt:100644:f2376e2 "initial content\n"
    └── foo.txt:100644:257cc56 "foo\n"
    "#);

    // TODO: make hashes of integrated commits stable.
    let unstable_log = git(&repo)
        .args(["log", "--graph", "--pretty=format:%s %d"])
        .output()?
        .stdout
        .as_bstr()
        .to_owned();
    insta::assert_snapshot!(unstable_log, @r"
    * GitButler Workspace Commit  (HEAD -> gitbutler/workspace)
    * feature-b: add line 2  (feature-b)
    * feature-b: add line 1 
    * Integrated worktree  (feature-a)
    * feature-a: add line 2 
    * feature-a: add line 1 
    * init  (origin/main, main)
    ");

    Ok(())
}

#[test]
fn causes_workdir_conflicts_simple() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-branches")?;
    let mut ctx = test_ctx.ctx;
    ctx.settings.feature_flags.cv3 = false;
    let main_worktree_dir = ctx.workdir()?.expect("non-bare");

    let mut guard = ctx.exclusive_worktree_access();

    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
    let b = worktree_new(&mut ctx, guard.read_permission(), feature_b_name.as_ref())?;

    invoke_bash_at_dir(r#"echo "qux" > foo.txt"#, &main_worktree_dir);
    invoke_bash_at_dir(
        r#"echo "foo" > foo.txt && git add . && git commit -am "added conflicts above!""#,
        &b.created.path,
    );

    assert_eq!(
        worktree_integration_status(
            &mut ctx,
            guard.read_permission(),
            &b.created.id,
            feature_b_name.as_ref()
        )?,
        WorktreeIntegrationStatus::Integratable {
            cherry_pick_conflicts: false,
            commits_above_conflict: false,
            working_dir_conflicts: true
        },
        "In this case, we're putting a new commit on the top of the stack - the thing that should conflict is the working directory"
    );

    worktree_integrate(
        &mut ctx,
        guard.write_permission(),
        &b.created.id,
        feature_b_name.as_ref(),
    )
    .expect("it works");

    let foo = std::fs::read_to_string(main_worktree_dir.join("foo.txt"))?;
    insta::assert_snapshot!(foo, @r"
    <<<<<<< ours
    qux
    ||||||| ancestor
    feature-b line 1
    =======
    foo
    >>>>>>> theirs
    ");

    Ok(())
}

#[test]
fn causes_workdir_conflicts_complex() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-branches")?;
    let mut ctx = test_ctx.ctx;
    ctx.settings.feature_flags.cv3 = false;
    let main_worktree_dir = ctx.workdir()?.expect("non-bare");

    let mut guard = ctx.exclusive_worktree_access();

    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
    let a = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;

    std::fs::write(main_worktree_dir.join("foo.txt"), "qux\n")?;
    invoke_bash_at_dir(
        r#"echo "foo" > foo.txt && git add . && git commit -am "added conflicts above!""#,
        &a.created.path,
    );
    assert_eq!(
        worktree_integration_status(
            &mut ctx,
            guard.read_permission(),
            &a.created.id,
            feature_a_name.as_ref()
        )?,
        WorktreeIntegrationStatus::Integratable {
            cherry_pick_conflicts: false,
            commits_above_conflict: true,
            working_dir_conflicts: true
        },
        "When integrating into feature-a, it should cause the commits above which touch foo.txt and the worktree to conflict"
    );
    assert_eq!(
        worktree_integration_status(
            &mut ctx,
            guard.read_permission(),
            &a.created.id,
            feature_b_name.as_ref()
        )?,
        WorktreeIntegrationStatus::Integratable {
            cherry_pick_conflicts: true,
            commits_above_conflict: false,
            working_dir_conflicts: false
        },
        "When integrating into feature-b, because the thing that commits is the cherry on top of the source, it auto-resolves to what was originally there, resulting in the working_dir not conflicting"
    );

    worktree_integrate(
        &mut ctx,
        guard.write_permission(),
        &a.created.id,
        feature_a_name.as_ref(),
    )
    .expect("it works");

    let foo = std::fs::read_to_string(main_worktree_dir.join("foo.txt"))?;
    insta::assert_snapshot!(foo, @r"
    <<<<<<< ours
    qux
    ||||||| ancestor
    feature-b line 1
    =======
    foo
    >>>>>>> theirs
    ");

    Ok(())
}

#[test]
fn causes_workspace_conflict() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-and-parallel")?;
    let mut ctx = test_ctx.ctx;

    let guard = ctx.exclusive_worktree_access();

    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
    let feature_c_name = gix::refs::FullName::try_from("refs/heads/feature-c")?;
    let c = worktree_new(&mut ctx, guard.read_permission(), feature_c_name.as_ref())?;

    invoke_bash_at_dir(
        r#"echo "foo" >> file.txt && git add . && git commit -am "added conflicts above!""#,
        &c.created.path,
    );

    assert_eq!(
        worktree_integration_status(
            &mut ctx,
            guard.read_permission(),
            &c.created.id,
            feature_c_name.as_ref()
        )?,
        WorktreeIntegrationStatus::CausesWorkspaceConflicts,
        "When integrating into feature-c, because we modified a file that sits in feature a & b, it causes the workspace to conflict"
    );
    assert_eq!(
        worktree_integration_status(
            &mut ctx,
            guard.read_permission(),
            &c.created.id,
            feature_b_name.as_ref()
        )?,
        WorktreeIntegrationStatus::Integratable {
            cherry_pick_conflicts: true,
            commits_above_conflict: false,
            working_dir_conflicts: false
        },
        "When integrating into feature-b, we can cherry pick, but it will conflict"
    );
    assert_eq!(
        worktree_integration_status(
            &mut ctx,
            guard.read_permission(),
            &c.created.id,
            feature_a_name.as_ref()
        )?,
        WorktreeIntegrationStatus::Integratable {
            cherry_pick_conflicts: true,
            commits_above_conflict: false,
            working_dir_conflicts: false
        },
        "When integrating into feature-a, we can cherry pick, but it will conflict"
    );

    Ok(())
}
