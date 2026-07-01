use std::collections::HashSet;

use anyhow::Result;
use but_testsupport::{read_only_in_memory_scenario, visualize_commit_graph_all};
use gitbutler_repo::commit_ids_excluding_reachable_from_with_graph;
use snapbox::IntoData;

#[test]
fn commit_ids_excluding_reachable_from_matches_hidden_walk_for_merge_history() -> Result<()> {
    let repo = read_only_in_memory_scenario("merge-history-prune")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
*   302203c (HEAD -> merged) merge C into merged
|\  
| *   ac3212d (C) merge D into C
| |\  
| | * f43cbb4 (D) D
| * | ecdf221 C
| |/  
* |   eac2241 (A) merge B into A
|\ \  
| |/  
|/|   
| * 7c77b77 (B) B
|/  
* e54fc74 A
* 2f0e583 (main) base

"#]]
        .raw()
    );
    let from = repo.rev_parse_single("merged")?.detach();
    let stop_before = repo.rev_parse_single("main")?.detach();
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());

    let actual: HashSet<_> =
        commit_ids_excluding_reachable_from_with_graph(&repo, from, stop_before, &mut graph)?
            .into_iter()
            .collect();
    let expected: HashSet<_> = [
        "302203c", "ac3212d", "f43cbb4", "ecdf221", "eac2241", "7c77b77", "e54fc74",
    ]
    .into_iter()
    .map(|spec| Ok(repo.rev_parse_single(spec)?.detach()))
    .collect::<Result<_>>()?;

    assert_eq!(actual, expected);
    Ok(())
}
