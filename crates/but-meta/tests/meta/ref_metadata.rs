use but_core::ref_metadata::{
    Workspace,
    WorkspaceCommitRelation::{Merged, Outside},
};
use but_db::DbHandle;
use gix::refs::FullName;

use crate::stack;

#[test]
fn duplicate_names_preserve_applied_and_stale_stack_grouping() -> anyhow::Result<()> {
    let tmp = but_testsupport::gix_testtools::tempfile::tempdir()?;
    let mut db = DbHandle::new_in_directory(tmp.path())?;
    let name: FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let mut workspace = Workspace {
        stacks: vec![
            stack(1, &["tip-a", "shared"], Merged)?,
            stack(2, &["shared"], Outside)?,
            stack(3, &["tip-b", "shared"], Merged)?,
        ],
        ..Default::default()
    };
    workspace.stacks[0].branches[1].archived = true;
    db.meta_mut()?.set_workspace(name.as_ref(), &workspace)?;
    let metadata = db.meta()?;
    db.meta_mut()?.set_workspace(
        name.as_ref(),
        metadata
            .workspace(name.as_ref())
            .expect("workspace was saved"),
    )?;
    drop(db);

    let db = DbHandle::new_in_directory(tmp.path())?;
    assert_eq!(
        db.meta()?.workspace(name.as_ref()),
        Some(&workspace),
        "roundtrips preserve each stack's own copy of shared segments and never move applied branches into stale outside stacks (#15573)"
    );
    assert!(
        !tmp.path().join("virtual-branches.toml").exists(),
        "metadata writes never create a TOML mirror"
    );
    Ok(())
}

#[test]
fn managed_workspace_order_is_available_to_ad_hoc_workspaces() -> anyhow::Result<()> {
    let mut db = but_testsupport::in_memory_db();
    let name: FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let workspace = Workspace {
        stacks: vec![
            stack(1, &["C", "A", "D"], Merged)?,
            stack(2, &["B"], Merged)?,
        ],
        ..Default::default()
    };
    db.meta_mut()?.set_workspace(name.as_ref(), &workspace)?;

    let metadata = db.meta()?;
    for (branch, stack) in [
        ("refs/heads/A", &workspace.stacks[0]),
        ("refs/heads/B", &workspace.stacks[1]),
    ] {
        let expected: Vec<_> = stack
            .branches
            .iter()
            .map(|branch| branch.ref_name.clone())
            .collect();
        assert_eq!(
            metadata.branch_stack_order(branch.try_into()?),
            Some(expected.as_slice()),
            "checking out a managed stack's middle branch reuses its tip-to-base order without joining independent stacks"
        );
    }
    Ok(())
}
