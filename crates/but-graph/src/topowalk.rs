use anyhow::Context;
use smallvec::SmallVec;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque};

#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct Untraversed {
    // The latest untraversed commit will be popped from the BinaryHeap.
    time_seconds: i64,
    id: gix::ObjectId,
    parents_cache: SmallVec<[gix::ObjectId; 1]>,
}
struct Traversed {
    time_seconds: i64,
    parents: SmallVec<[gix::ObjectId; 1]>,
}

fn insert_untraversed(
    repo: &gix::Repository,
    id: gix::ObjectId,
    untraversed: &mut BinaryHeap<Untraversed>,
    untraversed_ids: &mut HashSet<gix::ObjectId>,
    shallow_boundary: Option<&gix::shallow::Commits>,
) -> anyhow::Result<()> {
    if !untraversed_ids.insert(id) {
        return Ok(());
    }
    let commit = repo.find_commit(id)?;
    untraversed.push(Untraversed {
        time_seconds: commit.time()?.seconds,
        id,
        // A shallow clone doesn't have the parents of the commits at its boundary.
        parents_cache: if shallow_boundary
            .is_some_and(|boundary| boundary.binary_search(&id).is_ok())
        {
            SmallVec::new()
        } else {
            commit.parent_ids().map(gix::Id::detach).collect()
        },
    });
    Ok(())
}

/// Return the commits reachable from `heads` in topological order, children first.
///
/// Once the walk reaches a commit in `heads_on_reaching`, the heads it maps to are walked too.
///
/// The ancestry of `excludes` is walked only as long as there is something interesting left:
/// commits not in it, `heads` not yet reached, or walked commits not known to be in it that it may
/// still reach. Like Git, the walk assumes that a commit of that ancestry may reach such a commit
/// if it's no older, unless it's known to be below one, which makes it an ancestor instead. The
/// commits of that ancestry that were walked are returned too, so the references on them stay
/// visible.
///
/// If `limit` is set, the walk stops after that many commits, the most recent first, except that
/// it still walks each head it hasn't reached yet, but none of their parents.
pub fn walk(
    repo: &gix::Repository,
    heads: impl IntoIterator<Item = gix::ObjectId>,
    heads_on_reaching: &HashMap<gix::ObjectId, Vec<gix::ObjectId>>,
    excludes: impl IntoIterator<Item = gix::ObjectId>,
    limit: Option<usize>,
    shallow_boundary: Option<&gix::shallow::Commits>,
) -> anyhow::Result<Vec<gix::ObjectId>> {
    // Invariants, unless the limit left an id out:
    //  - if an id appears in `excluded`, it appears in either `untraversed`
    //    or `id_to_traversed`
    //  - if an id appears in `excluded` and `id_to_traversed`, all its parents
    //    also appear in `excluded`
    //  - an id cannot appear in both `untraversed` and `id_to_traversed`
    //  - all parents of all traversed values appear either in `untraversed`
    //    or `id_to_traversed`
    let mut excluded = HashSet::<gix::ObjectId>::new();
    let mut untraversed = BinaryHeap::<Untraversed>::new();
    let mut id_to_traversed = HashMap::<gix::ObjectId, Traversed>::new();

    // Ids that appear in `untraversed`.
    let mut untraversed_ids = HashSet::<gix::ObjectId>::new();

    // Ids in `id_to_traversed` that do not have parents.
    let mut traversed_orphans = HashSet::<gix::ObjectId>::new();

    // The times and ids in `id_to_traversed` that do not appear in `excluded`, oldest first.
    let mut undecided = BTreeSet::<(i64, gix::ObjectId)>::new();

    // Ids known to be ancestors of commits that were in `undecided` when they were traversed.
    let mut below_undecided = HashSet::<gix::ObjectId>::new();

    let mut id_to_child_count = HashMap::<gix::ObjectId, u64>::new();

    let mut pushed_to_heads = HashSet::<gix::ObjectId>::new();
    let mut heads: VecDeque<gix::ObjectId> = {
        let mut new_heads = VecDeque::<gix::ObjectId>::new();
        for id in heads {
            if pushed_to_heads.insert(id) {
                new_heads.push_back(id);
                insert_untraversed(
                    repo,
                    id,
                    &mut untraversed,
                    &mut untraversed_ids,
                    shallow_boundary,
                )?;
            }
        }
        new_heads
    };
    let excludes: Vec<_> = excludes.into_iter().collect();
    for &id in &excludes {
        insert_untraversed(
            repo,
            id,
            &mut untraversed,
            &mut untraversed_ids,
            shallow_boundary,
        )?;
        excluded.insert(id);
    }

    while (traversed_orphans.iter().any(|id| !excluded.contains(id))
        || untraversed_ids.iter().any(|id| !excluded.contains(id))
        || heads.iter().any(|id| untraversed_ids.contains(id))
        || undecided.first().is_some_and(|(oldest_time_seconds, _)| {
            untraversed.iter().any(|next| {
                next.time_seconds >= *oldest_time_seconds && !below_undecided.contains(&next.id)
            })
        }))
        && let Some(Untraversed {
            time_seconds,
            id,
            parents_cache,
        }) = untraversed.pop()
    {
        untraversed_ids.remove(&id);
        // `pushed_to_heads` only holds the heads while walking.
        if limit.is_some_and(|limit| id_to_traversed.len() >= limit)
            && !pushed_to_heads.contains(&id)
        {
            continue;
        }
        if parents_cache.is_empty() {
            traversed_orphans.insert(id);
        }
        let is_below_undecided = !excluded.contains(&id) || below_undecided.contains(&id);
        for parent_id in &parents_cache {
            *id_to_child_count.entry(*parent_id).or_default() += 1;
            if !id_to_traversed.contains_key(parent_id) {
                insert_untraversed(
                    repo,
                    *parent_id,
                    &mut untraversed,
                    &mut untraversed_ids,
                    shallow_boundary,
                )?;
            }
            if excluded.contains(&id) {
                // Propagate to all known ancestors (untraversed or traversed)
                let mut ancestor_ids = vec![*parent_id];
                while let Some(ancestor_id) = ancestor_ids.pop() {
                    if excluded.insert(ancestor_id)
                        && let Some(traversed) = id_to_traversed.get(&ancestor_id)
                    {
                        undecided.remove(&(traversed.time_seconds, ancestor_id));
                        ancestor_ids.extend(traversed.parents.iter().cloned());
                    }
                }
            }
            if is_below_undecided {
                // Propagate to all known ancestors (untraversed or traversed)
                let mut ancestor_ids = vec![*parent_id];
                while let Some(ancestor_id) = ancestor_ids.pop() {
                    if below_undecided.insert(ancestor_id)
                        && let Some(traversed) = id_to_traversed.get(&ancestor_id)
                    {
                        ancestor_ids.extend(traversed.parents.iter().cloned());
                    }
                }
            }
        }
        if !excluded.contains(&id) {
            undecided.insert((time_seconds, id));
        }
        id_to_traversed.insert(
            id,
            Traversed {
                time_seconds,
                parents: parents_cache,
            },
        );
        for head_id in heads_on_reaching.get(&id).into_iter().flatten() {
            if pushed_to_heads.insert(*head_id) {
                heads.push_back(*head_id);
                if !id_to_traversed.contains_key(head_id) {
                    insert_untraversed(
                        repo,
                        *head_id,
                        &mut untraversed,
                        &mut untraversed_ids,
                        shallow_boundary,
                    )?;
                }
            }
        }
    }

    // An excluded commit no head reaches, like a target commit its branch was rewound past,
    // starts the outcome too, or the walked commits below it would never be returned.
    for id in excludes {
        if id_to_traversed.contains_key(&id)
            && id_to_child_count.get(&id).is_none_or(|count| *count == 0)
            && pushed_to_heads.insert(id)
        {
            heads.push_back(id);
        }
    }
    let mut outcome = Vec::new();
    while let Some(position) = heads.iter().position(|id| {
        id_to_child_count
            .get(id)
            .is_none_or(|child_count| *child_count == 0)
    }) {
        let id = heads
            .remove(position)
            .context("BUG: we just got position from position()")?;
        if let Some(Traversed { parents, .. }) = id_to_traversed.remove(&id) {
            for parent_id in parents.into_iter().rev() {
                *id_to_child_count.get_mut(&parent_id).context(
                    "BUG: traversed without corresponding entry in `id_to_child_count`",
                )? -= 1;
                if id_to_traversed.contains_key(&parent_id) && pushed_to_heads.insert(parent_id) {
                    heads.push_front(parent_id);
                }
            }
        }
        outcome.push(id);
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    fn scenario(name: &str) -> anyhow::Result<gix::Repository> {
        let root = but_testsupport::gix_testtools::scripted_fixture_read_only("scenarios.sh")
            .map_err(anyhow::Error::from_boxed)?;
        Ok(gix::open_opts(
            root.join(name),
            gix::open::Options::isolated(),
        )?)
    }

    fn summaries(repo: &gix::Repository, ids: Vec<gix::ObjectId>) -> anyhow::Result<Vec<String>> {
        ids.into_iter()
            .map(|id| Ok(repo.find_commit(id)?.message()?.summary().to_string()))
            .collect()
    }

    #[test]
    fn limit_stops_the_walk() -> anyhow::Result<()> {
        let repo = scenario("history-below-target")?;
        let head = repo.rev_parse_single("A")?.detach();
        let ids = super::walk(&repo, [head], &HashMap::new(), [], Some(2), None)?;
        assert_eq!(
            summaries(&repo, ids)?,
            ["A", "M"],
            "the walk ends after the two most recent commits"
        );
        Ok(())
    }

    #[test]
    fn limit_still_walks_each_head() -> anyhow::Result<()> {
        let repo = scenario("history-below-target")?;
        let heads = [
            repo.rev_parse_single("A")?.detach(),
            repo.rev_parse_single("A~3")?.detach(),
        ];
        let ids = super::walk(&repo, heads, &HashMap::new(), [], Some(1), None)?;
        assert_eq!(
            summaries(&repo, ids)?,
            ["A", "old-1"],
            "a head past the limit is walked, but not the commits between the heads"
        );
        Ok(())
    }

    #[test]
    fn excluded_history_is_walked_until_it_reaches_the_heads_merged_into_it() -> anyhow::Result<()>
    {
        let repo = scenario("branches-merged-into-target")?;
        let target = repo.rev_parse_single("main")?.detach();
        let heads = [
            target,
            repo.rev_parse_single("A")?.detach(),
            repo.rev_parse_single("B")?.detach(),
        ];
        let ids = super::walk(&repo, heads, &HashMap::new(), [target], None, None)?;
        assert_eq!(
            summaries(&repo, ids)?,
            ["upstream", "merge B", "merge A", "A", "B", "init"],
            "the walk goes on to \"merge A\", which is no older than A, to learn that A is merged too"
        );
        Ok(())
    }
}
