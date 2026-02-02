use but_graph::Graph;
use but_testsupport::visualize_commit_graph_all;

use crate::init::utils::{
    add_workspace, add_workspace_without_target, read_only_in_memory_scenario, standard_options,
    standard_options_with_extra_target,
};

#[test]
fn with_target_ref() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   59a427f (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * a62b0de (A) A2
    | * 120a217 A1
    * | 0a415d8 (main) M3
    | | * 1f5c47b (origin/main) RM1
    | |/  
    |/|   
    * | 73ba99d M2
    |/  
    * fafd9d0 init
    ");

    add_workspace(&mut meta);

    let ws = Graph::from_head(&repo, &*meta, standard_options())?
        .validated()?
        .into_workspace()?;

    // We have a target_ref but nothing else
    assert!(ws.target_ref.is_some());
    assert!(ws.target_commit.is_none());
    assert!(ws.extra_target.is_none());

    let main_id = repo.rev_parse_single("main")?.detach();

    let merge_base = ws.merge_base_with_target_branch(main_id);
    let expected = repo.rev_parse_single(":/M2")?;
    assert_eq!(merge_base, Some(expected.detach()));

    Ok(())
}

#[test]
fn with_extra_target_when_no_target_ref() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/two-branches-one-below-base")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   e82dfab (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * 6fdab32 (A) A1
    * | 78b1b59 (B) B1
    | | * 938e6f2 (origin/main, main) M4
    | |/  
    |/|   
    * | f52fcec M3
    |/  
    * bce0c5e M2
    * 3183e43 M1
    ");

    add_workspace(&mut meta);
    meta.data_mut().default_target = None;

    // Use extra_target to set a lower bound
    let graph = Graph::from_head(&repo, &*meta, standard_options_with_extra_target(&repo, "main"))?.validated()?;
    let ws = graph.into_workspace()?;

    assert!(ws.target_ref.is_none());
    assert!(ws.target_commit.is_none());
    assert!(ws.extra_target.is_some());

    let a_id = repo.rev_parse_single("A")?.detach();

    let merge_base = ws.merge_base_with_target_branch(a_id);
    let expected = repo.rev_parse_single(":/M2")?;
    assert_eq!(merge_base, Some(expected.detach()));

    Ok(())
}

#[test]
fn returns_none_when_no_target_is_set() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;

    add_workspace_without_target(&mut meta);
    let graph = Graph::from_head(&repo, &*meta, standard_options())?.validated()?;
    let ws = graph.into_workspace()?;

    assert!(ws.target_ref.is_none(), "should not have target_ref");
    assert!(ws.extra_target.is_none(), "should not have extra_target");
    assert!(ws.target_commit.is_none(), "should not have target_commit");

    let a2_id = repo.rev_parse_single("A")?.detach();
    let merge_base = ws.merge_base_with_target_branch(a2_id);
    assert!(merge_base.is_none(), "can't compute merge-base without the other side");

    Ok(())
}

#[test]
fn returns_none_when_commit_not_in_graph() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;

    add_workspace(&mut meta);
    let ws = Graph::from_head(&repo, &*meta, standard_options())?
        .validated()?
        .into_workspace()?;

    let merge_base = ws.merge_base_with_target_branch(repo.object_hash().null());
    assert!(merge_base.is_none(), "should return None when commit is not in graph");

    Ok(())
}
