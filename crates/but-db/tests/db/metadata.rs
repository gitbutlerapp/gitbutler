use but_core::{
    WORKSPACE_REF_NAME,
    ref_metadata::{
        StackId, StackKind, Workspace, WorkspaceCommitRelation, WorkspaceStack,
        WorkspaceStackBranch,
    },
};
use but_db::DbHandle;
use gix::refs::FullName;

fn workspace(db: &DbHandle) -> anyhow::Result<Workspace> {
    let mut workspace = db
        .meta()?
        .workspace(WORKSPACE_REF_NAME.try_into()?)
        .cloned()
        .unwrap_or_default();
    workspace.stacks.push(WorkspaceStack {
        id: StackId::generate(),
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
        branches: vec![WorkspaceStackBranch {
            ref_name: "refs/heads/feature".try_into()?,
            archived: false,
        }],
    });
    Ok(workspace)
}

#[test]
fn standalone_metadata_mutation_reserves_writer_before_reading() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut db = DbHandle::new_in_directory(dir.path())?;
    let mut observer = DbHandle::new_in_directory(dir.path())?;
    let workspace = workspace(&db)?;

    for commit in [false, true] {
        let mutation = db.meta_mut()?;
        assert!(
            observer.immediate_transaction_nonblocking()?.is_none(),
            "another writer must not invalidate the metadata snapshot before its writes"
        );
        assert!(
            observer.meta()?.workspaces().next().is_none(),
            "readers remain able to inspect committed metadata"
        );
        if commit {
            mutation.set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)?;
        } else {
            drop(mutation);
        }
        assert!(
            observer.immediate_transaction_nonblocking()?.is_some(),
            "committing or dropping the metadata mutation releases the writer lock"
        );
        assert_eq!(
            dir.path().join("REFRESH").exists(),
            commit,
            "only a committed metadata mutation notifies observers"
        );
    }
    Ok(())
}

#[test]
fn metadata_is_isolated_and_notifies_only_after_commit() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut db = DbHandle::new_in_directory(dir.path())?;
    let observer = DbHandle::new_in_directory(dir.path())?;
    let empty = db.meta()?;
    let workspace = workspace(&db)?;
    let name: FullName = "refs/heads/feature".try_into()?;
    // This value predates the workspace write: saving it must resolve the current stack.
    let mut branch = empty.branch(name.as_ref()).cloned().unwrap_or_default();
    branch.review.pull_request = Some(42);
    let sentinel = dir.path().join("REFRESH");

    for commit in [false, true] {
        let mut tx = db.immediate_transaction()?;
        tx.meta_mut()?
            .set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)?;
        tx.meta_mut()?.set_branch(name.as_ref(), &branch)?;
        assert_eq!(
            tx.meta()?
                .branch(name.as_ref())
                .expect("branch metadata was saved")
                .review
                .pull_request,
            Some(42),
            "writes are visible inside the transaction"
        );
        assert!(
            observer.meta()?.branch(name.as_ref()).is_none(),
            "other connections cannot see pending metadata"
        );
        assert!(
            !sentinel.exists(),
            "pending metadata must not notify observers"
        );
        if commit {
            tx.commit()?;
        } else {
            tx.rollback()?;
        }
        assert_eq!(
            sentinel.exists(),
            commit,
            "only committed metadata emits REFRESH"
        );
    }

    assert_eq!(
        observer
            .meta()?
            .branch(name.as_ref())
            .expect("branch metadata was saved")
            .review
            .pull_request,
        Some(42),
        "committed writes become visible through existing connections"
    );
    assert!(
        empty.branch(name.as_ref()).is_none(),
        "metadata snapshots do not change after reads"
    );
    assert!(
        !dir.path().join("virtual_branches.toml").exists(),
        "metadata never creates the former storage file"
    );
    std::fs::remove_file(&sentinel)?;
    {
        let mut tx = db.immediate_transaction()?;
        tx.meta_mut()?.remove(name.as_ref())?;
    }
    assert!(
        !sentinel.exists(),
        "dropping a transaction rolls back without notifying"
    );
    assert!(
        db.meta()?.branch(name.as_ref()).is_some(),
        "drop preserved committed metadata"
    );
    Ok(())
}

#[test]
fn workspace_failure_rolls_back_branch_order_savepoints() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut db = DbHandle::new_in_directory(dir.path())?;
    let mut workspace = workspace(&db)?;
    db.meta_mut()?
        .set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)?;
    let before = db.virtual_branches().get_snapshot()?;
    let order_before = db.branch_order().get_snapshot()?;
    std::fs::remove_file(dir.path().join("REFRESH"))?;
    workspace.stacks[0].branches.insert(
        0,
        WorkspaceStackBranch {
            ref_name: "refs/heads/new-tip".try_into()?,
            archived: false,
        },
    );
    // The first stack's order is already written when the second stack is rejected.
    workspace.stacks.push(WorkspaceStack {
        id: StackId::generate(),
        branches: Vec::new(),
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
    });
    assert!(
        db.meta_mut()?
            .set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)
            .is_err(),
        "an empty incoming stack is invalid"
    );
    assert_eq!(
        db.virtual_branches().get_snapshot()?,
        before,
        "failed workspace updates preserve all VB rows"
    );
    assert_eq!(
        db.branch_order().get_snapshot()?,
        order_before,
        "nested order savepoints roll back with the metadata mutation"
    );
    assert!(
        !dir.path().join("REFRESH").exists(),
        "failed writes must not notify observers"
    );
    Ok(())
}

#[test]
fn metadata_updates_preserve_raw_fields_and_rename_atomically() -> anyhow::Result<()> {
    let mut db = DbHandle::new_at_path(":memory:")?;
    let workspace = workspace(&db)?;
    db.meta_mut()?
        .set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)?;
    let mut snapshot = db
        .virtual_branches()
        .get_snapshot()?
        .expect("workspace was saved");
    snapshot.state.last_pushed_base_sha = Some("1111111111111111111111111111111111111111".into());
    snapshot.stacks[0].legacy_notes = "keep notes".into();
    snapshot.stacks[0].legacy_head_sha = "2222222222222222222222222222222222222222".into();
    snapshot.stacks[0].source_refname = Some("refs/heads/source".into());
    snapshot.stacks[0].upstream_remote_name = Some("origin".into());
    snapshot.stacks[0].upstream_branch_name = Some("feature".into());
    snapshot.heads[0].head_sha = "3333333333333333333333333333333333333333".into();
    db.meta_mut()?.replace_snapshot(&snapshot)?;

    let old: FullName = "refs/heads/feature".try_into()?;
    let renamed: FullName = "refs/heads/renamed".try_into()?;
    let mut branch = db.meta()?.branch(old.as_ref()).cloned().unwrap_or_default();
    branch.review.pull_request = Some(17);
    db.meta_mut()?.set_branch(old.as_ref(), &branch)?;
    snapshot.heads[0].pr_number = Some(17);
    assert_eq!(
        db.virtual_branches().get_snapshot()?,
        Some(snapshot.clone()),
        "branch writes only change the requested review fields"
    );
    db.meta_mut()?
        .set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)?;
    assert_eq!(
        db.virtual_branches().get_snapshot()?,
        Some(snapshot.clone()),
        "workspace writes retain opaque legacy fields and head ids"
    );

    db.meta_mut()?
        .set_branch_stack_order(std::slice::from_ref(&renamed))?;
    let order = db.branch_order().get_snapshot()?;
    assert!(
        db.meta_mut()?
            .rename(old.as_ref(), renamed.as_ref())
            .is_err(),
        "a destination in branch order also prevents rename"
    );
    assert_eq!(
        db.virtual_branches().get_snapshot()?,
        Some(snapshot.clone()),
        "rename rejection cannot partially rename managed data"
    );
    assert_eq!(
        db.branch_order().get_snapshot()?,
        order,
        "rename rejection preserves ordering"
    );
    db.meta_mut()?.remove(renamed.as_ref())?;
    db.meta_mut()?.rename(old.as_ref(), renamed.as_ref())?;
    snapshot.heads[0].name = "renamed".into();
    assert_eq!(
        db.virtual_branches().get_snapshot()?,
        Some(snapshot),
        "rename preserves stack identity and all unrelated fields"
    );
    assert_eq!(
        db.meta()?
            .branch_stack_order(renamed.as_ref())
            .map(<[_]>::to_vec),
        Some(vec![renamed]),
        "rename moves branch ordering with branch metadata"
    );
    Ok(())
}

#[test]
fn worktree_adoption_composes_with_a_transaction() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let repo = gix::init(dir.path())?;
    let mut db = DbHandle::new_at_path(":memory:")?;
    for commit in [false, true] {
        let mut tx = db.immediate_transaction()?;
        let entries = but_db::worktrees::worktrees_with_state(&repo, &mut tx.connection_mut())?;
        assert!(
            entries.is_empty(),
            "the fixture contains no linked worktrees"
        );
        assert!(
            tx.worktree_meta().adoption_ran()?,
            "adoption is visible inside the transaction"
        );
        if commit {
            tx.commit()?;
        } else {
            tx.rollback()?;
        }
        assert_eq!(
            db.worktree_meta().adoption_ran()?,
            commit,
            "adoption follows the enclosing transaction's outcome"
        );
    }
    Ok(())
}

#[test]
fn workspace_reuses_stack_identity_without_reordering_it_under_a_new_id() -> anyhow::Result<()> {
    let mut db = DbHandle::new_at_path(":memory:")?;
    let mut workspace = db
        .meta()?
        .workspace(WORKSPACE_REF_NAME.try_into()?)
        .cloned()
        .unwrap_or_default();
    for (index, name) in ["A", "B", "C"].into_iter().enumerate() {
        workspace.stacks.push(WorkspaceStack {
            id: StackId::from_number_for_testing(index as u128),
            workspacecommit_relation: WorkspaceCommitRelation::Merged,
            branches: vec![WorkspaceStackBranch {
                ref_name: format!("refs/heads/{name}").try_into()?,
                archived: false,
            }],
        });
    }
    db.meta_mut()?
        .set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)?;

    // A graph operation can give an existing branch a fresh projected stack identity.
    // Reuse its stored identity and order; the remaining C stack takes the vacated position.
    let mut torn = workspace.stacks.remove(1);
    let original_id = torn.id;
    torn.id = StackId::from_number_for_testing(3);
    workspace.stacks.push(torn);
    db.meta_mut()?
        .set_workspace(WORKSPACE_REF_NAME.try_into()?, &workspace)?;

    let metadata = db.meta()?;
    let workspace = metadata
        .workspace(WORKSPACE_REF_NAME.try_into()?)
        .expect("workspace metadata was saved");
    assert_eq!(
        workspace
            .find_stack_with_branch("refs/heads/B".try_into()?, StackKind::AppliedAndUnapplied)
            .map(|stack| stack.id),
        Some(original_id),
        "the existing branch retains its stored stack identity"
    );
    assert_eq!(
        workspace
            .stacks
            .iter()
            .map(|stack| stack.branches[0].ref_name.shorten().to_string())
            .collect::<Vec<_>>(),
        ["A", "B", "C"],
        "the reused B stack keeps its order and sorts before C at the same position"
    );
    Ok(())
}
