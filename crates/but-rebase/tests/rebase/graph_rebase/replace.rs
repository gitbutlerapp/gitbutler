//! These tests exercise the replace operation.
use anyhow::{Context, Result};
use but_graph::Graph;
use but_rebase::graph_rebase::{GraphExt, Step};
use but_testsupport::{git_status, visualize_commit_graph_all, visualize_tree};

use crate::{
    graph_rebase::set_var,
    utils::{fixture_writable, standard_options},
};

#[test]
fn reword_a_commit() -> Result<()> {
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

    let head_tree = repo.head_tree()?.id;

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut editor = graph.to_editor(&repo)?;

    // get the origional a
    let a = repo.rev_parse_single("A")?.detach();

    // reword commit a
    let a_obj = repo.find_commit(a)?;
    let mut a_obj = a_obj.decode()?;
    a_obj.message = "A: a second coming".into();
    let a_new = repo.write_object(a_obj)?.detach();

    // select the origional a out of the graph
    let a_selector = editor
        .select_commit(a)
        .context("Failed to find commit a in editor graph")?;
    // replace it with the new one
    editor.replace(
        &a_selector,
        Step::Pick {
            id: a_new,
            preserved_parents: None,
        },
    );

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    assert_eq!(head_tree, repo.head_tree()?.id);

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 4894b95 (HEAD -> with-inner-merge) on top of inner merge
    *   af38519 Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | 6de6b92 (A) A: a second coming
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    ");

    Ok(())
}

#[test]
fn ammend_a_commit() -> Result<()> {
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

    let head_tree = repo.head_tree()?.id();
    insta::assert_snapshot!(visualize_tree(head_tree), @r#"
    f766d1f
    ├── added-after-with-inner-merge:100644:861be1b "seq 10\n"
    ├── file:100644:d78dd4f "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    └── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    "#);

    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;

    let mut editor = graph.to_editor(&repo)?;

    // get the origional a
    let a = repo.rev_parse_single("A")?;
    insta::assert_snapshot!(visualize_tree(a), @r#"
    0cc630c
    └── file:100644:d78dd4f "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    "#);

    // reword commit a
    let mut a_obj = but_core::Commit::from_id(a)?;

    let mut builder = repo.edit_tree(a_obj.tree)?;
    let new_blob = repo.write_blob("I'm a new file :D\n")?;
    builder.upsert("new-file.txt", gix::objs::tree::EntryKind::Blob, new_blob)?;
    let tree = builder.write()?;

    a_obj.tree = tree.detach();
    a_obj.message = "A: a second coming".into();
    let a_new = repo.write_object(a_obj.inner)?.detach();

    // select the origional a out of the graph
    let a_selector = editor
        .select_commit(a.detach())
        .context("Failed to find commit a in editor graph")?;
    // replace it with the new one
    editor.replace(
        &a_selector,
        Step::Pick {
            id: a_new,
            preserved_parents: None,
        },
    );

    let outcome = editor.rebase()?;
    outcome.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 4a8642c (HEAD -> with-inner-merge) on top of inner merge
    *   fa3d6b8 Merge branch 'B' into with-inner-merge
    |\  
    | * 984fd1c (B) C: new file with 10 lines
    * | f1905a8 (A) A: a second coming
    |/  
    * 8f0d338 (tag: base, main) base
    ");
    insta::assert_snapshot!(git_status(&repo)?, @r"
    ");

    // A should include our extra blob
    let a = repo.rev_parse_single("A")?;
    insta::assert_snapshot!(visualize_tree(a), @r#"
    0c482d4
    ├── file:100644:d78dd4f "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    └── new-file.txt:100644:715faaf "I\'m a new file :D\n"
    "#);

    // New head tree should also include our extra blob
    let new_head_tree = repo.head_tree()?.id();
    insta::assert_snapshot!(visualize_tree(new_head_tree), @r#"
    89042ca
    ├── added-after-with-inner-merge:100644:861be1b "seq 10\n"
    ├── file:100644:d78dd4f "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    ├── new-file:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    └── new-file.txt:100644:715faaf "I\'m a new file :D\n"
    "#);

    Ok(())
}

#[test]
#[ignore]
fn replaces_violating_fp_protection_should_cause_rebase_failure() -> Result<()> {
    panic!("Branch protection hasn't been implemented dyet");
}
