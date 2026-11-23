use std::collections::HashSet;

use anyhow::Result;
use but_testsupport::{
    CommandExt as _, git, open_repo, read_only_in_memory_scenario, visualize_commit_graph_all,
    writable_scenario,
};
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

#[test]
fn hidden_walk_respects_shallow_boundaries_on_both_sides() -> Result<()> {
    let (repo, _tmp) = writable_scenario("merge-history-prune");
    let from = repo.rev_parse_single("A")?.detach();
    let hidden = repo.rev_parse_single("merged")?.detach();
    std::fs::write(
        repo.git_dir().join("shallow"),
        format!("{from}\n{hidden}\n"),
    )?;
    let mut graph = repo.revision_graph(None);

    assert_eq!(
        commit_ids_excluding_reachable_from_with_graph(&repo, from, hidden, &mut graph)?,
        [from],
        "the hidden tip cannot reach the visible tip, and the visible tip is also shallow"
    );
    Ok(())
}

#[test]
fn hidden_walk_respects_replacements_with_a_cached_commit_graph() -> Result<()> {
    let (repo, _tmp) = writable_scenario("merge-history-prune");
    git(&repo)
        .args(["commit-graph", "write", "--reachable"])
        .run();
    let cache = repo.commit_graph()?;
    let from = repo.rev_parse_single("A")?.detach();
    let hidden = repo.rev_parse_single("merged")?.detach();
    let b = repo.rev_parse_single("B")?.detach();
    let mut graph = repo.revision_graph(Some(&cache));
    assert!(
        commit_ids_excluding_reachable_from_with_graph(&repo, from, hidden, &mut graph)?.is_empty(),
        "the original hidden tip contains A"
    );
    git(&repo).args(["replace", "merged", "D"]).run();
    let repo = open_repo(repo.git_dir())?;

    assert_eq!(
        commit_ids_excluding_reachable_from_with_graph(&repo, from, hidden, &mut graph)?,
        [from, b],
        "the replacement for the hidden tip shares only the history beneath A and B"
    );
    Ok(())
}
