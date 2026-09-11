use but_graph::Graph;

use super::target_meta;
use crate::init::utils::{add_workspace, read_only_in_memory_scenario, standard_options};

#[test]
fn child_most_stack_base_wins_over_the_target() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/two-branches-one-below-base")?;
    add_workspace(&mut meta);
    let ws = Graph::from_head(
        &repo,
        &*meta,
        target_meta(&repo),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    let m3 = repo.rev_parse_single(":/M3")?.detach();
    let m4 = repo.rev_parse_single(":/M4")?.detach();
    assert_eq!(
        ws.target_commit.as_ref().map(|t| t.commit_id),
        Some(m4),
        "the target is ahead of every stack"
    );
    assert_eq!(
        ws.highest_base(),
        Some(m3),
        "A rests on M2 and B on M3, so M3 is the child-most base"
    );
    Ok(())
}

#[test]
fn target_commit_without_stacks() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/worktree-behind-target")?;
    add_workspace(&mut meta);
    let ws = Graph::from_head(
        &repo,
        &*meta,
        target_meta(&repo),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    assert!(ws.stacks.is_empty(), "nothing rests on anything yet");
    assert_eq!(
        ws.highest_base(),
        ws.target_commit.as_ref().map(|t| t.commit_id),
        "an empty workspace starts new branches at its target"
    );
    Ok(())
}

#[test]
fn none_without_target() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = read_only_in_memory_scenario("ws/no-target-without-ws-commit")?;
    add_workspace(&mut meta);
    let ws = Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    assert_eq!(ws.highest_base(), None, "no target means no base to cut at");
    Ok(())
}
