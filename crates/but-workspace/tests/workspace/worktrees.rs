use anyhow::Result;
use bstr::ByteSlice;
use but_graph::Graph;
use but_workspace::ref_info::LocalCommitRelation;
use but_workspace::worktrees::WorktreeBase;

use crate::ref_info::with_workspace_commit::utils::{StackState, add_stack, add_workspace};
use crate::utils::writable_scenario_slow;

/// Build a graph seeded with every active linked worktree of `repo`, the way `but-ctx`
/// does when the `worktreeManipulation` flag is on, and project the result.
fn ref_info_with_worktree_tips(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
) -> Result<but_workspace::RefInfo> {
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        push_remote: None,
    };
    let mut db = but_testsupport::in_memory_db();
    // Adoption already ran, so the fixture worktrees count as active.
    db.worktree_meta_mut().mark_adopted()?;
    let graph = Graph::from_head(
        repo,
        meta,
        project_meta,
        &mut db,
        but_graph::init::Options {
            worktrees: true,
            ..but_graph::init::Options::limited()
        },
    )?
    .validated()?;
    but_workspace::graph_to_ref_info(
        &graph.into_workspace()?,
        repo,
        but_workspace::ref_info::Options {
            expensive_commit_info: true,
            ..Default::default()
        },
    )
}

#[test]
fn worktrees_are_projected_onto_the_workspace() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    let mut meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?;
    add_workspace(&mut meta);
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);

    let info = ref_info_with_worktree_tips(&repo, &meta)?;
    let summary: Vec<_> = info
        .worktrees
        .iter()
        .map(|wt| {
            (
                wt.name.to_string(),
                wt.commits
                    .iter()
                    .map(|c| c.message.trim().as_bstr().to_string())
                    .collect::<Vec<_>>(),
                wt.base,
            )
        })
        .collect();

    let a1 = repo.rev_parse_single("A~1")?.detach();
    let a2 = repo.rev_parse_single("A")?.detach();
    let w1 = repo.rev_parse_single("wt-inside")?.detach();
    let m0 = repo.rev_parse_single("main~1")?.detach();
    let m1 = repo.rev_parse_single("main")?.detach();
    assert_eq!(
        summary,
        [
            (
                "wt-at".to_string(),
                Vec::new(),
                // Its `HEAD` *is* a workspace commit, so it owns nothing and rests right there.
                Some(WorktreeBase::InWorkspace(a2))
            ),
            (
                "wt-below".to_string(),
                vec!["U1".to_string()],
                // Branches off below the target without sitting on the target commit itself -
                // only its base being reachable from the target reveals it is outside.
                Some(WorktreeBase::Outside(m0))
            ),
            (
                "wt-disjoint".to_string(),
                vec!["D1".to_string()],
                // Unrelated history - the walk runs out of graph without finding a base.
                None
            ),
            (
                "wt-inside".to_string(),
                vec!["W1".to_string()],
                // Its commit branches off a commit that stack A owns.
                Some(WorktreeBase::InWorkspace(a1))
            ),
            (
                "wt-outside".to_string(),
                vec!["O1".to_string()],
                // The target commit stops the walk before it can reach the workspace.
                Some(WorktreeBase::Outside(m1))
            ),
            (
                "wt-stacked".to_string(),
                vec!["S1".to_string()],
                // Stacked on wt-inside, which is listed first and thus owns W1 exclusively.
                Some(WorktreeBase::InWorkspace(w1))
            ),
        ]
    );

    for wt in &info.worktrees {
        for commit in &wt.commits {
            assert_eq!(
                commit.relation,
                LocalCommitRelation::LocalOnly,
                "never-pushed worktree commits must not pretend to be on a remote"
            );
        }
    }
    Ok(())
}

#[test]
fn worktrees_are_empty_without_seeded_tips() -> Result<()> {
    let mut db = but_testsupport::in_memory_db();
    let (repo, _tmp) = writable_scenario_slow("worktree-workspace");
    let meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?;
    let graph = Graph::from_head(
        &repo,
        &meta,
        Default::default(),
        &mut db,
        but_graph::init::Options::limited(),
    )?;
    let info =
        but_workspace::graph_to_ref_info(&graph.into_workspace()?, &repo, Default::default())?;
    assert!(
        info.worktrees.is_empty(),
        "worktrees are only projected when the traversal was seeded with their tips"
    );
    Ok(())
}

#[test]
fn deep_disjoint_history_is_never_mistaken_for_being_below_the_target() -> Result<()> {
    let (repo, _tmp) = writable_scenario_slow("worktree-disjoint-deep");
    let mut meta = but_meta::VirtualBranchesTomlMetadata::from_path(
        repo.path().join("should-never-be-written.toml"),
    )?;
    add_workspace(&mut meta);
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);

    let info = ref_info_with_worktree_tips(&repo, &meta)?;
    let wt = &info.worktrees[0];
    assert_eq!(wt.name.to_string(), "wt-deep");
    assert_eq!(
        wt.commits
            .iter()
            .map(|c| c.message.trim().as_bstr().to_string())
            .collect::<Vec<_>>(),
        ["D5", "D4", "D3", "D2", "D1"],
        "every commit of the unrelated history is owned by the worktree"
    );
    assert_eq!(
        wt.base, None,
        "unrelated history has no base, no matter how deep its own chain is"
    );
    Ok(())
}
