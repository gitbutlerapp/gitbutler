use but_core::ref_metadata::{
    Branch, ProjectMeta, RefInfo, Review, Workspace,
    WorkspaceCommitRelation::{MergeFrom, Merged, Outside},
};
use but_db::DbHandle;
use gix::refs::FullName;

use crate::stack;

#[test]
fn only_obsolete_outside_stacks_are_collected() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("metadata-gc")?;
    let target = repo.head_id()?.detach();
    let mut db = but_testsupport::in_memory_db();
    let workspace_name: FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let mut workspace = Workspace {
        ref_info: RefInfo {
            created_at: Some(gix::date::Time::new(123, 3600)),
            updated_at: Some(gix::date::Time::new(456, -1800)),
        },
        stacks: vec![
            stack(1, &["merged-missing"], Merged)?,
            stack(2, &["muted-missing"], MergeFrom { commit_id: None })?,
            stack(
                3,
                &["partial-missing"],
                MergeFrom {
                    commit_id: Some(target),
                },
            )?,
            stack(4, &["target"], Outside)?,
            stack(5, &["before-target"], Outside)?,
            stack(6, &["ahead", "target"], Outside)?,
            stack(7, &["missing"], Outside)?,
            stack(8, &["broken"], Outside)?,
            stack(9, &["symbolic"], Outside)?,
        ],
    };
    workspace.stacks[5].branches[1].archived = true;
    let branch = Branch {
        ref_info: workspace.ref_info.clone(),
        review: Review {
            pull_request: Some(42),
            review_id: Some("review".into()),
        },
    };
    db.meta_mut()?
        .set_workspace(workspace_name.as_ref(), &workspace)?;
    for name in [
        "refs/heads/target",
        "refs/heads/ahead",
        "refs/heads/missing",
    ] {
        db.meta_mut()?.set_branch(name.try_into()?, &branch)?;
    }

    but_meta::garbage_collect(&repo, &project_meta(target), &mut db.connection_mut())?;

    workspace.stacks.retain(|stack| {
        [1, 2, 3, 6, 9]
            .map(but_core::ref_metadata::StackId::from_number_for_testing)
            .contains(&stack.id)
    });
    let metadata = db.meta()?;
    assert_eq!(
        metadata.workspace(workspace_name.as_ref()),
        Some(&workspace),
        "merged and muted stacks, symbolic heads, and stacks with unique tip commits retain their complete metadata"
    );
    for name in [
        "refs/heads/target",
        "refs/heads/ahead",
        "refs/heads/missing",
    ] {
        assert_eq!(
            metadata.branch(name.try_into()?),
            Some(&branch),
            "collecting workspace membership must not delete independent branch reviews or timestamps"
        );
    }
    Ok(())
}

#[test]
fn shared_stack_identity_does_not_couple_workspaces() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("metadata-gc")?;
    let target = repo.head_id()?.detach();
    let mut db = but_testsupport::in_memory_db();
    let first: FullName = "refs/heads/gitbutler/workspaces/first".try_into()?;
    let second: FullName = "refs/heads/gitbutler/workspaces/second".try_into()?;
    let mut first_value = Workspace {
        stacks: vec![stack(1, &["target"], Outside)?],
        ..Default::default()
    };
    let mut second_value = first_value.clone();
    second_value.stacks[0].workspacecommit_relation = Merged;
    db.meta_mut()?.set_workspace(first.as_ref(), &first_value)?;
    db.meta_mut()?
        .set_workspace(second.as_ref(), &second_value)?;
    let branch = Branch {
        review: Review {
            pull_request: Some(42),
            review_id: None,
        },
        ..Default::default()
    };
    db.meta_mut()?
        .set_branch("refs/heads/target".try_into()?, &branch)?;

    but_meta::garbage_collect(&repo, &project_meta(target), &mut db.connection_mut())?;

    first_value.stacks.clear();
    let metadata = db.meta()?;
    assert_eq!(
        metadata.workspace(first.as_ref()),
        Some(&first_value),
        "collecting the last outside stack retains the explicitly stored workspace"
    );
    assert_eq!(
        metadata.workspace(second.as_ref()),
        Some(&second_value),
        "the same stack ID and branch may remain merged in an independent workspace"
    );
    assert_eq!(
        metadata.branch("refs/heads/target".try_into()?),
        Some(&branch),
        "shared branch reviews remain available after collecting one workspace membership"
    );
    Ok(())
}

#[test]
fn absent_target_leaves_metadata_unchanged() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("metadata-gc")?;
    let mut db = but_testsupport::in_memory_db();
    db.meta_mut()?.set_workspace(
        "refs/heads/gitbutler/workspace".try_into()?,
        &Workspace {
            stacks: vec![stack(1, &["missing"], Outside)?],
            ..Default::default()
        },
    )?;
    let before = db.meta()?;

    but_meta::garbage_collect(&repo, &ProjectMeta::default(), &mut db.connection_mut())?;

    assert_eq!(
        db.meta()?,
        before,
        "without a project target even missing outside branches remain untouched"
    );
    Ok(())
}

#[test]
fn collection_uses_the_callers_transaction() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("metadata-gc")?;
    let target = repo.head_id()?.detach();
    let tmp = but_testsupport::gix_testtools::tempfile::tempdir()?;
    let mut db = DbHandle::new_in_directory(tmp.path())?;
    let observer = DbHandle::new_in_directory(tmp.path())?;
    let workspace_name: FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let workspace = Workspace {
        stacks: vec![stack(1, &["target"], Outside)?],
        ..Default::default()
    };
    db.meta_mut()?
        .set_workspace(workspace_name.as_ref(), &workspace)?;
    let before = db.meta()?;
    let mut pending_workspace = workspace;
    pending_workspace.ref_info.updated_at = Some(gix::date::Time::new(789, 0));
    pending_workspace
        .stacks
        .push(stack(2, &["ahead"], Outside)?);
    let mut expected_workspace = pending_workspace.clone();
    expected_workspace.stacks.remove(0);

    for commit in [false, true] {
        let mut tx = db.immediate_transaction()?;
        tx.meta_mut()?
            .set_workspace(workspace_name.as_ref(), &pending_workspace)?;
        but_meta::garbage_collect(&repo, &project_meta(target), &mut tx.connection_mut())?;
        let collected = tx.meta()?;
        assert_eq!(
            collected.workspace(workspace_name.as_ref()),
            Some(&expected_workspace),
            "collection sees earlier workspace edits made in the caller's transaction"
        );
        assert_eq!(
            observer.meta()?,
            before,
            "another connection cannot observe collection before commit"
        );
        if commit {
            tx.commit()?;
            assert_eq!(
                observer.meta()?,
                collected,
                "committing makes the collected workspace visible"
            );
        } else {
            tx.rollback()?;
            assert_eq!(
                db.meta()?,
                before,
                "rolling back restores all collected workspace memberships"
            );
        }
    }
    Ok(())
}

fn project_meta(target_commit_id: gix::ObjectId) -> ProjectMeta {
    ProjectMeta {
        target_commit_id: Some(target_commit_id),
        ..Default::default()
    }
}
