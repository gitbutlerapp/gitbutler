use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn twice() {
    let data_dir = paths::data_dir();

    let test_project = TestProject::default();

    {
        let project = gitbutler_project::add_with_path(data_dir.path(), test_project.path())
            .expect("failed to add project")
            .unwrap_project();
        let ctx = Context::new_from_legacy_project_and_settings(&project, AppSettings::default());

        gitbutler_branch_actions::set_base_branch(
            &ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            ctx.exclusive_worktree_access().write_permission(),
        )
        .unwrap();
        let stacks = stack_details(&ctx);
        assert_eq!(stacks.len(), 0);
        gitbutler_project::delete_with_path(data_dir.path(), project.id).unwrap();
    }

    {
        let project = gitbutler_project::add_with_path(data_dir.path(), test_project.path())
            .unwrap()
            .unwrap_project();
        let ctx = Context::new_from_legacy_project_and_settings(&project, AppSettings::default());
        gitbutler_branch_actions::set_base_branch(
            &ctx,
            &"refs/remotes/origin/master".parse().unwrap(),
            ctx.exclusive_worktree_access().write_permission(),
        )
        .unwrap();

        // even though project is on gitbutler/workspace, we should not import it
        let stacks = stack_details(&ctx);
        assert_eq!(stacks.len(), 0);
    }
}

#[test]
fn dirty_non_target() {
    // a situation when you initialize project while being on the local verison of the master
    // that has uncommited changes.
    let Test { repo, ctx, .. } = &Test::default();

    repo.checkout(&"refs/heads/some-feature".parse().unwrap());

    fs::write(repo.path().join("file.txt"), "content").unwrap();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.derived_name, "some-feature");
}

#[test]
fn dirty_target() {
    // a situation when you initialize project while being on the local verison of the master
    // that has uncommited changes.
    let Test { repo, ctx, .. } = &Test::default();

    fs::write(repo.path().join("file.txt"), "content").unwrap();

    let old = std::env::var("GIT_AUTHOR_NAME").ok();
    unsafe { std::env::set_var("GIT_AUTHOR_NAME", "GitButler") };
    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.derived_name, "g-branch-1");
    if let Some(old) = old {
        unsafe {
            std::env::set_var("GIT_AUTHOR_NAME", old);
        }
    }
}

#[test]
fn commit_on_non_target_local() {
    let Test { repo, ctx, .. } = &Test::default();

    repo.checkout(&"refs/heads/some-feature".parse().unwrap());
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    repo.commit_all("commit on target");

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.derived_name, "some-feature");
}

#[test]
fn commit_on_non_target_remote() {
    let Test { repo, ctx, .. } = &Test::default();

    repo.checkout(&"refs/heads/some-feature".parse().unwrap());
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    repo.commit_all("commit on target");
    repo.push_branch(&"refs/heads/some-feature".parse().unwrap());

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.derived_name, "some-feature");
    assert_eq!(stacks[0].1.branch_details[0].clone().commits.len(), 1);
}

#[test]
fn commit_on_target() {
    let Test { repo, ctx, .. } = &Test::default();

    let old = std::env::var("GIT_AUTHOR_NAME").ok();
    unsafe {
        std::env::set_var("GIT_AUTHOR_NAME", "GitButler");
    }

    fs::write(repo.path().join("file.txt"), "content").unwrap();
    repo.commit_all("commit on target");

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].1.derived_name, "g-branch-1");
    assert_eq!(stacks[0].1.branch_details[0].clone().commits.len(), 1);
    if let Some(old) = old {
        unsafe {
            std::env::set_var("GIT_AUTHOR_NAME", old);
        }
    }
}

#[test]
fn submodule() {
    let Test { repo, ctx, .. } = &Test::default();

    let test_project = TestProject::default();
    let submodule_url: gitbutler_url::Url =
        test_project.path().display().to_string().parse().unwrap();
    repo.add_submodule(&submodule_url, path::Path::new("submodule"));

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stacks = stack_details(ctx);
    assert_eq!(stacks.len(), 1);
}
