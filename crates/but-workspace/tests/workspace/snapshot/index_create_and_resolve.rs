use crate::snapshot::args_for_worktree_changes;
use crate::utils::{read_only_in_memory_scenario, visualize_index};
use but_testsupport::visualize_tree;
use but_workspace::snapshot;
use gix::prelude::ObjectIdExt;

#[test]
fn unborn_added_to_index() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("unborn-all-file-types-added-to-index")?;
    let (head_tree_id, state, no_workspace_and_meta) = args_for_worktree_changes(&repo)?;

    let out = snapshot::create_tree(head_tree_id, state, no_workspace_and_meta)?;
    insta::assert_snapshot!(visualize_tree(out.snapshot_tree.attach(&repo)), @r#"
    085f2bf
    ├── HEAD:4b825dc 
    ├── index:7f802e9 
    │   ├── link:120000:faf96c1 "untracked"
    │   ├── untracked:100644:d95f3ad "content\n"
    │   └── untracked-exe:100755:86daf54 "exe\n"
    └── worktree:7f802e9 
        ├── link:120000:faf96c1 "untracked"
        ├── untracked:100644:d95f3ad "content\n"
        └── untracked-exe:100755:86daf54 "exe\n"
    "#);
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        snapshot_tree: Sha1(085f2bfb08640d035adff078f19d75477fea1a86),
        head_tree: Sha1(4b825dc642cb6eb9a060e54bf8d69288fbee4904),
        worktree: Some(
            Sha1(7f802e9e3d48d97a7ca7bc4cbf7e0168bd587eed),
        ),
        index: Some(
            Sha1(7f802e9e3d48d97a7ca7bc4cbf7e0168bd587eed),
        ),
        index_conflicts: None,
        workspace_references: None,
        head_references: None,
        metadata: None,
    }
    ");

    let res_out = snapshot::resolve_tree(
        out.snapshot_tree.attach(&repo),
        out.head_tree,
        snapshot::resolve_tree::Options::default(),
    )?;
    let mut cherry_pick = res_out
        .worktree_cherry_pick
        .expect("a worktree change was applied");
    assert_eq!(cherry_pick.tree.write()?, out.worktree.unwrap());
    let index = res_out
        .index
        .expect("the index was altered with many added files");
    insta::assert_snapshot!(visualize_index(&index), @r"");

    assert!(res_out.metadata.is_none());
    assert!(
        res_out.workspace_references.is_none(),
        "didn't ask to store this"
    );
    Ok(())
}

#[test]
fn with_conflicts() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("merge-with-two-branches-conflict")?;
    let (head_tree_id, state, no_workspace_and_meta) = args_for_worktree_changes(&repo)?;

    let out = snapshot::create_tree(head_tree_id, state, no_workspace_and_meta)?;
    insta::assert_snapshot!(visualize_tree(out.snapshot_tree.attach(&repo)), @r#""#);
    insta::assert_debug_snapshot!(out, @r"");

    let res_out = snapshot::resolve_tree(
        out.snapshot_tree.attach(&repo),
        out.head_tree,
        snapshot::resolve_tree::Options::default(),
    )?;
    let mut cherry_pick = res_out
        .worktree_cherry_pick
        .expect("a worktree change was applied");
    assert_eq!(cherry_pick.tree.write()?, out.worktree.unwrap());
    let index = res_out
        .index
        .expect("the index was altered with many added files");
    insta::assert_snapshot!(visualize_index(&index), @r"");

    assert!(res_out.metadata.is_none());
    assert!(
        res_out.workspace_references.is_none(),
        "didn't ask to store this"
    );
    Ok(())
}
