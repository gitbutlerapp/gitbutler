use anyhow::Result;
use but_rebase::graph_rebase::{GraphExt as _, mutate::InsertSide};
use but_testsupport::visualize_commit_graph_all;
use but_workspace::commit::{insert_blank_commit, insert_blank_commit::RelativeTo};

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

#[test]
fn insert_below_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("two")?;

    let editor = graph.to_editor(&repo)?;
    insert_blank_commit(editor, InsertSide::Below, RelativeTo::Commit(id.detach()))?
        .0
        .materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 507be22 (HEAD -> three) commit three
    * b63edf0 (two) commit two
    * 335e397 
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_above_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("two")?;

    let editor = graph.to_editor(&repo)?;
    insert_blank_commit(editor, InsertSide::Above, RelativeTo::Commit(id.detach()))?
        .0
        .materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3513948 (HEAD -> three) commit three
    * c5af9ae (two) 
    * 16fd221 (origin/two) commit two
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_below_reference() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let reference = repo.find_reference("two")?;

    let editor = graph.to_editor(&repo)?;
    insert_blank_commit(
        editor,
        InsertSide::Below,
        RelativeTo::Reference(reference.name()),
    )?
    .0
    .materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3513948 (HEAD -> three) commit three
    * c5af9ae (two) 
    * 16fd221 (origin/two) commit two
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_above_reference() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let reference = repo.find_reference("two")?;

    let editor = graph.to_editor(&repo)?;
    insert_blank_commit(
        editor,
        InsertSide::Above,
        RelativeTo::Reference(reference.name()),
    )?
    .0
    .materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 3513948 (HEAD -> three) commit three
    * c5af9ae 
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}
