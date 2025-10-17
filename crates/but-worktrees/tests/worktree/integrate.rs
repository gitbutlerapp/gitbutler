use std::path::Path;

use anyhow::bail;
use but_testsupport::git;
use but_worktrees::{
    integrate::{WorktreeIntegrationStatus, worktree_integrate, worktree_integration_status},
    new::worktree_new,
};

use crate::util::{IntoString, test_ctx};

#[test]
fn test_create_unrelated_change_and_reintroduce() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-branches")?;
    let mut ctx = test_ctx.ctx;
    let repo = ctx.gix_repo()?;

    let mut guard = ctx.project().exclusive_worktree_access();

    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
    let a = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;

    bash_at(
        &a.created.path,
        r#"echo "foo" > qux.txt && git add . && git commit -am "added qux!""#,
    )?;

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
        "We should be able to integrate the unrelated change back into the origional reference"
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

    assert!(
        worktree_integrate(
            &mut ctx,
            guard.write_permission(),
            &a.created.id,
            feature_a_name.as_ref()
        )
        .is_ok()
    );

    let head_tree = git(&repo).args(["ls-tree", "HEAD"]).output_string()?;
    insta::assert_snapshot!(head_tree, @r"
    100644 blob 91c021af6e6e0d11aca2fb7f57b82818dfa9ad7c	bar.txt
    100644 blob f2376e2bab6c5194410bd8a55630f83f933d2f34	file.txt
    100644 blob bf8cf71260eca4acb27694afed34c3aadc8761d1	foo.txt
    100644 blob 257cc5642cb1a054f08cc83f2d943e56fd3ebe99	qux.txt
    ");

    let log = git(&repo)
        .args(["log", "--graph", "--pretty=format:%s %d"])
        .output_string()?;
    insta::assert_snapshot!(log, @r"
    * GitButler Workspace Commit  (HEAD -> gitbutler/workspace)
    * feature-b: add line 2  (feature-b)
    * feature-b: add line 1 
    * Integrated worktree  (feature-a)
    * feature-a: add line 2 
    * feature-a: add line 1 
    * init  (origin/main, origin/HEAD, main, gitbutler/target)
    ");

    Ok(())
}

#[test]
fn test_causes_conflicts_above() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-branches")?;
    let mut ctx = test_ctx.ctx;
    let repo = ctx.gix_repo()?;

    let mut guard = ctx.project().exclusive_worktree_access();

    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
    let a = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;

    bash_at(
        &a.created.path,
        r#"echo "foo" > foo.txt && git add . && git commit -am "added conflicts above!""#,
    )?;

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

    assert!(
        worktree_integrate(
            &mut ctx,
            guard.write_permission(),
            &a.created.id,
            feature_a_name.as_ref()
        )
        .is_ok()
    );

    let head_tree = git(&repo).args(["ls-tree", "HEAD"]).output_string()?;
    insta::assert_snapshot!(head_tree, @r"
    100644 blob 91c021af6e6e0d11aca2fb7f57b82818dfa9ad7c	bar.txt
    100644 blob f2376e2bab6c5194410bd8a55630f83f933d2f34	file.txt
    100644 blob 257cc5642cb1a054f08cc83f2d943e56fd3ebe99	foo.txt
    ");

    let foo = git(&repo)
        .args(["cat-file", "-p", "HEAD^{tree}:foo.txt"])
        .output_string()?;
    insta::assert_snapshot!(foo, @"foo");

    let log = git(&repo)
        .args(["log", "--graph", "--pretty=format:%s %d"])
        .output_string()?;
    insta::assert_snapshot!(log, @r"
    * GitButler Workspace Commit  (HEAD -> gitbutler/workspace)
    * feature-b: add line 2  (feature-b)
    * feature-b: add line 1 
    * Integrated worktree  (feature-a)
    * feature-a: add line 2 
    * feature-a: add line 1 
    * init  (origin/main, origin/HEAD, main, gitbutler/target)
    ");

    Ok(())
}

#[test]
fn test_causes_workdir_conflicts_simple() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-branches")?;
    let mut ctx = test_ctx.ctx;
    let path = ctx.project().path.clone();

    let mut guard = ctx.project().exclusive_worktree_access();

    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
    let b = worktree_new(&mut ctx, guard.read_permission(), feature_b_name.as_ref())?;

    bash_at(&path, r#"echo "qux" > foo.txt"#)?;
    bash_at(
        &b.created.path,
        r#"echo "foo" > foo.txt && git add . && git commit -am "added conflicts above!""#,
    )?;

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

    assert!(
        worktree_integrate(
            &mut ctx,
            guard.write_permission(),
            &b.created.id,
            feature_b_name.as_ref()
        )
        .is_ok()
    );

    let foo = bash_at(&path, "cat foo.txt")?;
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
fn test_causes_workdir_conflicts_complex() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-branches")?;
    let mut ctx = test_ctx.ctx;
    let path = ctx.project().path.clone();

    let mut guard = ctx.project().exclusive_worktree_access();

    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
    let a = worktree_new(&mut ctx, guard.read_permission(), feature_a_name.as_ref())?;

    bash_at(&path, r#"echo "qux" > foo.txt"#)?;
    bash_at(
        &a.created.path,
        r#"echo "foo" > foo.txt && git add . && git commit -am "added conflicts above!""#,
    )?;

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
        "When integrating into feature-b, because the thing that commits is the cherry on top of the source, it auto-resolves to what was origionally there, resulting in the working_dir not conflicting"
    );

    assert!(
        worktree_integrate(
            &mut ctx,
            guard.write_permission(),
            &a.created.id,
            feature_a_name.as_ref()
        )
        .is_ok()
    );

    let foo = bash_at(&path, "cat foo.txt")?;
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
fn test_causes_workspace_conflict() -> anyhow::Result<()> {
    let test_ctx = test_ctx("stacked-and-parallel")?;
    let mut ctx = test_ctx.ctx;

    let guard = ctx.project().exclusive_worktree_access();

    let feature_a_name = gix::refs::FullName::try_from("refs/heads/feature-a")?;
    let feature_b_name = gix::refs::FullName::try_from("refs/heads/feature-b")?;
    let feature_c_name = gix::refs::FullName::try_from("refs/heads/feature-c")?;
    let c = worktree_new(&mut ctx, guard.read_permission(), feature_c_name.as_ref())?;

    bash_at(
        &c.created.path,
        r#"echo "foo" >> file.txt && git add . && git commit -am "added conflicts above!""#,
    )?;

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

fn bash_at(path: &Path, command: &str) -> anyhow::Result<String> {
    let output = std::process::Command::from(gix::command::prepare("bash"))
        .current_dir(path)
        .arg("-c")
        .arg(command)
        .output()?;
    if output.status.success() {
        Ok(std::str::from_utf8(&output.stdout)?.to_owned())
    } else {
        bail!(
            "Failed running {}\n\nStdout:\n{}\n\nStderr:\n{}",
            command,
            std::str::from_utf8(&output.stdout)?,
            std::str::from_utf8(&output.stderr)?
        );
    }
}
