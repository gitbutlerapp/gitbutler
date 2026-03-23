use anyhow::Result;
use but_core::DiffSpec;
use but_rebase::graph_rebase::{Editor, LookupStep as _};
use but_workspace::commit::commit_amend;

use crate::ref_info::with_workspace_commit::utils::named_writable_scenario_with_description_and_graph as writable_scenario;

fn worktree_changes_as_specs(repo: &gix::Repository) -> Result<Vec<DiffSpec>> {
    Ok(but_core::diff::worktree_changes(repo)?
        .changes
        .into_iter()
        .map(DiffSpec::from)
        .collect())
}

#[test]
fn amend_commit_smoke_test() -> Result<()> {
    let (_tmp, graph, repo, mut _meta, _description) =
        writable_scenario("reword-three-commits", |_| {})?;
    let two_id = repo.rev_parse_single("two")?.detach();
    std::fs::write(
        repo.workdir_path("amended.txt").expect("non-bare"),
        "amended\n",
    )?;

    let mut ws = graph.into_workspace()?;
    let editor = Editor::create(&mut ws, &mut _meta, &repo)?;
    let outcome = commit_amend(editor, two_id, worktree_changes_as_specs(&repo)?, 0)?;

    assert!(outcome.rejected_specs.is_empty());
    let selector = outcome.commit_selector.expect("selector exists");
    let materialized = outcome.rebase.materialize()?;
    let rewritten_id = materialized.lookup_pick(selector)?;

    let rewritten_commit = repo.find_commit(rewritten_id)?;
    assert_eq!(rewritten_commit.message_raw()?, "commit two\n");
    let spec = format!("{rewritten_id}:amended.txt");
    let object_with_path = repo.rev_parse_single(spec.as_str())?;
    assert_eq!(object_with_path.object()?.kind, gix::objs::Kind::Blob);

    Ok(())
}
