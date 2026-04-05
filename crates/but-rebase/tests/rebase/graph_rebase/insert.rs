//! These tests exercise the insert operation.
use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{Editor, Step, mutate::InsertSide};
use but_testsupport::{git_status, visualize_commit_graph_all};

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

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

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
    let outcome = outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * ceb4158 (HEAD -> with-inner-merge) on top of inner merge
    * ea55b6e Merge branch 'B' into with-inner-merge
    *   ec48031 Commit below the merge commit
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"
    {
        Sha1(231acb683a6ecfb1ff546952057c4b3d3764b28c): Sha1(ec48031bb803e3711d7ce5646e80c72d8447aedc),
        Sha1(2fc288c36c8bb710c78203f78ea9883724ce142b): Sha1(ea55b6e69f7043c1afb46b8daeef988dc212be3c),
        Sha1(e8ee978dac10e6a85006543ef08be07c5824b4f7): Sha1(ceb4158ddbc6bc4c580c553e7d2aa6c9248d9d9e),
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

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

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
    let outcome = outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * ceb4158 (HEAD -> with-inner-merge) on top of inner merge
    * ea55b6e Merge branch 'B' into with-inner-merge
    *   ec48031 Commit below the merge commit
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | add59d2 (A) A: 10 lines on top
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"
    {
        Sha1(2fc288c36c8bb710c78203f78ea9883724ce142b): Sha1(ea55b6e69f7043c1afb46b8daeef988dc212be3c),
        Sha1(e8ee978dac10e6a85006543ef08be07c5824b4f7): Sha1(ceb4158ddbc6bc4c580c553e7d2aa6c9248d9d9e),
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

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

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
    let outcome = outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 9d2b9d9 (HEAD -> with-inner-merge) on top of inner merge
    *   8502201 Merge branch 'B' into with-inner-merge
    |\  
    | * 97c7cc6 (B) C: new file with 10 lines
    * | 0379d6c (A) A: 10 lines on top
    |/  
    * 055ead5 (tag: base, main) Commit above base commit
    * 8f0d338 base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @"");
    insta::assert_debug_snapshot!(outcome.history.commit_mappings(), @"
    {
        Sha1(2fc288c36c8bb710c78203f78ea9883724ce142b): Sha1(8502201c3e5e7ccd1acce001dd2c72c1af8f4a02),
        Sha1(984fd1c6d3975901147b1f02aae6ef0a16e5904e): Sha1(97c7cc62f0ddefff490fa673a101045dfd143749),
        Sha1(add59d26b2ffd7468fcb44c2db48111dd8f481e5): Sha1(0379d6c1d89617ff6c438e6e2ebdc9d5db4d831e),
        Sha1(e8aafee980f055ee43ef702a2d159fec9b781db1): Sha1(055ead578607b2021aeb80df7ac67e294d5272a9),
        Sha1(e8ee978dac10e6a85006543ef08be07c5824b4f7): Sha1(9d2b9d900e9ec5abbb44480114205fcad42ac1da),
    }
    ");

    Ok(())
}

#[test]
#[ignore]
fn inserts_violating_fp_protection_should_cause_rebase_failure() -> Result<()> {
    panic!("Branch protection hasn't been implemented yet");
}
