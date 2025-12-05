use anyhow::Result;
use but_rebase::graph_rebase::mutate::InsertSide;
use but_testsupport::{assure_stable_env, visualize_commit_graph_all};
use but_workspace::commit::insert_blank_commit;
use but_workspace::commit::insert_blank_commit::RelativeTo;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

#[test]
fn insert_below_commit() -> Result<()> {
    assure_stable_env();
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("two")?;

    insert_blank_commit(
        &graph,
        &repo,
        InsertSide::Below,
        RelativeTo::Commit(id.detach()),
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 31dbfdc (HEAD -> three) commit three
    * b65c813 (two) commit two
    * d2b480d 
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_above_commit() -> Result<()> {
    assure_stable_env();
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("two")?;

    insert_blank_commit(
        &graph,
        &repo,
        InsertSide::Above,
        RelativeTo::Commit(id.detach()),
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 923c9cd (HEAD -> three) commit three
    * 8bf04f0 (two) 
    * 16fd221 (origin/two) commit two
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_below_reference() -> Result<()> {
    assure_stable_env();
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let reference = repo.find_reference("two")?;

    insert_blank_commit(
        &graph,
        &repo,
        InsertSide::Below,
        RelativeTo::Reference(reference.name()),
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 923c9cd (HEAD -> three) commit three
    * 8bf04f0 (two) 
    * 16fd221 (origin/two) commit two
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn insert_above_reference() -> Result<()> {
    assure_stable_env();
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let reference = repo.find_reference("two")?;

    insert_blank_commit(
        &graph,
        &repo,
        InsertSide::Above,
        RelativeTo::Reference(reference.name()),
    )?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 923c9cd (HEAD -> three) commit three
    * 8bf04f0 
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}
