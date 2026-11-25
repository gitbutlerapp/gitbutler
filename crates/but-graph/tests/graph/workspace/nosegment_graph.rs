use but_testsupport::{id_by_rev, visualize_commit_graph_all};
use snapbox::prelude::*;

use super::target_meta;
use crate::{
    support::graph_dag,
    utils::{read_only_in_memory_scenario, standard_options},
};

#[test]
fn walk_stops_below_the_target_commit() -> anyhow::Result<()> {
    let (repo, meta, mut db) = read_only_in_memory_scenario("history-below-target")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 6bfd487 (HEAD -> A) A
* 203882d (origin/main, main) M
* 75fb04b old-2
* cb41913 old-1

"#]]
    );
    let ws = but_testsupport::graph_from_head(
        &repo,
        &*meta,
        target_meta(&repo),
        &mut db,
        standard_options(),
    )?
    .validated()?
    .into_workspace()?;

    let graph = but_graph::nosegment::workspace_segment_graph(
        &ws,
        &repo,
        &*meta,
        vec![repo.head_id()?.detach()],
    )?;
    let mut subjects = Vec::new();
    for sidx in graph.segments() {
        for commit in &graph[sidx].commits {
            subjects.push(
                repo.find_commit(commit.id)?
                    .message()?
                    .summary()
                    .to_string(),
            );
        }
    }
    subjects.sort();
    assert_eq!(
        subjects,
        ["A", "M", "old-2"],
        "the walk ends right below the target commit, which no tip needs it to go past"
    );
    Ok(())
}

#[test]
fn commits_limit_bounds_only_a_walk_without_target_commit() -> anyhow::Result<()> {
    let (repo, meta, _db) = read_only_in_memory_scenario("history-below-target")?;
    let head_ref = repo.head_name()?;
    let head_id = repo.head_id()?.detach();
    // The first commit, so the walk down to it takes longer than the limit.
    let target_commit_id = repo.rev_parse_single("A~3")?.detach();
    // No target branch, so the target commit isn't walked as a head that the limit can't cut.
    let subjects = |target_commit_id| -> anyhow::Result<Vec<String>> {
        let graph = but_graph::nosegment::segment_graph(
            &repo,
            &*meta,
            but_graph::init::Overlay::default(),
            Default::default(),
            head_ref.as_ref().map(|name| name.as_ref()),
            Some(head_id),
            vec![head_id],
            Vec::new(),
            target_commit_id,
            false,
            Some(1),
        )?;
        let mut subjects = Vec::new();
        for sidx in graph.segments() {
            for commit in &graph[sidx].commits {
                subjects.push(
                    repo.find_commit(commit.id)?
                        .message()?
                        .summary()
                        .to_string(),
                );
            }
        }
        subjects.sort();
        Ok(subjects)
    };

    assert_eq!(
        subjects(None)?,
        ["A", "M"],
        "without a target commit, the walk stops after the limit, at a commit that keeps its parents"
    );
    assert_eq!(
        subjects(Some(target_commit_id))?,
        ["A", "M", "old-1", "old-2"],
        "the target commit bounds the walk instead of the limit"
    );
    Ok(())
}

#[test]
fn worktree_lane_lands_on_the_target_commit_beside_another_worktree() -> anyhow::Result<()> {
    use but_graph::petgraph::{Direction, visit::EdgeRef};
    let (repo, meta, mut db) =
        read_only_in_memory_scenario("worktree-merged-beside-worktree-on-target")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   bc6525f (origin/main) merge feature
|\  
| * ba2a20b (feature) F2
| * fe8cdb4 F1
* | ce9073f other
|/  
* 965998b (HEAD -> main, idle) base

"#]]
        .raw()
    );
    // Adoption already ran, so the fixture worktrees count as active.
    db.worktree_meta_mut().mark_adopted()?;
    let project_meta = but_core::ref_metadata::ProjectMeta {
        target_commit_id: Some(id_by_rev(&repo, "main").detach()),
        target_ref: Some(but_core::ref_metadata::LocalRefName::new(
            "refs/heads/main".try_into()?,
        )?),
        ..Default::default()
    };
    let graph = but_testsupport::graph_from_head(
        &repo,
        &*meta,
        project_meta,
        &mut db,
        but_graph::init::Options {
            worktrees: true,
            ..standard_options()
        },
    )?
    .validated()?;
    // With `idle` on `base` too, `main` owns no commit there, yet `feature` lands on `base`
    // rather than on `main`, which the main checkout has there.
    snapbox::assert_data_eq!(
        graph_dag(&graph),
        snapbox::str![[r#"
◎  feature[📁worktree-merged-beside-worktree-on-target-feature]
│ ◎  idle[📁worktree-merged-beside-worktree-on-target-idle]
│ │ ◎  origin/main
│ │ ●  🟣bc6525f (✓)
╭───┤
│ │ ●  🟣ce9073f (✓)
│ │ ◎  👉main[🌳@repo] <> origin/main
│ ├─╯
● │  ·ba2a20b (⌂|✓)
● │  ·fe8cdb4 (⌂|✓)
├─╯
●  🏁·965998b (⌂|✓)
"#]]
    );

    let feature_lane = graph.segment_id_by_commit_id(id_by_rev(&repo, "feature~1").detach())?;
    let lands_on: Vec<_> = graph
        .edges_directed(feature_lane, Direction::Outgoing)
        .map(|edge| &graph[edge.target()])
        .collect();
    assert_eq!(
        lands_on
            .iter()
            .map(|segment| (
                segment.ref_name().map(ToOwned::to_owned),
                segment.commits.first().map(|commit| commit.id)
            ))
            .collect::<Vec<_>>(),
        [(None, Some(id_by_rev(&repo, "main").detach()))],
        "the editor rebases `feature` only with what moves `base`, not with `main`"
    );
    Ok(())
}
