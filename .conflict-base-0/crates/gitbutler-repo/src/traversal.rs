use std::collections::HashSet;

use anyhow::Result;
use gix::{
    prelude::ObjectIdExt,
    revision::plumbing::{graph, merge_base::Flags},
    revwalk::Graph,
};

/// Return commits on `from`'s first-parent chain, stopping before the first
/// commit that is reachable from `stop_before`.
///
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
    let mut commit_ids = Vec::new();
    let mut seen = HashSet::new();
    let mut to_visit = vec![from];

    while let Some(commit_id) = to_visit.pop() {
        if !seen.insert(commit_id) {
            continue;
        }

        let reaches_hidden_history = match repo.merge_base_with_graph(commit_id, stop_before, graph)
        {
            Ok(merge_base) => merge_base.detach() == commit_id,
            Err(gix::repository::merge_base_with_graph::Error::NotFound { .. }) => false,
            Err(err) => return Err(err.into()),
        };
        if reaches_hidden_history {
            continue;
        }

        commit_ids.push(commit_id);
        let commit = repo.find_commit(commit_id)?;
        to_visit.extend(commit.parent_ids().map(|parent_id| parent_id.detach()));
    }

    Ok(commit_ids)
}
