use anyhow::Result;
use but_testsupport::{assure_stable_env, visualize_commit_graph_all};
use but_workspace::commit::reword;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

#[test]
fn reword_head_commit() -> Result<()> {
    assure_stable_env();
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("three")?;
    reword(&graph, &repo, id.detach(), b"New name".into())?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 7580b8e (HEAD -> three) New name
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn reword_middle_commit() -> Result<()> {
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
    reword(&graph, &repo, id.detach(), b"New name".into())?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 086ad49 (HEAD -> three) commit three
    * d9cea5b (two) New name
    | * 16fd221 (origin/two) commit two
    |/  
    * 8b426d0 (one) commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}

#[test]
fn reword_base_commit() -> Result<()> {
    assure_stable_env();
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * c9f444c (HEAD -> three) commit three
    * 16fd221 (origin/two, two) commit two
    * 8b426d0 (one) commit one
    ");

    let head_tree = repo.head_tree_id()?;
    let id = repo.rev_parse_single("one")?;
    reword(&graph, &repo, id.detach(), b"New name".into())?;

    // We end up with two divergent histories here. This is to be expected if we
    // rewrite the very bottom commit in a repository.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    * 33b56b8 (HEAD -> three) commit three
    * 2548c60 (two) commit two
    * b8c5693 (one) New name
    * 16fd221 (origin/two) commit two
    * 8b426d0 commit one
    ");

    assert_eq!(head_tree, repo.head_tree_id()?);

    Ok(())
}
