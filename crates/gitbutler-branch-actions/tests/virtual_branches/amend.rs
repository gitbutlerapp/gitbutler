use but_oxidize::ObjectIdExt;
use but_workspace::{DiffSpec, HunkHeader};
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::list_commit_files;
use gitbutler_testsupport::stack_details;

use super::*;

#[test]
fn forcepush_allowed() -> anyhow::Result<()> {
    let Test {
        data_dir,
        repo,
        project_id,

        ctx,
        ..
    } = &mut Test::default();

    gitbutler_project::update_with_path(
        data_dir.as_ref().unwrap(),
        projects::UpdateRequest::default_with_id(*project_id),
    )
    .unwrap();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    gitbutler_project::update_with_path(
        data_dir.as_ref().unwrap(),
        projects::UpdateRequest::default_with_id(*project_id),
    )
    .unwrap();

    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let commit_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    gitbutler_branch_actions::stack::push_stack(
        ctx,
        stack_entry.id,
        false,
        false,
        stack_entry.name().map(|s| s.to_string()).unwrap(),
        false, // run_hooks
        vec![],
    )
    .unwrap();

    {
        // amend another hunk
        fs::write(repo.path().join("file2.txt"), "content2").unwrap();
        // let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        let to_amend = vec![DiffSpec {
            previous_path: None,
            path: "file2.txt".into(),
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 0,
                new_start: 1,
                new_lines: 1,
            }],
        }];
        gitbutler_branch_actions::amend(ctx, stack_entry.id, commit_id, to_amend).unwrap();

        let (_, b) = stack_details(ctx)
            .into_iter()
            .find(|s| s.0 == stack_entry.id)
            .unwrap();
        assert_eq!(b.branch_details[0].commits.len(), 1);
        assert_eq!(
            list_commit_files(ctx, b.branch_details[0].commits[0].id.to_git2())?.len(),
            2
        );
    }
    Ok(())
}

#[test]
fn non_locked_hunk() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|s| s.0 == stack_entry.id)
        .unwrap();
    assert_eq!(b.branch_details[0].commits.len(), 1);

    {
        // amend another hunk
        fs::write(repo.path().join("file2.txt"), "content2").unwrap();
        // let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        let to_amend = vec![DiffSpec {
            previous_path: None,
            path: "file2.txt".into(),
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 0,
                new_start: 1,
                new_lines: 1,
            }],
        }];
        gitbutler_branch_actions::amend(ctx, stack_entry.id, commit_oid, to_amend).unwrap();

        let (_, b) = stack_details(ctx)
            .into_iter()
            .find(|s| s.0 == stack_entry.id)
            .unwrap();
        assert_eq!(b.branch_details[0].commits.len(), 1);
        assert_eq!(
            list_commit_files(ctx, b.branch_details[0].commits[0].id.to_git2())?.len(),
            2
        );
    }
    Ok(())
}

#[test]
fn locked_hunk() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|s| s.0 == stack_entry.id)
        .unwrap();
    assert_eq!(b.branch_details[0].commits.len(), 1);
    assert_eq!(
        list_commit_files(ctx, b.branch_details[0].commits[0].id.to_git2())?[0].hunks[0].diff_lines,
        "@@ -0,0 +1 @@\n+content\n\\ No newline at end of file\n"
    );

    {
        // amend another hunk
        fs::write(repo.path().join("file.txt"), "more content").unwrap();
        // let to_amend: BranchOwnershipClaims = "file.txt:1-2".parse().unwrap();
        let to_amend = vec![DiffSpec {
            previous_path: None,
            path: "file.txt".into(),
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
            }],
        }];
        gitbutler_branch_actions::amend(ctx, stack_entry.id, commit_oid, to_amend).unwrap();

        let (_, b) = stack_details(ctx)
            .into_iter()
            .find(|s| s.0 == stack_entry.id)
            .unwrap();
        assert_eq!(b.branch_details[0].commits.len(), 1);
        assert_eq!(
            list_commit_files(ctx, b.branch_details[0].commits[0].id.to_git2())?[0].hunks[0]
                .diff_lines,
            "@@ -0,0 +1 @@\n+more content\n\\ No newline at end of file\n"
        );
    }
    Ok(())
}

#[test]
fn non_existing_ownership() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(
        ctx,
        &"refs/remotes/origin/master".parse().unwrap(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    let stack_entry = gitbutler_branch_actions::create_virtual_branch(
        ctx,
        &BranchCreateRequest::default(),
        ctx.project().exclusive_worktree_access().write_permission(),
    )
    .unwrap();

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    let (_, b) = stack_details(ctx)
        .into_iter()
        .find(|s| s.0 == stack_entry.id)
        .unwrap();
    assert_eq!(b.branch_details[0].commits.len(), 1);

    {
        // amend non existing hunk
        // let to_amend: BranchOwnershipClaims = "file2.txt:1-2".parse().unwrap();
        let to_amend = vec![DiffSpec {
            previous_path: None,
            path: "file2.txt".into(),
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 0,
                new_start: 1,
                new_lines: 1,
            }],
        }];
        assert_eq!(
            gitbutler_branch_actions::amend(ctx, stack_entry.id, commit_oid, to_amend)
                .unwrap_err()
                .to_string(),
            r#"Failed to amend with commit engine. Rejected specs: [(NoEffectiveChanges, DiffSpec { previous_path: None, path: "file2.txt", hunk_headers: [HunkHeader("-1,0", "+1,1")] })]"#,
        );
    }
}
