use but_graph::Graph;
use but_testsupport::visualize_commit_graph_all;

use crate::init::utils::{
    add_workspace, add_workspace_with_target, add_workspace_without_target,
    read_only_in_memory_scenario, standard_options, standard_options_with_extra_target,
};

#[test]
fn returns_target_tip_when_stacks_have_different_bases() -> anyhow::Result<()> {
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

    // A branches from M2, B branches from M3.
    // resolved_target_commit_id should return M4 (the tip of origin/main).
    add_workspace(&mut meta);

    let ws = Graph::from_head(&repo, &*meta, standard_options())?
        .validated()?
        .into_workspace()?;

    let tip = ws.resolved_target_commit_id();
    let expected_m4 = repo.rev_parse_single(":/M4")?.detach();
    assert_eq!(
        tip,
        Some(expected_m4),
        "should return M4, the tip of origin/main"
    );

    Ok(())
}

#[test]
fn returns_target_tip_when_one_stack_is_above_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/two-branches-one-above-base")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   c5587c9 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    |\  
    | * de6d39c (A) A1
    | * a821094 (origin/main, main) M3
    * | ce25240 (B) B1
    |/  
    * bce0c5e M2
    * 3183e43 M1
    ");

    // A branches from M3 (which is also origin/main), B branches from M2.
    // resolved_target_commit_id should return M3 (the tip of origin/main).
    add_workspace(&mut meta);

    let ws = Graph::from_head(&repo, &*meta, standard_options())?
        .validated()?
        .into_workspace()?;

    let tip = ws.resolved_target_commit_id();
    let expected_m3 = repo.rev_parse_single(":/M3")?.detach();
    assert_eq!(
        tip,
        Some(expected_m3),
        "should return M3, the tip of origin/main"
    );

    Ok(())
}

#[test]
fn prefers_target_commit_over_target_ref() -> anyhow::Result<()> {
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

    // Set target_commit (default_target.sha) to M2, while target_ref points to origin/main (RM1).
    let m2 = repo.rev_parse_single(":/M2")?.detach();
    add_workspace_with_target(&mut meta, m2);

    let ws = Graph::from_head(&repo, &*meta, standard_options())?
        .validated()?
        .into_workspace()?;

    assert!(ws.target_ref.is_some(), "target_ref should be set");
    assert!(ws.target_commit.is_some(), "target_commit should be set");

    let result = ws.resolved_target_commit_id();
    assert_eq!(
        result,
        Some(m2),
        "should prefer stored target_commit (M2) over target_ref tip (RM1)"
    );

    Ok(())
}

#[test]
fn returns_none_when_no_target() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;

    add_workspace_without_target(&mut meta);
    let ws = Graph::from_head(&repo, &*meta, standard_options())?
        .validated()?
        .into_workspace()?;

    assert!(
        ws.resolved_target_commit_id().is_none(),
        "should return None when no target is set"
    );

    Ok(())
}

#[test]
fn returns_extra_target_without_target_ref() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/two-branches-one-below-base")?;

    add_workspace(&mut meta);
    meta.data_mut().default_target = None;

    let ws = Graph::from_head(
        &repo,
        &*meta,
        standard_options_with_extra_target(&repo, "main"),
    )?
    .validated()?
    .into_workspace()?;

    let expected_target_id = repo.rev_parse_single("main")?.detach();
    assert_eq!(
        ws.resolved_target_commit_id(),
        Some(expected_target_id),
        "extra integrated target is used as the effective target commit"
    );

    Ok(())
}
