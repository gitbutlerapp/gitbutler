use but_graph::Graph;
use but_testsupport::visualize_commit_graph_all;
use snapbox::IntoData;

use super::target_meta;
use crate::init::utils::{add_workspace, read_only_in_memory_scenario, standard_options};

#[test]
fn with_target_ref() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
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

"#]]
        .raw()
    );

    add_workspace(&mut meta);

    let ws = Graph::from_head(
        &repo,
        target_meta(&repo),
        &mut meta.connection_mut(),
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    assert!(ws.target_ref.is_some());

    let main_id = repo.rev_parse_single("main")?.detach();

    let res = ws.merge_base_with_target_branch(main_id);
    let expected_merge_base = repo.rev_parse_single(":/M2")?.detach();
    let expected_target_id = repo.rev_parse_single("origin/main")?.detach();
    assert_eq!(res, Some((expected_merge_base, expected_target_id)));

    Ok(())
}

#[test]
fn returns_none_when_no_target_is_set() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;

    add_workspace(&mut meta);
    let graph = Graph::from_head(
        &repo,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut meta.connection_mut(),
        standard_options(),
    )?
    .validated()?;
    let ws = graph.into_workspace()?;

    assert!(ws.target_ref.is_none(), "should not have target_ref");
    assert!(ws.target_commit.is_none(), "should not have target_commit");

    let a2_id = repo.rev_parse_single("A")?.detach();
    let res = ws.merge_base_with_target_branch(a2_id);
    assert!(
        res.is_none(),
        "can't compute merge-base without the other side"
    );

    Ok(())
}

#[test]
fn returns_none_when_commit_not_in_graph() -> anyhow::Result<()> {
    let (repo, mut meta) = read_only_in_memory_scenario("ws/local-target-and-stack")?;

    add_workspace(&mut meta);
    let ws = Graph::from_head(
        &repo,
        target_meta(&repo),
        &mut meta.connection_mut(),
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    let res = ws.merge_base_with_target_branch(repo.object_hash().null());
    assert!(
        res.is_none(),
        "should return None when commit is not in graph"
    );

    Ok(())
}
