use anyhow::Result;
use but_core::{RefMetadata, ref_metadata::StackId};

#[test]
fn update_branch_name_renames_branch_order_metadata() -> Result<()> {
    let (mut ctx, _tmp) = crate::driverless::writable_context(
        "for-listing.sh",
        "one-vbranch-in-workspace-one-commit",
    )?;
    let virtual_ref = gix::refs::FullName::try_from("refs/heads/virtual")?;
    let workspace_ref = gix::refs::FullName::try_from("refs/heads/gitbutler/workspace")?;
    let renamed_ref = gix::refs::FullName::try_from("refs/heads/renamed-virtual")?;

    {
        let mut meta = ctx.meta()?;
        meta.set_branch_stack_order(&[virtual_ref.clone(), workspace_ref.clone()])?;
    }

    gitbutler_branch_actions::stack::update_branch_name(
        &mut ctx,
        StackId::from_number_for_testing(1),
        "virtual".into(),
        "renamed-virtual".into(),
    )?;

    assert_eq!(
        ctx.meta()?.branch_stack_order(renamed_ref.as_ref())?,
        Some(vec![renamed_ref.clone(), workspace_ref.clone()]),
        "branch-order metadata should follow the renamed branch ref"
    );
    assert!(
        ctx.meta()?
            .branch_stack_order(virtual_ref.as_ref())?
            .is_none(),
        "old branch ref should not retain order metadata after rename"
    );
    Ok(())
}

#[test]
fn failed_update_branch_name_leaves_branch_order_metadata_unchanged() -> Result<()> {
    let (mut ctx, _tmp) =
        crate::driverless::writable_context("for-listing.sh", "one-vbranch-in-workspace")?;
    let virtual_ref = gix::refs::FullName::try_from("refs/heads/virtual")?;
    let workspace_ref = gix::refs::FullName::try_from("refs/heads/gitbutler/workspace")?;

    {
        let mut meta = ctx.meta()?;
        meta.set_branch_stack_order(&[virtual_ref.clone(), workspace_ref.clone()])?;
    }

    assert!(
        gitbutler_branch_actions::stack::update_branch_name(
            &mut ctx,
            StackId::from_number_for_testing(1),
            "virtual".into(),
            "gitbutler/workspace".into(),
        )
        .is_err(),
        "renaming onto an existing local branch should fail before metadata is rewritten"
    );
    assert_eq!(
        ctx.meta()?.branch_stack_order(virtual_ref.as_ref())?,
        Some(vec![virtual_ref, workspace_ref]),
        "failed rename should leave branch-order metadata unchanged"
    );
    Ok(())
}
