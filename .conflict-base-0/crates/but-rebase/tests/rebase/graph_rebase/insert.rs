//! These tests exercise the insert operation.
use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, Step, mutate::InsertSide};
use but_testsupport::{git_status, graph_tree, visualize_commit_graph_all};

use crate::utils::{fixture_writable, standard_options};

/// Inserting below a merge commit should inherit all of it's parents
#[test]
fn insert_below_merge_commit() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("merge-in-the-middle")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * e8ee978 (HEAD -> with-inner-merge) on top of inner merge
    *   2fc288c Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let merge_id = repo.rev_parse_single("HEAD~")?;

    // Create a commit that we can stick below the merge commit
    let mut merge_obj = but_core::Commit::from_id(merge_id)?;
    merge_obj.message = "Commit below the merge commit".into();
    merge_obj.parents = vec![].into();
    let new_commit = repo.write_object(merge_obj.inner)?.detach();

    // select the merge commit
    let selector = editor
        .select_commit(merge_id.detach())
        .context("Failed to find commit a in editor graph")?;
    // replace it with the new one
    editor.insert(selector, Step::new_pick(new_commit), InsertSide::Below)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:with-inner-merge[🌳]
        ├── ·f699c45 (⌂|1)
        └── ·16b7c68 (⌂|1)
            └── ►:1[1]:anon:
                └── ·8ca0053 (⌂|1)
                    ├── ►:2[2]:A
                    │   └── ·add59d2 (⌂|1)
                    │       └── ►:4[3]:main
                    │           └── 🏁·8f0d338 (⌂|1) ►tags/base
                    └── ►:3[2]:B
                        └── ·984fd1c (⌂|1)
                            └── →:4: (main)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f699c45 (HEAD -> with-inner-merge) on top of inner merge
    * 16b7c68 Merge branch 'B' into with-inner-merge
    *   8ca0053 Commit below the merge commit
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"
    {
        Sha1(231acb683a6ecfb1ff546952057c4b3d3764b28c): Sha1(8ca0053aa15fe12ab6b467a82bedf86401628c17),
        Sha1(2fc288c36c8bb710c78203f78ea9883724ce142b): Sha1(16b7c68c1dae39aa8b5e4c56e3bc4b1d508cfb25),
        Sha1(e8ee978dac10e6a85006543ef08be07c5824b4f7): Sha1(f699c45a85f07b90f2d9531269d79b2ffbf3795e),
    }
    ");

    Ok(())
}

/// Inserting below a merge commit should inherit all of it's parents
#[test]
fn insert_below_merge_commit_excluded_mappings() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("merge-in-the-middle")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * e8ee978 (HEAD -> with-inner-merge) on top of inner merge
    *   2fc288c Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let merge_id = repo.rev_parse_single("HEAD~")?;

    // Create a commit that we can stick below the merge commit
    let mut merge_obj = but_core::Commit::from_id(merge_id)?;
    merge_obj.message = "Commit below the merge commit".into();
    merge_obj.parents = vec![].into();
    let new_commit = repo.write_object(merge_obj.inner)?.detach();

    // select the merge commit
    let selector = editor
        .select_commit(merge_id.detach())
        .context("Failed to find commit a in editor graph")?;
    // replace it with the new one
    editor.insert(
        selector,
        Step::new_untracked_pick(new_commit),
        InsertSide::Below,
    )?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:with-inner-merge[🌳]
        ├── ·f699c45 (⌂|1)
        └── ·16b7c68 (⌂|1)
            └── ►:1[1]:anon:
                └── ·8ca0053 (⌂|1)
                    ├── ►:2[2]:A
                    │   └── ·add59d2 (⌂|1)
                    │       └── ►:4[3]:main
                    │           └── 🏁·8f0d338 (⌂|1) ►tags/base
                    └── ►:3[2]:B
                        └── ·984fd1c (⌂|1)
                            └── →:4: (main)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f699c45 (HEAD -> with-inner-merge) on top of inner merge
    * 16b7c68 Merge branch 'B' into with-inner-merge
    *   8ca0053 Commit below the merge commit
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"
    {
        Sha1(2fc288c36c8bb710c78203f78ea9883724ce142b): Sha1(16b7c68c1dae39aa8b5e4c56e3bc4b1d508cfb25),
        Sha1(e8ee978dac10e6a85006543ef08be07c5824b4f7): Sha1(f699c45a85f07b90f2d9531269d79b2ffbf3795e),
    }
    ");

    Ok(())
}

/// Inserting above a commit should inherit it's parents
#[test]
fn insert_above_commit_with_two_children() -> Result<()> {
    let (repo, _tmpdir, mut meta) = fixture_writable("merge-in-the-middle")?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * e8ee978 (HEAD -> with-inner-merge) on top of inner merge
    *   2fc288c Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    let graph = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        standard_options(),
    )?
    .validated()?;

    let mut ws = graph.into_workspace()?;
    let mut editor = Editor::create(&mut ws, &mut *meta, &repo)?;

    let base_id = repo.rev_parse_single("base")?;

    // Create a commit that we can stick below the merge commit
    let mut base_obj = but_core::Commit::from_id(base_id)?;
    base_obj.message = "Commit above base commit".into();
    base_obj.parents = vec![].into();
    let new_commit = repo.write_object(base_obj.inner)?.detach();

    // select the merge commit
    let selector = editor
        .select_commit(base_id.detach())
        .context("Failed to find commit a in editor graph")?;
    // replace it with the new one
    editor.insert(selector, Step::new_pick(new_commit), InsertSide::Above)?;

    let outcome = editor.rebase()?;
    let overlayed = graph_tree(&outcome.overlayed_graph()?).to_string();
    insta::assert_snapshot!(overlayed, @"

    └── 👉►:0[0]:with-inner-merge[🌳]
        └── ·42f9ff4 (⌂|1)
            └── ►:1[1]:anon:
                └── ·5219d30 (⌂|1)
                    ├── ►:2[2]:A
                    │   └── ·72d9d9b (⌂|1)
                    │       └── ►:4[3]:main
                    │           ├── ·3dc4e45 (⌂|1) ►tags/base
                    │           └── 🏁·8f0d338 (⌂|1)
                    └── ►:3[2]:B
                        └── ·df0cf44 (⌂|1)
                            └── →:4: (main)
    ");
    let outcome = outcome.materialize()?;
    assert_eq!(overlayed, graph_tree(&outcome.workspace.graph).to_string());

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 42f9ff4 (HEAD -> with-inner-merge) on top of inner merge
    *   5219d30 Merge branch 'B' into with-inner-merge
    |\  
    | * df0cf44 (B) C: new file with 10 lines
    * | 72d9d9b (A) A: 10 lines on top
    |/  
    * 3dc4e45 (tag: base, main) Commit above base commit
    * 8f0d338 base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"
    {
        Sha1(2fc288c36c8bb710c78203f78ea9883724ce142b): Sha1(5219d30048fd87943c2c87401527b75f26a1f8be),
        Sha1(984fd1c6d3975901147b1f02aae6ef0a16e5904e): Sha1(df0cf447d953bcb8e79bac528f65f53bd498b9d2),
        Sha1(add59d26b2ffd7468fcb44c2db48111dd8f481e5): Sha1(72d9d9b11a71e30cc0ae1d14d03038568f0d964c),
        Sha1(e8aafee980f055ee43ef702a2d159fec9b781db1): Sha1(3dc4e4586430dfca273b5bdefc0e7fc5cd99de82),
        Sha1(e8ee978dac10e6a85006543ef08be07c5824b4f7): Sha1(42f9ff4611aecf1274b35a44b09ab2054b12d52d),
    }
    ");

    Ok(())
}

#[test]
#[ignore]
fn inserts_violating_fp_protection_should_cause_rebase_failure() -> Result<()> {
    panic!("Branch protection hasn't been implemented yet");
}
