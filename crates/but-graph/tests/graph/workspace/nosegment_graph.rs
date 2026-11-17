use but_graph::Graph;
use but_testsupport::visualize_commit_graph_all;

use super::target_meta;
use crate::init::utils::{read_only_in_memory_scenario, standard_options};

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
    let ws = Graph::from_head(
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
