use anyhow::Result;
use but_rebase::graph_rebase::GraphExt;
use but_testsupport::visualize_commit_graph_all;
use but_workspace::commit::reword;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

#[test]
fn reword_head_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("three")?;
    let editor = graph.to_editor(&repo)?;
    reword(editor, id.detach(), b"New name".into())?
        .0
        .materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * f0a8655 (HEAD -> three) New name
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn reword_middle_commit() -> Result<()> {
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
    reword(editor, id.detach(), b"New name".into())?
        .0
        .materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 9feebdd (HEAD -> three) commit three
    * ada51de (two) New name
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn reword_base_commit() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("one")?;
    let editor = graph.to_editor(&repo)?;
    reword(editor, id.detach(), b"New name".into())?
        .0
        .materialize()?;

    // We end up with two divergent histories here. This is to be expected if we
    // rewrite the very bottom commit in a repository.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * fe9fe7a (HEAD -> three) commit three
    * 096a124 (two) commit two
    * 1121ebc (one) New name
    * 16fd221 (origin/two) commit two
    * 8b426d0 commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}
