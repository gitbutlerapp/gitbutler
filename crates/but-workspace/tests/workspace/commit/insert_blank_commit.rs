use anyhow::Result;
use but_rebase::graph_rebase::{
    GraphExt as _,
    mutate::{InsertSide, RelativeTo},
};
use but_testsupport::visualize_commit_graph_all;
use but_workspace::commit::insert_blank_commit;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

#[test]
fn insert_below_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
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
    * 70ba329 (HEAD -> three) commit three
    * 0f4f9d0 (two) commit two
    * b3b14c2 
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_above_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
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
    * 8e87ff3 (HEAD -> three) commit three
    * 024b774 (two) 
    * 16fd221 (origin/two) commit two
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_below_reference() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let reference = repo.find_reference("two")?;

    let editor = graph.to_editor(&repo)?;
    insert_blank_commit(editor, InsertSide::Below, RelativeTo::Reference(reference.name()))?
        .0
        .materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8e87ff3 (HEAD -> three) commit three
    * 024b774 (two) 
    * 16fd221 (origin/two) commit two
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_above_reference() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) = writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let reference = repo.find_reference("two")?;

    let editor = graph.to_editor(&repo)?;
    insert_blank_commit(editor, InsertSide::Above, RelativeTo::Reference(reference.name()))?
        .0
        .materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 8e87ff3 (HEAD -> three) commit three
    * 024b774 
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}
