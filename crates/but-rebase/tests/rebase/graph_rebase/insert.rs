//! These tests exercise the insert operation.
use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{GraphExt, Step, mutate::InsertSide};
use but_testsupport::{git_status, visualize_commit_graph_all};

use crate::utils::{fixture_writable, standard_options};

/// Inserting below a merge commit should inherit all of it's parents
#[test]
fn insert_below_merge_commit() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("merge-in-the-middle")?;

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

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut editor = graph.to_editor(&repo)?;

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
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * dca85f5 (HEAD -> with-inner-merge) on top of inner merge
    * dfff8d1 Merge branch 'B' into with-inner-merge
    *   f593b23 Commit below the merge commit
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

/// Inserting above a commit should inherit it's parents
#[test]
fn insert_above_commit_with_two_children() -> Result<()> {
    let (repo, _tmpdir, meta) = fixture_writable("merge-in-the-middle")?;

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

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut editor = graph.to_editor(&repo)?;

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
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * a371fba (HEAD -> with-inner-merge) on top of inner merge
    *   b311d2a Merge branch 'B' into with-inner-merge
    |\  
    | * 5101a39 (B) C: new file with 10 lines
    * | 2bb9fcc (A) A: 10 lines on top
    |/  
    * 1f3bf7d (tag: base, main) Commit above base commit
    * 8f0d338 base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");

    Ok(())
}

#[test]
#[ignore]
fn inserts_violating_fp_protection_should_cause_rebase_failure() -> Result<()> {
    panic!("Branch protection hasn't been implemented yet");
}
