use crate::snapshot::args_for_worktree_changes;
use crate::utils::read_only_in_memory_scenario;
use but_testsupport::visualize_tree;
use but_workspace::snapshot;
use gix::prelude::ObjectIdExt;

#[test]
fn unborn_empty() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("unborn-empty")?;
    let (head_tree_id, state, no_workspace_and_meta) = args_for_worktree_changes(&repo)?;

    let out = snapshot::create_tree(head_tree_id, state, no_workspace_and_meta)?;
    assert!(
        out.is_empty(),
        "There is nothing to pick up and no change at all."
    );
    insta::assert_snapshot!(visualize_tree(out.snapshot_tree.attach(&repo)), @"4b825dc");
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        snapshot_tree: Sha1(4b825dc642cb6eb9a060e54bf8d69288fbee4904),
        head_tree: Sha1(4b825dc642cb6eb9a060e54bf8d69288fbee4904),
        worktree: None,
        index: None,
        index_conflicts: None,
        workspace_references: None,
        head_references: None,
        metadata: None,
    }
    ");
    let out = snapshot::resolve_tree(
        out.snapshot_tree.attach(&repo),
        out.head_tree,
        snapshot::resolve_tree::Options::default(),
    )?;
    assert!(out.index.is_none());
    assert!(out.metadata.is_none());
    assert!(
        out.worktree_cherry_pick.is_none(),
        "no worktree to cherry-pick"
    );
    assert!(
        out.workspace_references.is_none(),
        "didn't ask to store this"
    );

    Ok(())
}

#[test]
fn unborn_untracked() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("unborn-untracked-all-file-types")?;
    let (head_tree_id, mut state, no_workspace_and_meta) = args_for_worktree_changes(&repo)?;

    let out = snapshot::create_tree(head_tree_id, state.clone(), no_workspace_and_meta)?;
    assert!(!out.is_empty(), "it picks up the untracked files");
    insta::assert_snapshot!(visualize_tree(out.snapshot_tree.attach(&repo)), @r#"
    a863d4e
    ├── HEAD:4b825dc 
    └── worktree:7f802e9 
        ├── link:120000:faf96c1 "untracked"
        ├── untracked:100644:d95f3ad "content\n"
        └── untracked-exe:100755:86daf54 "exe\n"
    "#);
    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        snapshot_tree: Sha1(a863d4e32304f2f8e5d80b18a4f6fd614c052590),
        head_tree: Sha1(4b825dc642cb6eb9a060e54bf8d69288fbee4904),
        worktree: Some(
            Sha1(7f802e9e3d48d97a7ca7bc4cbf7e0168bd587eed),
        ),
        index: None,
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
    assert_eq!(
        cherry_pick.tree.write()?,
        out.worktree.unwrap(),
        "Applying worktree changes to their base yields the worktree changes exactly.\
        Due to the way this works, we don't have to test much as we rely on gix merge to work."
    );
    assert!(res_out.index.is_none());
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
fn worktree_all_filetypes() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("all-file-types-renamed-and-modified")?;
    let (head_tree_id, mut state, no_workspace_and_meta) = args_for_worktree_changes(&repo)?;

    let out = snapshot::create_tree(head_tree_id, state.clone(), no_workspace_and_meta)?;
    insta::assert_snapshot!(visualize_tree(out.snapshot_tree.attach(&repo)), @r#"
    9d274f3
    ├── HEAD:3fd29f0 
    │   ├── executable:100755:01e79c3 "1\n2\n3\n"
    │   ├── file:100644:3aac70f "5\n6\n7\n8\n"
    │   └── link:120000:c4c364c "nonexisting-target"
    └── worktree:e56fc9b 
        ├── executable-renamed:100755:8a1218a "1\n2\n3\n4\n5\n"
        ├── file-renamed:100644:c5c4315 "5\n6\n7\n8\n9\n10\n"
        └── link-renamed:120000:94e4e07 "other-nonexisting-target"
    "#);

    insta::assert_debug_snapshot!(out, @r"
    Outcome {
        snapshot_tree: Sha1(9d274f3ad046ca7d50285c6c5056bfe89f16587c),
        head_tree: Sha1(3fd29f0ca55ee4dc3ea6bf02a761c15fd6dc8428),
        worktree: Some(
            Sha1(e56fc9bacdd11ebe576b5d96d21127c423698126),
        ),
        index: None,
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
    assert_eq!(
        cherry_pick.tree.write()?,
        out.worktree.unwrap(),
        "Applying worktree changes to their base yields the worktree changes exactly.\
        Due to the way this works, we don't have to test much as we rely on gix merge to work."
    );
    assert!(res_out.index.is_none());
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
