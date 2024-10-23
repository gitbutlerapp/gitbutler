pub mod file_ownership;
pub mod ownership;

use anyhow::Result;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_patch_reference::{CommitOrChangeId, PatchReference};
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::{LogUntil, RepositoryExt as _};
use gitbutler_repo_actions::RepoActionsExt;
use gitbutler_stack::VirtualBranchesHandle;
use gitbutler_stack::{PatchReferenceUpdate, TargetUpdate};
use itertools::Itertools;
use tempfile::TempDir;

#[test]
fn init_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let mut branch = test_ctx.branch;
    let result = branch.initialize(&ctx); // this is noop really
    assert!(result.is_ok());
    assert!(branch.initialized());
    assert_eq!(branch.heads.len(), 1);
    assert_eq!(branch.heads[0].name, "a-branch-2"); // matches the stack name
    assert_eq!(
        branch.heads[0].target,
        CommitOrChangeId::ChangeId(
            ctx.repository()
                .find_commit(branch.head())?
                .change_id()
                .unwrap()
        )
    );
    // Assert persisted
    assert_eq!(branch, test_ctx.handle.get_branch(branch.id)?);
    Ok(())
}

#[test]
fn init_already_initialized_noop() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let mut branch = test_ctx.branch;
    let result = branch.initialize(&ctx);
    assert!(result.is_ok());
    let result = branch.initialize(&ctx);
    assert!(result.is_ok()); // noop
    Ok(())
}

#[test]
fn add_series_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let reference = PatchReference {
        name: "asdf".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
        description: Some("my description".into()),
    };
    let result = test_ctx.branch.add_series(&ctx, reference, None);
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads.len(), 2);
    assert_eq!(test_ctx.branch.heads[0].name, "asdf");
    assert_eq!(
        test_ctx.branch.heads[0].description,
        Some("my description".into())
    );
    // Assert persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn add_series_top_of_stack() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let result =
        test_ctx
            .branch
            .add_series_top_of_stack(&ctx, "asdf".into(), Some("my description".into()));
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads.len(), 2);
    assert_eq!(test_ctx.branch.heads[1].name, "asdf");
    assert_eq!(
        test_ctx.branch.heads[1].description,
        Some("my description".into())
    );
    // Assert persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn add_series_top_base() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let merge_base = ctx.repository().find_commit(
        ctx.repository()
            .merge_base(test_ctx.branch.head(), test_ctx.default_target.sha)?,
    )?;
    let reference = PatchReference {
        name: "asdf".into(),
        target: CommitOrChangeId::CommitId(merge_base.id().to_string()),
        description: Some("my description".into()),
    };
    let result = test_ctx.branch.add_series(&ctx, reference, None);
    println!("{:?}", result);
    // Assert persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn add_multiple_series() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;

    assert_eq!(test_ctx.branch.heads.len(), 1);
    assert_eq!(head_names(&test_ctx), vec!["a-branch-2"]); // defaults to stack name
    let default_head = test_ctx.branch.heads[0].clone();

    let head_4 = PatchReference {
        name: "head_4".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits.last().unwrap().change_id().unwrap()),
        description: None,
    };
    let result = test_ctx
        .branch
        .add_series(&ctx, head_4, Some(default_head.name.clone()));
    assert!(result.is_ok());
    assert_eq!(head_names(&test_ctx), vec!["a-branch-2", "head_4"]);

    let head_2 = PatchReference {
        name: "head_2".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits.last().unwrap().change_id().unwrap()),
        description: None,
    };
    let result = test_ctx.branch.add_series(&ctx, head_2, None);
    assert!(result.is_ok());
    assert_eq!(
        head_names(&test_ctx),
        vec!["head_2", "a-branch-2", "head_4"]
    );

    let head_1 = PatchReference {
        name: "head_1".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits.first().unwrap().change_id().unwrap()),
        description: None,
    };

    let result = test_ctx.branch.add_series(&ctx, head_1, None);
    assert!(result.is_ok());
    assert_eq!(
        head_names(&test_ctx),
        vec!["head_1", "head_2", "a-branch-2", "head_4"]
    );
    Ok(())
}

#[test]
fn add_series_commit_id_when_change_id_available() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let reference = PatchReference {
        name: "asdf".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[1].id().to_string()),
        description: None,
    };
    let result = test_ctx.branch.add_series(&ctx, reference, None);
    assert_eq!(
        result.err().unwrap().to_string(),
        format!(
            "The commit {} has a change id associated with it. Use the change id instead",
            test_ctx.commits[1].id()
        )
    );
    Ok(())
}

#[test]
fn add_series_invalid_name_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let reference = PatchReference {
        name: "name with spaces".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
        description: None,
    };
    let result = test_ctx.branch.add_series(&ctx, reference, None);
    assert_eq!(result.err().unwrap().to_string(), "Invalid branch name");
    Ok(())
}

#[test]
fn add_series_duplicate_name_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let reference = PatchReference {
        name: "asdf".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
        description: None,
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert!(result.is_ok());
    let result = test_ctx.branch.add_series(&ctx, reference, None);
    assert_eq!(
        result.err().unwrap().to_string(),
        "A patch reference with the name asdf exists"
    );
    Ok(())
}

#[test]
fn add_series_matching_git_ref_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let reference = PatchReference {
        name: "existing-branch".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
        description: None,
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert_eq!(
        result.err().unwrap().to_string(),
        "A git reference with the name existing-branch exists"
    );
    Ok(())
}

#[test]
fn add_series_including_refs_head_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let reference = PatchReference {
        name: "refs/heads/my-branch".into(),
        target: CommitOrChangeId::CommitId(test_ctx.commits[0].id().to_string()),
        description: None,
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert_eq!(
        result.err().unwrap().to_string(),
        "Stack head name cannot start with 'refs/heads'"
    );
    Ok(())
}

#[test]
fn add_series_target_commit_doesnt_exist() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let reference = PatchReference {
        name: "my-branch".into(),
        target: CommitOrChangeId::CommitId("30696678319e0fa3a20e54f22d47fc8cf1ceaade".into()), // does not exist
        description: None,
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert!(result
        .err()
        .unwrap()
        .to_string()
        .contains("object not found"),);
    Ok(())
}

#[test]
fn add_series_target_change_id_doesnt_exist() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let reference = PatchReference {
        name: "my-branch".into(),
        target: CommitOrChangeId::ChangeId("does-not-exist".into()), // does not exist
        description: None,
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert_eq!(
        result.err().unwrap().to_string(),
        "No commit with change id does-not-exist found",
    );
    Ok(())
}

#[test]
fn add_series_target_commit_not_in_stack() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let other_commit_id = test_ctx.other_commits.last().unwrap().id().to_string();
    let reference = PatchReference {
        name: "my-branch".into(),
        target: CommitOrChangeId::CommitId(other_commit_id.clone()), // does not exist
        description: None,
    };
    let result = test_ctx.branch.add_series(&ctx, reference.clone(), None);
    assert_eq!(
        result.err().unwrap().to_string(),
        format!(
            "The commit {} is not between the stack head and the stack base",
            other_commit_id
        )
    );
    Ok(())
}

#[test]
fn remove_series_last_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let result = test_ctx
        .branch
        .remove_series(&ctx, test_ctx.branch.heads[0].name.clone());
    assert_eq!(
        result.err().unwrap().to_string(),
        "Cannot remove the last branch from the stack"
    );
    Ok(())
}

#[test]
fn remove_series_nonexistent_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let result = test_ctx
        .branch
        .remove_series(&ctx, "does-not-exist".to_string());
    assert_eq!(
        result.err().unwrap().to_string(),
        "Series with name does-not-exist not found"
    );
    Ok(())
}

#[test]
fn remove_series_with_multiple_last_heads() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;

    assert_eq!(test_ctx.branch.heads.len(), 1);
    assert_eq!(head_names(&test_ctx), vec!["a-branch-2"]); // defaults to stack name
    let default_head = test_ctx.branch.heads[0].clone();

    let to_stay = PatchReference {
        name: "to_stay".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits.last().unwrap().change_id().unwrap()),
        description: None,
    };
    let result = test_ctx.branch.add_series(&ctx, to_stay.clone(), None);
    assert!(result.is_ok());
    assert_eq!(head_names(&test_ctx), vec!["to_stay", "a-branch-2"]);

    let result = test_ctx
        .branch
        .remove_series(&ctx, default_head.name.clone());
    assert!(result.is_ok());
    assert_eq!(head_names(&test_ctx), vec!["to_stay"]);
    assert_eq!(
        test_ctx.branch.heads[0].target,
        CommitOrChangeId::ChangeId(test_ctx.commits.last().unwrap().change_id().unwrap())
    ); // it references the newest commit
    Ok(())
}

#[test]
fn remove_series_no_orphan_commits() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;

    assert_eq!(test_ctx.branch.heads.len(), 1);
    assert_eq!(head_names(&test_ctx), vec!["a-branch-2"]); // defaults to stack name
    let default_head = test_ctx.branch.heads[0].clone(); // references the newest commit

    let to_stay = PatchReference {
        name: "to_stay".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits.first().unwrap().change_id().unwrap()),
        description: None,
    }; // references the oldest commit
    let result = test_ctx.branch.add_series(&ctx, to_stay.clone(), None);
    assert!(result.is_ok());
    assert_eq!(head_names(&test_ctx), vec!["to_stay", "a-branch-2"]);

    let result = test_ctx
        .branch
        .remove_series(&ctx, default_head.name.clone());
    assert!(result.is_ok());
    assert_eq!(head_names(&test_ctx), vec!["to_stay"]);
    assert_eq!(
        test_ctx.branch.heads[0].target,
        CommitOrChangeId::ChangeId(test_ctx.commits.last().unwrap().change_id().unwrap())
    ); // it was updated to reference the newest commit
    Ok(())
}

#[test]
fn update_series_noop_does_nothing() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let heads_before = test_ctx.branch.heads.clone();
    let noop_update = PatchReferenceUpdate::default();
    let result = test_ctx
        .branch
        .update_series(&ctx, "a-branch-2".into(), &noop_update);
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads, heads_before);
    Ok(())
}

#[test]
fn update_series_name_fails_validation() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let update = PatchReferenceUpdate {
        name: Some("invalid name".into()),
        target_update: None,
        description: None,
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "a-branch-2".into(), &update);
    assert_eq!(result.err().unwrap().to_string(), "Invalid branch name");
    Ok(())
}

#[test]
fn update_series_name_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let update = PatchReferenceUpdate {
        name: Some("new-name".into()),
        target_update: None,
        description: None,
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "a-branch-2".into(), &update);
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads[0].name, "new-name");
    // Assert persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn update_series_set_description() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let update = PatchReferenceUpdate {
        name: None,
        target_update: None,
        description: Some(Some("my description".into())),
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "a-branch-2".into(), &update);
    assert!(result.is_ok());
    assert_eq!(
        test_ctx.branch.heads[0].description,
        Some("my description".into())
    );
    // Assert persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn update_series_target_fails_commit_not_in_stack() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let other_commit_id = test_ctx.other_commits.last().unwrap().id().to_string();
    let update = PatchReferenceUpdate {
        name: None,
        target_update: Some(TargetUpdate {
            target: CommitOrChangeId::CommitId(other_commit_id.clone()),
            preceding_head: None,
        }),
        description: None,
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "a-branch-2".into(), &update);
    assert_eq!(
        result.err().unwrap().to_string(),
        format!(
            "The commit {} is not between the stack head and the stack base",
            other_commit_id
        )
    );
    Ok(())
}

#[test]
fn update_series_target_orphan_commit_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let initial_state = test_ctx.branch.heads.clone();
    let first_commit_change_id = test_ctx.commits.first().unwrap().change_id().unwrap();
    let update = PatchReferenceUpdate {
        name: Some("new-lol".into()),
        target_update: Some(TargetUpdate {
            target: CommitOrChangeId::ChangeId(first_commit_change_id.clone()),
            preceding_head: None,
        }),
        description: None,
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "a-branch-2".into(), &update);

    assert_eq!(
        result.err().unwrap().to_string(),
        "This update would cause orphaned patches, which is disallowed"
    );
    assert_eq!(initial_state, test_ctx.branch.heads); // no change due to failure
    Ok(())
}

#[test]
fn update_series_target_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let commit_0_change_id = CommitOrChangeId::ChangeId(test_ctx.commits[0].change_id().unwrap());
    let series_1 = PatchReference {
        name: "series_1".into(),
        target: commit_0_change_id.clone(),
        description: None,
    };
    let result = test_ctx.branch.add_series(&ctx, series_1, None);
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads[0].target, commit_0_change_id);
    let commit_1_change_id = CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap());
    let update = PatchReferenceUpdate {
        name: None,
        target_update: Some(TargetUpdate {
            target: commit_1_change_id.clone(),
            preceding_head: None,
        }),
        description: None,
    };
    let result = test_ctx
        .branch
        .update_series(&ctx, "series_1".into(), &update);
    assert!(result.is_ok());
    assert_eq!(test_ctx.branch.heads[0].target, commit_1_change_id);
    // Assert persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn push_series_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;

    let state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let mut target = state.get_default_target()?;
    target.push_remote_name = Some("origin".into());
    state.set_default_target(target)?;

    let result = test_ctx.branch.push_details(&ctx, "a-branch-2".into());
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn update_name_after_push() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;

    let state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let mut target = state.get_default_target()?;
    target.push_remote_name = Some("origin".into());
    state.set_default_target(target)?;

    let push_details = test_ctx.branch.push_details(&ctx, "a-branch-2".into())?;
    let result = ctx.push(
        push_details.head,
        &push_details.remote_refname,
        false,
        None,
        Some(Some(test_ctx.branch.id)),
    );
    assert!(result.is_ok());
    let result = test_ctx.branch.update_series(
        &ctx,
        "a-branch-2".into(),
        &PatchReferenceUpdate {
            name: Some("new-name".into()),
            ..Default::default()
        },
    );
    assert!(result.is_ok());
    Ok(())
}

#[test]
fn list_series_default_head() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let result = test_ctx.branch.list_series(&ctx);
    assert!(result.is_ok());
    let result = result?;
    // the number of series matches the number of heads
    assert_eq!(result.len(), test_ctx.branch.heads.len());
    assert_eq!(result[0].head.name, "a-branch-2");
    assert_eq!(
        result[0].local_commits.iter().map(|c| c.id()).collect_vec(),
        test_ctx.commits.iter().map(|c| c.id()).collect_vec()
    );
    Ok(())
}

#[test]
fn list_series_two_heads_same_commit() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let head_before = PatchReference {
        name: "head_before".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits.last().unwrap().change_id().unwrap()),
        description: None,
    };
    // add `head_before` before the initial head
    let result = test_ctx.branch.add_series(&ctx, head_before, None);
    assert!(result.is_ok());

    let result = test_ctx.branch.list_series(&ctx);
    assert!(result.is_ok());
    let result = result?;

    // the number of series matches the number of heads
    assert_eq!(result.len(), test_ctx.branch.heads.len());

    assert_eq!(
        result[0].local_commits.iter().map(|c| c.id()).collect_vec(),
        test_ctx.commits.iter().map(|c| c.id()).collect_vec()
    );
    assert_eq!(result[0].head.name, "head_before");
    assert_eq!(
        result[1].local_commits.iter().map(|c| c.id()).collect_vec(),
        vec![]
    );
    assert_eq!(result[1].head.name, "a-branch-2");
    Ok(())
}

#[test]
fn list_series_two_heads_different_commit() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    test_ctx.branch.initialize(&ctx)?;
    let head_before = PatchReference {
        name: "head_before".into(),
        // point to the first commit
        target: CommitOrChangeId::ChangeId(test_ctx.commits.first().unwrap().change_id().unwrap()),
        description: None,
    };
    // add `head_before` before the initial head
    let result = test_ctx.branch.add_series(&ctx, head_before, None);
    assert!(result.is_ok());
    let result = test_ctx.branch.list_series(&ctx);
    assert!(result.is_ok());
    let result = result?;
    // the number of series matches the number of heads
    assert_eq!(result.len(), test_ctx.branch.heads.len());
    let mut expected_patches = test_ctx.commits.iter().map(|c| c.id()).collect_vec();
    assert_eq!(
        result[0].local_commits.iter().map(|c| c.id()).collect_vec(),
        vec![expected_patches.remove(0)]
    );
    assert_eq!(result[0].head.name, "head_before");
    assert_eq!(expected_patches.len(), 2);
    assert_eq!(
        result[1].local_commits.iter().map(|c| c.id()).collect_vec(),
        expected_patches
    ); // the other two patches are in the second series
    assert_eq!(result[1].head.name, "a-branch-2");

    Ok(())
}

#[test]
fn set_stack_head_commit_invalid() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let result = test_ctx
        .branch
        .set_stack_head(&ctx, git2::Oid::zero(), None);
    assert!(result.is_err());
    Ok(())
}

#[test]
fn set_stack_head() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let commit = test_ctx.other_commits.last().unwrap();
    let result = test_ctx.branch.set_stack_head(&ctx, commit.id(), None);
    assert!(result.is_ok());
    let result = test_ctx.branch.list_series(&ctx)?;
    assert_eq!(
        result.first().unwrap().head.target,
        CommitOrChangeId::ChangeId(commit.change_id().unwrap())
    );
    assert_eq!(
        test_ctx.branch.head(),
        test_ctx.other_commits.last().unwrap().id()
    );
    Ok(())
}

#[test]
fn replace_head_single() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let top_of_stack = test_ctx.branch.heads.last().unwrap().target.clone();
    let from_head = PatchReference {
        name: "from_head".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
        description: None,
    };
    test_ctx.branch.add_series(&ctx, from_head, None)?;
    // replace with previous head
    let result = test_ctx
        .branch
        .replace_head(&ctx, &test_ctx.commits[1], &test_ctx.commits[0]);
    assert!(result.is_ok());
    // the head is updated to point to the new commit
    assert_eq!(
        test_ctx.branch.heads[0].target,
        CommitOrChangeId::ChangeId(test_ctx.commits[0].change_id().unwrap())
    );
    // the top of the stack is not changed
    assert_eq!(test_ctx.branch.heads.last().unwrap().target, top_of_stack);
    // the state was persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn replace_head_single_with_merge_base() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let top_of_stack = test_ctx.branch.heads.last().unwrap().target.clone();
    let from_head = PatchReference {
        name: "from_head".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
        description: None,
    };
    test_ctx.branch.add_series(&ctx, from_head, None)?;
    // replace with merge base
    let merge_base = ctx.repository().find_commit(
        ctx.repository()
            .merge_base(test_ctx.branch.head(), test_ctx.default_target.sha)?,
    )?;
    let result = test_ctx
        .branch
        .replace_head(&ctx, &test_ctx.commits[1], &merge_base);
    assert!(result.is_ok());
    // the head is updated to point to the new commit
    // this time it's a commit id
    assert_eq!(
        test_ctx.branch.heads[0].target,
        CommitOrChangeId::CommitId(merge_base.id().to_string())
    );
    // the top of the stack is not changed
    assert_eq!(test_ctx.branch.heads.last().unwrap().target, top_of_stack);
    // the state was persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn replace_head_with_invalid_commit_error() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let from_head = PatchReference {
        name: "from_head".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
        description: None,
    };
    test_ctx.branch.add_series(&ctx, from_head, None)?;
    let stack = test_ctx.branch.clone();
    let result =
        test_ctx
            .branch
            .replace_head(&ctx, &test_ctx.commits[1], &test_ctx.other_commits[0]); //in another stack
    assert!(result.is_err());
    // is unmodified
    assert_eq!(stack, test_ctx.branch);
    // same in persistence
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn replace_head_with_same_noop() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let from_head = PatchReference {
        name: "from_head".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
        description: None,
    };
    test_ctx.branch.add_series(&ctx, from_head, None)?;
    let stack = test_ctx.branch.clone();
    let result = test_ctx
        .branch
        .replace_head(&ctx, &test_ctx.commits[1], &test_ctx.commits[1]);
    assert!(result.is_ok());
    // is unmodified
    assert_eq!(stack, test_ctx.branch);
    // same in persistence
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn replace_no_head_noop() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let stack = test_ctx.branch.clone();
    let result = test_ctx
        .branch
        .replace_head(&ctx, &test_ctx.commits[1], &test_ctx.commits[0]);
    assert!(result.is_ok());
    // is unmodified
    assert_eq!(stack, test_ctx.branch);
    // same in persistence
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn replace_non_member_commit_noop_no_error() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let stack = test_ctx.branch.clone();
    let result =
        test_ctx
            .branch
            .replace_head(&ctx, &test_ctx.other_commits[0], &test_ctx.commits[0]);
    assert!(result.is_ok());
    // is unmodified
    assert_eq!(stack, test_ctx.branch);
    Ok(())
}

#[test]
fn replace_top_of_stack_single() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let initial_head = ctx.repository().find_commit(test_ctx.branch.head())?;

    let result = test_ctx
        .branch
        .replace_head(&ctx, &initial_head, &test_ctx.commits[1]);
    assert!(result.is_ok());
    // the head is updated to point to the new commit
    assert_eq!(
        test_ctx.branch.heads[0].target,
        CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap())
    );
    assert_eq!(test_ctx.branch.head(), test_ctx.commits[1].id());
    assert_eq!(test_ctx.branch.heads.len(), 1);
    // the state was persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn replace_head_multiple() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let top_of_stack = test_ctx.branch.heads.last().unwrap().target.clone();
    let from_head_1 = PatchReference {
        name: "from_head_1".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
        description: None,
    };
    let from_head_2 = PatchReference {
        name: "from_head_2".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
        description: None,
    };
    // both references point to the same commit
    test_ctx.branch.add_series(&ctx, from_head_1, None)?;
    test_ctx
        .branch
        .add_series(&ctx, from_head_2, Some("from_head_1".into()))?;
    // replace the commit
    let result = test_ctx
        .branch
        .replace_head(&ctx, &test_ctx.commits[1], &test_ctx.commits[0]);
    assert!(result.is_ok());
    // both heads are  updated to point to the new commit
    assert_eq!(
        test_ctx.branch.heads[0].target,
        CommitOrChangeId::ChangeId(test_ctx.commits[0].change_id().unwrap())
    );
    assert_eq!(
        test_ctx.branch.heads[1].target,
        CommitOrChangeId::ChangeId(test_ctx.commits[0].change_id().unwrap())
    );
    // the top of the stack is not changed
    assert_eq!(test_ctx.branch.heads.last().unwrap().target, top_of_stack);
    // the state was persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn replace_head_top_of_stack_multiple() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let initial_head = ctx.repository().find_commit(test_ctx.branch.head())?;
    let extra_head = PatchReference {
        name: "extra_head".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
        description: None,
    };
    // an extra head just beneath the top of the stack
    test_ctx.branch.add_series(&ctx, extra_head, None)?;
    // replace top of stack the commit
    let result = test_ctx
        .branch
        .replace_head(&ctx, &initial_head, &test_ctx.commits[1]);
    assert!(result.is_ok());
    // both heads are  updated to point to the new commit
    assert_eq!(
        test_ctx.branch.heads[0].target,
        CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap())
    );
    assert_eq!(
        test_ctx.branch.heads[1].target,
        CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap())
    );
    assert_eq!(test_ctx.branch.head(), test_ctx.commits[1].id());
    // order is the same
    assert_eq!(test_ctx.branch.heads[0].name, "extra_head");
    // the state was persisted
    assert_eq!(
        test_ctx.branch,
        test_ctx.handle.get_branch(test_ctx.branch.id)?
    );
    Ok(())
}

#[test]
fn set_legacy_refname() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let remote_branch: RemoteRefname = "refs/remotes/origin/my-branch".parse()?;
    test_ctx.branch.upstream = Some(remote_branch.clone());
    test_ctx
        .branch
        .set_legacy_compatible_stack_reference(&ctx)?;
    // reference name was updated
    assert_eq!(test_ctx.branch.heads[0].name, "my-branch");
    Ok(())
}

#[test]
fn set_legacy_refname_no_upstream_set() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let initial_state = test_ctx.branch.clone();
    test_ctx
        .branch
        .set_legacy_compatible_stack_reference(&ctx)?;
    // no changes
    assert_eq!(initial_state, test_ctx.branch);
    Ok(())
}

#[test]
fn set_legacy_refname_multiple_heads() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let remote_branch: RemoteRefname = "refs/remotes/origin/my-branch".parse()?;
    test_ctx.branch.upstream = Some(remote_branch.clone());
    let extra_head = PatchReference {
        name: "extra_head".into(),
        target: CommitOrChangeId::ChangeId(test_ctx.commits[1].change_id().unwrap()),
        description: None,
    };
    // an extra head just beneath the top of the stack
    test_ctx.branch.add_series(&ctx, extra_head, None)?;
    let initial_state = test_ctx.branch.clone();
    test_ctx
        .branch
        .set_legacy_compatible_stack_reference(&ctx)?;
    // no changes
    assert_eq!(initial_state, test_ctx.branch);
    Ok(())
}

#[test]
fn set_legacy_refname_pushed() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let remote_branch: RemoteRefname = "refs/remotes/origin/my-branch".parse()?;
    test_ctx.branch.upstream = Some(remote_branch.clone());

    let state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let mut target = state.get_default_target()?;
    target.push_remote_name = Some("origin".into());
    state.set_default_target(target)?;
    let push_details = test_ctx.branch.push_details(&ctx, "a-branch-2".into())?;
    ctx.push(
        push_details.head,
        &push_details.remote_refname,
        false,
        None,
        Some(Some(test_ctx.branch.id)),
    )?;
    let initial_state = test_ctx.branch.clone();

    test_ctx
        .branch
        .set_legacy_compatible_stack_reference(&ctx)?;
    // no changes
    assert_eq!(initial_state, test_ctx.branch);
    Ok(())
}

fn command_ctx(name: &str) -> Result<(CommandContext, TempDir)> {
    gitbutler_testsupport::writable::fixture("stacking.sh", name)
}

fn head_names(test_ctx: &TestContext) -> Vec<String> {
    test_ctx
        .branch
        .heads
        .iter()
        .map(|h| h.name.clone())
        .collect_vec()
}

fn test_ctx(ctx: &CommandContext) -> Result<TestContext> {
    let handle = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let branches = handle.list_all_branches()?;
    let branch = branches.iter().find(|b| b.name == "virtual").unwrap();
    let other_branch = branches.iter().find(|b| b.name != "virtual").unwrap();
    let target = handle.get_default_target()?;
    let mut branch_commits =
        ctx.repository()
            .log(branch.head(), LogUntil::Commit(target.sha), false)?;
    branch_commits.reverse();
    let mut other_commits =
        ctx.repository()
            .log(other_branch.head(), LogUntil::Commit(target.sha), false)?;
    other_commits.reverse();
    Ok(TestContext {
        branch: branch.clone(),
        commits: branch_commits,
        // other_branch: other_branch.clone(),
        other_commits,
        handle,
        default_target: target,
    })
}
struct TestContext<'a> {
    branch: gitbutler_stack::Stack,
    /// Oldest commit first
    commits: Vec<git2::Commit<'a>>,
    /// Oldest commit first
    #[allow(dead_code)]
    other_commits: Vec<git2::Commit<'a>>,
    handle: VirtualBranchesHandle,
    default_target: gitbutler_stack::Target,
}
