use but_core::{
    WORKSPACE_REF_NAME,
    ref_metadata::{
        RefInfo, StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
    },
};

fn r(name: &str) -> &gix::refs::FullNameRef {
    name.try_into().expect("valid test reference")
}

#[test]
fn empty_workspace_and_ref_times_are_persisted() -> anyhow::Result<()> {
    let mut db = but_db::DbHandle::new_at_path(":memory:")?;
    let mut workspace = db.meta().workspace(r(WORKSPACE_REF_NAME))?;
    assert!(
        db.meta().workspace_opt(r(WORKSPACE_REF_NAME))?.is_none(),
        "an empty database has no workspace"
    );
    workspace.ref_info = RefInfo {
        created_at: Some(gix::date::Time::new(42, 3600)),
        updated_at: Some(gix::date::Time::new(56, -3600)),
    };
    db.meta_mut()?.set_workspace(&workspace)?;
    let stored = db
        .meta()
        .workspace_opt(r(WORKSPACE_REF_NAME))?
        .expect("explicit empty workspace exists");
    assert_eq!(
        *stored, *workspace,
        "workspace timestamps and empty membership survive storage"
    );

    let mut branch = db.meta().branch(r("refs/heads/topic"))?;
    branch.ref_info = workspace.ref_info.clone();
    db.meta_mut()?.set_branch(&branch)?;
    assert_eq!(
        *db.meta().branch(branch.as_ref())?,
        *branch,
        "branch timestamps survive storage"
    );
    assert!(
        db.meta_mut()?.remove(r(WORKSPACE_REF_NAME))?,
        "an explicit workspace can be removed"
    );
    assert!(
        db.meta().workspace_opt(r(WORKSPACE_REF_NAME))?.is_none(),
        "removed workspace stays absent"
    );
    Ok(())
}

#[test]
fn moving_and_renaming_a_branch_preserves_its_metadata() -> anyhow::Result<()> {
    let mut db = but_db::DbHandle::new_at_path(":memory:")?;
    let source = r("refs/heads/topic");
    let destination = r("refs/heads/renamed");
    let mut branch = db.meta().branch(source)?;
    branch.review.pull_request = Some(42);
    branch.review.review_id = Some("review-id".into());
    branch.ref_info.updated_at = Some(gix::date::Time::new(56, 0));
    db.meta_mut()?.set_branch(&branch)?;
    let mut workspace = db.meta().workspace(r(WORKSPACE_REF_NAME))?;
    workspace.stacks = vec![WorkspaceStack {
        id: StackId::from_number_for_testing(1),
        branches: vec![WorkspaceStackBranch {
            ref_name: source.to_owned(),
            archived: false,
        }],
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
    }];
    db.meta_mut()?.set_workspace(&workspace)?;
    assert_eq!(
        *db.meta().branch(source)?,
        *branch,
        "moving branches between stacks keeps metadata"
    );
    db.meta_mut()?.rename(source, destination)?;
    assert!(
        db.meta().branch_opt(source)?.is_none(),
        "rename removes the old metadata key"
    );
    assert_eq!(
        *db.meta().branch(destination)?,
        *branch,
        "rename preserves branch data"
    );
    assert!(
        db.meta().workspace(r(WORKSPACE_REF_NAME))?.stacks[0].branches[0].ref_name == destination,
        "workspace membership follows the renamed ref"
    );
    Ok(())
}

#[test]
fn rename_collision_leaves_metadata_and_order_unchanged() -> anyhow::Result<()> {
    let mut db = but_db::DbHandle::new_at_path(":memory:")?;
    let source = r("refs/heads/topic");
    let destination = r("refs/heads/existing");
    for name in [source, destination] {
        let branch = db.meta().branch(name)?;
        db.meta_mut()?.set_branch(&branch)?;
    }
    db.meta_mut()?
        .set_branch_stack_order(&[source.to_owned(), destination.to_owned()])?;
    let before = db.virtual_branches().get_snapshot()?;
    let order = db.meta().branch_stack_order(source)?;
    assert!(
        db.meta_mut()?.rename(source, destination).is_err(),
        "rename must reject an existing metadata key"
    );
    assert_eq!(
        db.virtual_branches().get_snapshot()?,
        before,
        "failed rename preserves branch data"
    );
    assert_eq!(
        db.meta().branch_stack_order(source)?,
        order,
        "failed rename preserves stack order"
    );

    let mut snapshot = before.expect("both branches were persisted");
    snapshot.heads.retain(|head| head.name != "existing");
    snapshot
        .stacks
        .retain(|stack| snapshot.heads.iter().any(|head| head.stack_id == stack.id));
    db.virtual_branches_mut()?.replace_snapshot(&snapshot)?;
    db.branch_order_mut()?
        .remove_reference("refs/heads/existing")?;
    db.meta_mut()?.rename(source, destination)?;
    assert!(
        db.meta().branch_opt(destination)?.is_some(),
        "reference timestamps left by a legacy removal do not reserve its name"
    );
    Ok(())
}

#[test]
fn remote_ref_names_are_preserved_and_rollback_does_not_write_a_mirror() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let mut db = but_db::DbHandle::new_in_directory(tmp.path())?;
    let remote = r("refs/remotes/origin/topic");
    {
        let mut tx = db.transaction()?;
        let mut branch = tx.meta().branch(remote)?;
        branch.review.pull_request = Some(42);
        tx.meta_mut()?.set_branch(&branch)?;
        assert_eq!(
            tx.meta().branch(remote)?.review.pull_request,
            Some(42),
            "remote refs retain their namespace"
        );
        assert!(
            !tmp.path().join("virtual_branches.toml").exists(),
            "uncommitted metadata must not be mirrored"
        );
        tx.rollback()?;
    }
    assert!(
        db.meta().branch_opt(remote)?.is_none(),
        "outer rollback discards nested metadata writes"
    );
    assert!(
        !tmp.path().join("virtual_branches.toml").exists(),
        "rollback must not create a TOML mirror"
    );
    Ok(())
}

#[test]
fn duplicate_names_keep_each_stacks_own_head_data() -> anyhow::Result<()> {
    for relation in [
        WorkspaceCommitRelation::Merged,
        WorkspaceCommitRelation::Outside,
    ] {
        let mut db = but_db::DbHandle::new_at_path(":memory:")?;
        let mut workspace = db.meta().workspace(r(WORKSPACE_REF_NAME))?;
        workspace.stacks = [
            (1, "refs/heads/first", WorkspaceCommitRelation::Merged),
            (2, "refs/heads/second", relation),
        ]
        .into_iter()
        .map(|(id, tip, workspacecommit_relation)| WorkspaceStack {
            id: StackId::from_number_for_testing(id),
            branches: [tip, "refs/heads/shared"]
                .into_iter()
                .map(|name| WorkspaceStackBranch {
                    ref_name: r(name).to_owned(),
                    archived: false,
                })
                .collect(),
            workspacecommit_relation,
        })
        .collect();
        db.meta_mut()?.set_workspace(&workspace)?;
        let mut before = db
            .virtual_branches()
            .get_snapshot()?
            .expect("persisted workspace");
        for (index, head) in before.heads.iter_mut().enumerate() {
            head.head_sha = format!("{:040x}", index + 1);
            head.pr_number = Some(index as i64 + 10);
        }
        before.heads[0].name = "refs/heads/shared".into();
        before.stacks[0].sort_order = 1;
        before.stacks[1].sort_order = 0;
        before.stacks.reverse();
        db.virtual_branches_mut()?.replace_snapshot(&before)?;
        assert_eq!(
            db.meta()
                .branch(r("refs/heads/shared"))?
                .review
                .pull_request,
            Some(12),
            "lookup follows stack order instead of stack ID order"
        );
        let mut branch = db.meta().branch(r("refs/heads/shared"))?;
        branch.review.pull_request = Some(42);
        db.meta_mut()?.set_branch(&branch)?;
        before.heads[2].pr_number = Some(42);
        assert_eq!(
            db.meta()
                .branch(r("refs/heads/shared"))?
                .review
                .pull_request,
            Some(42),
            "updating metadata changes the same stack that was read"
        );
        let workspace = db.meta().workspace(r(WORKSPACE_REF_NAME))?;
        db.meta_mut()?.set_workspace(&workspace)?;
        assert_eq!(
            db.virtual_branches().get_snapshot()?,
            Some(before),
            "shared applied segments and stale unapplied copies preserve their own hashes and reviews"
        );
    }
    Ok(())
}

#[test]
fn ref_info_snapshots_restore_all_times_and_workspace_identity() -> anyhow::Result<()> {
    let mut db = but_db::DbHandle::new_at_path(":memory:")?;
    let mut workspace = db.meta().workspace(r(WORKSPACE_REF_NAME))?;
    workspace.ref_info.updated_at = Some(gix::date::Time::new(56, 3600));
    db.meta_mut()?.set_workspace(&workspace)?;
    let snapshot = db.meta().ref_info_snapshot()?;
    snapshot.validate()?;
    db.meta_mut()?.remove(r(WORKSPACE_REF_NAME))?;
    assert_ne!(
        db.meta().ref_info_snapshot()?,
        snapshot,
        "removal changes reference metadata"
    );
    db.meta_mut()?.restore_ref_info(&snapshot)?;
    assert_eq!(
        db.meta().ref_info_snapshot()?,
        snapshot,
        "snapshot restores every reference field"
    );
    assert!(
        db.meta().workspace_opt(r(WORKSPACE_REF_NAME))?.is_some(),
        "restored empty workspace exists"
    );
    Ok(())
}

#[test]
fn a_stack_cannot_retain_the_same_branch_twice() -> anyhow::Result<()> {
    let mut db = but_db::DbHandle::new_at_path(":memory:")?;
    let mut workspace = db.meta().workspace(r(WORKSPACE_REF_NAME))?;
    let branch = WorkspaceStackBranch {
        ref_name: r("refs/heads/topic").to_owned(),
        archived: false,
    };
    workspace.stacks.push(WorkspaceStack {
        id: StackId::from_number_for_testing(1),
        branches: vec![branch.clone(), branch],
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
    });
    db.meta_mut()?.set_workspace(&workspace)?;
    assert_eq!(
        db.meta().workspace(r(WORKSPACE_REF_NAME))?.stacks[0]
            .branches
            .len(),
        1,
        "stale duplicates within a stack are removed before persisting its order"
    );
    Ok(())
}

#[test]
fn read_only_metadata_accepts_databases_before_ref_info() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let mut db = but_db::DbHandle::new_in_directory(tmp.path())?;
    let name = r("refs/heads/topic");
    let mut branch = db.meta().branch(name)?;
    branch.review.pull_request = Some(42);
    db.meta_mut()?.set_branch(&branch)?;
    drop(db);

    // Observers open existing databases without applying new migrations.
    let path = but_db::DbHandle::db_file_path(tmp.path());
    rusqlite::Connection::open(&path)?.execute("DROP TABLE ref_info", [])?;
    let db = but_db::DbHandle::open_existing_read_only_at_path(path)?;

    let branch = db.meta().branch(name)?;
    assert_eq!(
        branch.review.pull_request,
        Some(42),
        "existing branch metadata remains readable before the timestamp migration"
    );
    assert_eq!(
        branch.ref_info,
        RefInfo::default(),
        "older databases have no persisted branch timestamps"
    );
    let workspace = db.meta().workspace(r(WORKSPACE_REF_NAME))?;
    assert_eq!(
        workspace.stacks[0].branches[0].ref_name.as_ref(),
        name,
        "workspace projection still sees the stored branches"
    );
    assert_eq!(
        db.meta().iter().collect::<anyhow::Result<Vec<_>>>()?.len(),
        2,
        "iteration returns both the branch and its workspace"
    );
    Ok(())
}
