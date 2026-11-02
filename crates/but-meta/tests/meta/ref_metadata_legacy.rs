use but_db::legacy::{Stack as LegacyStack, StackBranch, VirtualBranches};

#[test]
fn legacy_snapshot_roundtrip_preserves_fields() -> anyhow::Result<()> {
    for name in [
        "virtual-branches-01",
        "non-unique-branches",
        "legacy-change-id",
    ] {
        let source: VirtualBranches = toml::from_str(&std::fs::read_to_string(format!(
            "tests/fixtures/legacy/{name}.toml"
        ))?)?;
        let snapshot = but_db::legacy::legacy_to_snapshot(&source)?;
        assert_eq!(
            but_db::legacy::snapshot_to_legacy(&snapshot)?,
            source,
            "archived snapshots preserve all legacy fields through the database representation"
        );
    }
    Ok(())
}

#[test]
fn legacy_change_id_deserializes_as_null_sha() -> anyhow::Result<()> {
    let source: VirtualBranches =
        toml::from_str(include_str!("../fixtures/legacy/legacy-change-id.toml"))?;
    let stack = source
        .branches
        .values()
        .next()
        .expect("fixture contains one stack");
    assert_eq!(
        stack.heads.len(),
        1,
        "fixture contains one legacy change ID"
    );
    assert!(
        stack.heads[0].head.is_null(),
        "legacy change IDs have no object ID"
    );
    Ok(())
}

#[test]
fn garbage_collect_removes_outside_workspace_stack_at_target() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("dlib-standin")?;
    let target = repo.head_id()?.detach();
    let mut state = but_db::legacy::VirtualBranches::default();

    let workspace_stack = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head: target,
            name: "kept".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        0,
        true,
    );
    let outside_stack = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head: target,
            name: "collected".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        1,
        false,
    );
    let workspace_stack_id = workspace_stack.id;
    let outside_stack_id = outside_stack.id;
    state.branches.insert(workspace_stack_id, workspace_stack);
    state.branches.insert(outside_stack_id, outside_stack);

    let mut db = but_db::DbHandle::new_at_path(":memory:")?;
    db.virtual_branches_mut()?
        .replace_snapshot(&but_db::legacy::legacy_to_snapshot(&state)?)?;
    but_meta::garbage_collect(&repo, &project_meta(target), &mut db)?;
    let stored = but_db::legacy::snapshot_to_legacy(
        &db.virtual_branches().get_snapshot()?.unwrap_or_default(),
    )?;

    assert!(
        stored.branches.contains_key(&workspace_stack_id),
        "applied stacks survive garbage collection"
    );
    assert!(
        !stored.branches.contains_key(&outside_stack_id),
        "obsolete unapplied stacks are removed"
    );
    Ok(())
}

#[test]
fn garbage_collect_removes_outside_workspace_stack_with_missing_head() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("dlib-standin")?;
    let target = repo.head_id()?.detach();
    let missing_head = gix::ObjectId::from_hex(b"30696678319e0fa3a20e54f22d47fc8cf1ceaade")?;
    let mut state = but_db::legacy::VirtualBranches::default();
    let outside_stack = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head: missing_head,
            name: "missing".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        0,
        false,
    );
    let outside_stack_id = outside_stack.id;
    state.branches.insert(outside_stack_id, outside_stack);

    let mut db = but_db::DbHandle::new_at_path(":memory:")?;
    db.virtual_branches_mut()?
        .replace_snapshot(&but_db::legacy::legacy_to_snapshot(&state)?)?;
    but_meta::garbage_collect(&repo, &project_meta(target), &mut db)?;
    let stored = but_db::legacy::snapshot_to_legacy(
        &db.virtual_branches().get_snapshot()?.unwrap_or_default(),
    )?;

    assert!(
        !stored.branches.contains_key(&outside_stack_id),
        "obsolete unapplied stacks are removed"
    );
    Ok(())
}

#[test]
fn garbage_collect_removes_outside_workspace_stack_with_broken_ref() -> anyhow::Result<()> {
    let (mut repo, _tmp) = but_testsupport::writable_scenario("dlib-standin");
    let target = repo.head_id()?.detach();
    let missing_head = gix::ObjectId::from_hex(b"30696678319e0fa3a20e54f22d47fc8cf1ceaade")?;
    repo.reference(
        "refs/heads/missing",
        missing_head,
        gix::refs::transaction::PreviousValue::Any,
        "test",
    )?;
    repo.reload()?;
    let mut state = but_db::legacy::VirtualBranches::default();
    let outside_stack = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head: target,
            name: "missing".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        0,
        false,
    );
    let outside_stack_id = outside_stack.id;
    state.branches.insert(outside_stack_id, outside_stack);

    let mut db = but_db::DbHandle::new_at_path(":memory:")?;
    db.virtual_branches_mut()?
        .replace_snapshot(&but_db::legacy::legacy_to_snapshot(&state)?)?;
    but_meta::garbage_collect(&repo, &project_meta(target), &mut db)?;
    let stored = but_db::legacy::snapshot_to_legacy(
        &db.virtual_branches().get_snapshot()?.unwrap_or_default(),
    )?;

    assert!(
        !stored.branches.contains_key(&outside_stack_id),
        "obsolete unapplied stacks are removed"
    );
    Ok(())
}

fn project_meta(target_commit_id: gix::ObjectId) -> but_core::ref_metadata::ProjectMeta {
    but_core::ref_metadata::ProjectMeta {
        target_commit_id: Some(target_commit_id),
        ..Default::default()
    }
}
