use but_workspace::commit_engine::{DiffSpec, HunkHeader};
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_branch_actions::list_commit_files;

use super::*;

#[test]
fn forcepush_allowed() -> anyhow::Result<()> {
    let Test {
        repo,
        project_id,

        projects,
        ctx,
        ..
    } = &Test::default();

    projects
        .update(&projects::UpdateRequest {
            id: *project_id,
            ..Default::default()
        })
        .unwrap();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    projects
        .update(&projects::UpdateRequest {
            id: *project_id,
            ..Default::default()
        })
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let commit_id =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    #[allow(deprecated)]
    gitbutler_branch_actions::push_virtual_branch(ctx, stack_entry.id, false, None).unwrap();

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

        let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
            .unwrap()
            .branches
            .into_iter()
            .find(|b| b.id == stack_entry.id)
            .unwrap();
        assert!(branch.requires_force);
        assert_eq!(branch.series[0].clone()?.patches.len(), 1);
        assert_eq!(branch.files.len(), 0);
        assert_eq!(
            list_commit_files(ctx, branch.series[0].clone()?.patches[0].id)?.len(),
            2
        );
    }
    Ok(())
}

#[test]
fn forcepush_forbidden() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    gitbutler_branch_actions::update_virtual_branch(
        ctx,
        BranchUpdateRequest {
            id: stack_entry.id,
            allow_rebasing: Some(false),
            ..Default::default()
        },
    )
    .unwrap();

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    #[allow(deprecated)]
    gitbutler_branch_actions::push_virtual_branch(ctx, stack_entry.id, false, None).unwrap();

    {
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
        assert_eq!(
            gitbutler_branch_actions::amend(ctx, stack_entry.id, commit_oid, to_amend)
                .unwrap_err()
                .to_string(),
            "force-push is not allowed"
        );
    }
}

#[test]
fn non_locked_hunk() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();
    assert_eq!(branch.series[0].clone()?.patches.len(), 1);
    assert_eq!(branch.files.len(), 0);

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

        let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
            .unwrap()
            .branches
            .into_iter()
            .find(|b| b.id == stack_entry.id)
            .unwrap();
        assert_eq!(branch.series[0].clone()?.patches.len(), 1);
        assert_eq!(branch.files.len(), 0);
        assert_eq!(
            list_commit_files(ctx, branch.series[0].clone()?.patches[0].id)?.len(),
            2
        );
    }
    Ok(())
}

#[test]
fn locked_hunk() -> anyhow::Result<()> {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();
    assert_eq!(branch.series[0].clone()?.patches.len(), 1);
    assert_eq!(branch.files.len(), 0);
    assert_eq!(
        list_commit_files(ctx, branch.series[0].clone()?.patches[0].id)?[0].hunks[0].diff_lines,
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

        let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
            .unwrap()
            .branches
            .into_iter()
            .find(|b| b.id == stack_entry.id)
            .unwrap();

        assert_eq!(branch.series[0].clone()?.patches.len(), 1);
        assert_eq!(branch.files.len(), 0);
        assert_eq!(
            list_commit_files(ctx, branch.series[0].clone()?.patches[0].id)?[0].hunks[0].diff_lines,
            "@@ -0,0 +1 @@\n+more content\n\\ No newline at end of file\n"
        );
    }
    Ok(())
}

#[test]
fn non_existing_ownership() {
    let Test { repo, ctx, .. } = &Test::default();

    gitbutler_branch_actions::set_base_branch(ctx, &"refs/remotes/origin/master".parse().unwrap())
        .unwrap();

    let stack_entry =
        gitbutler_branch_actions::create_virtual_branch(ctx, &BranchCreateRequest::default())
            .unwrap();

    // create commit
    fs::write(repo.path().join("file.txt"), "content").unwrap();
    let commit_oid =
        gitbutler_branch_actions::create_commit(ctx, stack_entry.id, "commit one", None).unwrap();

    let branch = gitbutler_branch_actions::list_virtual_branches(ctx)
        .unwrap()
        .branches
        .into_iter()
        .find(|b| b.id == stack_entry.id)
        .unwrap();
    assert_eq!(branch.series[0].clone().unwrap().patches.len(), 1);
    assert_eq!(branch.files.len(), 0);

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
