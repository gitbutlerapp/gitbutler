//! These tests exercise the insert operation.
use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{GraphExt, Step, mutate::InsertSide};
use but_testsupport::{git_status, visualize_commit_graph_all};

use crate::{
    graph_rebase::set_var,
    utils::{fixture_writable, standard_options},
};

/// Inserting below a merge commit should inherit all of it's parents
#[test]
fn insert_below_merge_commit() -> Result<()> {
    set_var("GITBUTLER_CHANGE_ID", "1");
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
    insta::assert_snapshot!(git_status(&repo)?, @r"
    ");

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
    editor.insert(
        &selector,
        Step::Pick {
            id: new_commit,
            preserved_parents: None,
        },
        InsertSide::Below,
    );

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * bd07069 (HEAD -> with-inner-merge) on top of inner merge
    * 3b80a45 Merge branch 'B' into with-inner-merge
    *   181b10a Commit below the merge commit
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    ");

    Ok(())
}

/// Inserting above a commit should inherit it's parents
#[test]
fn insert_above_commit_with_two_children() -> Result<()> {
    set_var("GITBUTLER_CHANGE_ID", "1");
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
    insta::assert_snapshot!(git_status(&repo)?, @r"
    ");

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
    editor.insert(
        &selector,
        Step::Pick {
            id: new_commit,
            preserved_parents: None,
        },
        InsertSide::Above,
    );

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 64487d8 (HEAD -> with-inner-merge) on top of inner merge
    *   04fa958 Merge branch 'B' into with-inner-merge
    |\  
    | * dd12e5b (B) C: new file with 10 lines
    * | f2c305e (A) A: 10 lines on top
    |/  
    * 6c1b9f3 (tag: base, main) Commit above base commit
    * 8f0d338 base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    ");

    Ok(())
}

#[test]
#[ignore]
fn inserts_violating_fp_protection_should_cause_rebase_failure() -> Result<()> {
    panic!("Branch protection hasn't been implemented dyet");
}
