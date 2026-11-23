use std::collections::HashSet;

use anyhow::Result;
use gix::{
    prelude::ObjectIdExt,
    revision::plumbing::{graph, merge_base::Flags},
    revwalk::Graph,
};

/// The returned commits are ordered from `from` backwards along the first-parent chain, excluding
/// the first commit that is reachable from `stop_before` by ancestry.
pub fn first_parent_commit_ids_until(
    repo: &gix::Repository,
    from: gix::ObjectId,
    stop_before: gix::ObjectId,
) -> Result<Vec<gix::ObjectId>> {
    from.attach(repo)
        .ancestors()
        .first_parent_only()
        .with_hidden(Some(stop_before))
        .all()?
        .map(|info| Ok(info?.id))
        .collect()
}

/// Return commits reachable from `from` that are not reachable from `stop_before`.
///
/// This matches the semantics of walking `from` with `stop_before` hidden.
///
/// Reuse `graph` across repeated ancestry queries for better performance.
pub fn commit_ids_excluding_reachable_from_with_graph(
    repo: &gix::Repository,
    from: gix::ObjectId,
    stop_before: gix::ObjectId,
    graph: &mut Graph<'_, '_, graph::Commit<Flags>>,
) -> Result<Vec<gix::ObjectId>> {
    let shallow_commits = repo.shallow_commits()?;
    let mut commit_ids = Vec::new();
    let mut seen = HashSet::new();
    let mut to_visit = vec![from];

    while let Some(commit_id) = to_visit.pop() {
        if !seen.insert(commit_id) {
            continue;
        }

        if repo.is_ancestor_with_graph(commit_id, stop_before, graph)? {
            continue;
        }

        commit_ids.push(commit_id);
        if shallow_commits
            .as_ref()
            .is_some_and(|commits| commits.binary_search(&commit_id).is_ok())
        {
            continue;
        }
        let commit = repo.find_commit(commit_id)?;
        to_visit.extend(commit.parent_ids().map(|parent_id| parent_id.detach()));
    }

    Ok(commit_ids)
}
