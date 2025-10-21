use but_testsupport::visualize_tree;
use but_workspace::snapshot;
use gix::prelude::ObjectIdExt;

use crate::{
    snapshot::args_for_worktree_changes,
    utils::{read_only_in_memory_scenario, visualize_index},
};

#[test]
fn unborn_added_to_index() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("unborn-all-file-types-added-to-index")?;
    let (head_tree_id, mut state, no_workspace_and_meta) = args_for_worktree_changes(&repo)?;

    let out = snapshot::create_tree(head_tree_id, state.clone(), no_workspace_and_meta)?;
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
    insta::assert_snapshot!(visualize_index(&index), @r"
    120000:faf96c1 link
    100644:d95f3ad untracked
    100755:86daf54 untracked-exe
    ");

    assert!(res_out.metadata.is_none());
    assert!(
        res_out.workspace_references.is_none(),
        "didn't ask to store this"
    );

    state.selection.clear();
    let out = snapshot::create_tree(head_tree_id, state, no_workspace_and_meta)?;
    // An empty selection always means there is no effective change.
    insta::assert_snapshot!(visualize_tree(out.snapshot_tree.attach(&repo)), @"4b825dc");
    Ok(())
}

#[test]
fn with_conflicts() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("merge-with-two-branches-conflict")?;
    let (head_tree_id, mut state, no_workspace_and_meta) = args_for_worktree_changes(&repo)?;

    let out = snapshot::create_tree(head_tree_id, state.clone(), no_workspace_and_meta)?;
    insta::assert_snapshot!(visualize_tree(out.snapshot_tree.attach(&repo)), @r#"
    60bd065
    ├── HEAD:429a9b9 
    │   └── file:100644:e6c4914 "20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    ├── index-conflicts:189678e 
    │   └── file:0c1481f 
    │       ├── 1:100644:e69de29 ""
    │       ├── 2:100644:e6c4914 "20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    │       └── 3:100644:e33f5e9 "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"
    └── worktree:35f29cc 
        └── file:100644:1330395 "<<<<<<< HEAD\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n=======\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n>>>>>>> A\n"
    "#);
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        snapshot_tree: Sha1(60bd065acc7ffead5a0c96fe88272bb7c518df35),
        head_tree: Sha1(429a9b9078be82ae1e729a6af6f0649605431545),
        worktree: Some(
            Sha1(35f29cc82c94e40611d738100b21dedae6d72a74),
        ),
        index: None,
        index_conflicts: Some(
            Sha1(189678eb92add4601574a23c96a7c8d40bd9d154),
        ),
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
    insta::assert_snapshot!(visualize_index(&index), @r"
    100644:e69de29 file:1
    100644:e6c4914 file:2
    100644:e33f5e9 file:3
    ");

    assert!(res_out.metadata.is_none());
    assert!(
        res_out.workspace_references.is_none(),
        "didn't ask to store this"
    );

    state.selection.clear();
    let out = snapshot::create_tree(head_tree_id, state, no_workspace_and_meta)?;
    // An empty selection always means there is no effective change.
    insta::assert_snapshot!(visualize_tree(out.snapshot_tree.attach(&repo)), @"4b825dc");
    Ok(())
}

#[test]
fn index_added_modified_deleted() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("index-modified-added-deleted")?;
    let (head_tree_id, state, no_workspace_and_meta) = args_for_worktree_changes(&repo)?;

    let out = snapshot::create_tree(head_tree_id, state, no_workspace_and_meta)?;
    insta::assert_snapshot!(visualize_tree(out.snapshot_tree.attach(&repo)), @r#"
    449404d
    └── index:3fd7ead 
        ├── link:120000:e940347 "only-in-index"
        ├── modified-content:100644:70c2547 "index-content\n"
        └── modified-exe:100755:ef1943d "change-exe-bit\n"
    "#);
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        snapshot_tree: Sha1(449404df68c6041fc4f9b2f1cd30725bdb7ba329),
        head_tree: Sha1(632babec715de181e63036ee0cc3686efa67528d),
        worktree: None,
        index: Some(
            Sha1(3fd7ead2468a4def3db4c946c0f3b933eb8f2682),
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
    let index = res_out.index.expect("the index was altered");
    insta::assert_snapshot!(visualize_index(&index), @r"
    120000:e940347 link
    100644:70c2547 modified-content
    100755:ef1943d modified-exe
    ");

    assert!(res_out.metadata.is_none());
    assert!(
        res_out.workspace_references.is_none(),
        "didn't ask to store this"
    );
    Ok(())
}
