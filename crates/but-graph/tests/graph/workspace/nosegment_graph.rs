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

    let graph = ws.nosegment_graph(&repo, &*meta, vec![repo.head_id()?.detach()])?;
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
