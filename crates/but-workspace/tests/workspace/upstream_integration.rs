use anyhow::Result;
use but_rebase::graph_rebase::mutate::RelativeTo;
use but_testsupport::visualize_commit_graph_all;
use but_workspace::{BottomUpdate, BottomUpdateKind, integrate_upstream};
use gix::prelude::ObjectIdExt as _;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack, add_stack_with_segments,
    named_writable_scenario_with_description_and_graph,
};

#[test]
fn rebase_single_segment_stack_onto_advanced_target() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "integrate-upstream-single-segment-target-ahead",
            |meta| {
                add_stack(meta, 1, "A", StackState::InWorkspace);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c2878fb (HEAD -> gitbutler/workspace, A) A2
    * 49d4b34 A1
    | * b2c2849 (origin/main, main) upstream
    |/  
    * 3183e43 M1
    ");

    let target_id = repo
        .find_reference("refs/remotes/origin/main")?
        .id()
        .detach();
    let bottom_id = repo.rev_parse_single("A~1")?.detach();
    let mut ws = graph.into_workspace()?;

    let but_workspace::IntegrateUpstreamOutcome { ws_meta, rebase } = integrate_upstream(
        &mut ws,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(bottom_id),
        }],
    )?;

    assert_eq!(ws_meta.target_commit_id, Some(target_id));

    rebase.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 432c34f (HEAD -> gitbutler/workspace, A) A2
    * a4ef8b7 A1
    * b2c2849 (origin/main, main) upstream
    * 3183e43 M1
    ");

    Ok(())
}

#[test]
fn rebase_merge_bottom_commit_replaces_only_first_parent() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "integrate-upstream-merge-bottom",
            |meta| {
                add_stack(meta, 1, "A", StackState::InWorkspace);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   f58e85d (HEAD -> gitbutler/workspace, A) merge-bottom
    |\  
    | * 9d447b0 (side) side
    |/  
    | * b2c2849 (origin/main, main) upstream
    |/  
    * 3183e43 M1
    ");

    let target_id = repo
        .find_reference("refs/remotes/origin/main")?
        .id()
        .detach();
    let side_id = repo.rev_parse_single("side")?.detach();
    let old_bottom_id = repo.rev_parse_single("A")?.detach();
    let old_bottom = but_core::Commit::from_id(old_bottom_id.attach(&repo))?;
    let old_parents = old_bottom.inner.parents.iter().copied().collect::<Vec<_>>();
    assert_eq!(
        old_parents.len(),
        2,
        "fixture should produce a merge bottom commit"
    );
    assert_eq!(
        old_parents[1], side_id,
        "fixture should use side as second parent"
    );

    let mut ws = graph.into_workspace()?;
    let but_workspace::IntegrateUpstreamOutcome { ws_meta, rebase } = integrate_upstream(
        &mut ws,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(old_bottom_id),
        }],
    )?;

    assert_eq!(ws_meta.target_commit_id, Some(target_id));

    rebase.materialize()?;

    let new_bottom_id = repo.rev_parse_single("A")?.detach();
    let new_bottom = but_core::Commit::from_id(new_bottom_id.attach(&repo))?;
    let new_parents = new_bottom.inner.parents.iter().copied().collect::<Vec<_>>();

    assert_ne!(
        new_bottom_id, old_bottom_id,
        "rebase should rewrite the merge commit"
    );
    assert_eq!(
        new_parents,
        vec![target_id, side_id],
        "only the first parent should be replaced by the target ref"
    );
    assert_eq!(repo.rev_parse_single("side")?.detach(), side_id);
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   a0b70a5 (HEAD -> gitbutler/workspace, A) merge-bottom
    |\  
    | * 9d447b0 (side) side
    * | b2c2849 (origin/main, main) upstream
    |/  
    * 3183e43 M1
    ");

    Ok(())
}

#[test]
fn merge_empty_stack_with_advanced_target() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "integrate-upstream-empty-stack-with-target",
            |meta| {
                add_stack(meta, 1, "A", StackState::InWorkspace);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * b2c2849 (origin/main, main) upstream
    * 3183e43 (HEAD -> gitbutler/workspace, A) M1
    ");

    let target_id = repo
        .find_reference("refs/remotes/origin/main")?
        .id()
        .detach();
    let mut ws = graph.into_workspace()?;

    let but_workspace::IntegrateUpstreamOutcome { ws_meta, rebase } = integrate_upstream(
        &mut ws,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Merge,
            selector: RelativeTo::Reference("refs/heads/A".try_into()?),
        }],
    )?;

    assert_eq!(ws_meta.target_commit_id, Some(target_id));

    rebase.materialize()?;

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
    *   06bf2b0 (HEAD -> gitbutler/workspace, A) Merge refs/remotes/origin/main into refs/heads/A
    |\  
    | * b2c2849 (origin/main, main) upstream
    |/  
    * 3183e43 M1
    ");

    Ok(())
}

#[test]
fn merge_rejects_multi_segment_stack() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "integrate-upstream-two-segment-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * d69fe94 (B) B
    * 09d8e52 (A) A
    * 85efbe4 (origin/main, main) M
    ");

    let mut ws = graph.into_workspace()?;
    let err = integrate_upstream(
        &mut ws,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Merge,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )
    .err()
    .expect("merge should fail for multi-segment stacks");

    assert_eq!(
        err.to_string(),
        "Merge updates require exactly one matching single-segment stack"
    );

    Ok(())
}

#[test]
fn rejects_non_bottom_commit_selector() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "integrate-upstream-two-segment-stack",
            |meta| {
                add_stack_with_segments(meta, 1, "A", StackState::InWorkspace, &[]);
                add_stack_with_segments(meta, 2, "B", StackState::InWorkspace, &[]);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 2076060 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * d69fe94 (B) B
    * 09d8e52 (A) A
    * 85efbe4 (origin/main, main) M
    ");

    let mut ws = graph.into_workspace()?;
    let err = integrate_upstream(
        &mut ws,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("B")?.detach()),
        }],
    )
    .err()
    .expect("only bottom-most commits should be accepted");

    assert_eq!(
        err.to_string(),
        "Failed to discover desired bottom d69fe9427ac4a2422ab953acba483f804e8098ef"
    );

    Ok(())
}

#[test]
fn integrate_upstream_requires_target_ref() -> Result<()> {
    let (_tmp, graph, repo, mut meta, _description) =
        named_writable_scenario_with_description_and_graph(
            "integrate-upstream-no-target-empty-stack",
            |meta| {
                add_stack(meta, 1, "A", StackState::InWorkspace);
            },
        )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, main, A) M1");

    let mut ws = graph.into_workspace()?;
    let err = integrate_upstream(
        &mut ws,
        &mut meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Merge,
            selector: RelativeTo::Reference("refs/heads/A".try_into()?),
        }],
    )
    .err()
    .expect("workspaces without a target ref should be rejected");

    assert_eq!(
        err.to_string(),
        "Cannot update a workspace with no target ref"
    );

    Ok(())
}
