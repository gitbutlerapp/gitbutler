use but_core::worktree::changes;
use but_core::UnifiedDiff;

#[test]
fn untracked_in_unborn() -> anyhow::Result<()> {
    let repo = crate::worktree::repo("untracked-unborn")?;
    UnifiedDiff::compute(
        &repo,
        "untracked".into(),
        repo.object_hash().null(),
        None,
        3,
    )?;
    let actual = changes(&repo)?;
    insta::assert_debug_snapshot!(actual, @r#"
    [
        WorktreeChange {
            path: "untracked",
            status: Untracked {
                state: ChangeState {
                    id: Sha1(0000000000000000000000000000000000000000),
                    kind: Blob,
                },
            },
        },
    ]
    "#);
    Ok(())
}
