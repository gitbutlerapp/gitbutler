use anyhow::Context;
use gix::{hashtable::hash_map::Entry, revwalk::graph::IdMap};
use smallvec::SmallVec;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque};

pub(crate) struct Walk {
    /// The commits the walk visited, children first.
    pub order: Vec<gix::ObjectId>,
    /// Includes the immediate parents at which the walk stopped.
    pub commits: IdMap<Commit>,
}

#[derive(Debug)]
pub(crate) struct Commit {
    time_seconds: i64,
    pub parent_ids: SmallVec<[gix::ObjectId; 1]>,
    pub is_workspace: bool,
    pub is_shallow: bool,
}

impl Commit {
    fn read(
        repo: &gix::Repository,
        id: gix::ObjectId,
        shallow_boundary: Option<&gix::shallow::Commits>,
    ) -> anyhow::Result<Self> {
        let commit = repo.find_commit(id)?;
        Ok(Self {
            time_seconds: commit.time()?.seconds,
            parent_ids: commit.parent_ids().map(gix::Id::detach).collect(),
            is_workspace: crate::workspace::commit::is_managed_workspace_by_message(
                commit.message_raw()?,
            ),
            is_shallow: shallow_boundary
                .is_some_and(|boundary| boundary.binary_search(&id).is_ok()),
        })
    }

    /// A shallow clone records its boundary commits' parents even though it can't walk them.
    pub fn parents_to_walk(&self) -> &[gix::ObjectId] {
        if self.is_shallow {
            &[]
        } else {
            &self.parent_ids
        }
    }
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
) -> anyhow::Result<Walk> {
    // Invariants, unless the limit left an id out:
    //  - if an id appears in `excluded`, it appears in either `untraversed`
    //    or `traversed`
    //  - if an id appears in `excluded` and `traversed`, all its parents
    //    also appear in `excluded`
    //  - an id cannot appear in both `untraversed` and `traversed`
    //  - all parents of all traversed values appear either in `untraversed`
    //    or `traversed`
    let mut untraversed = BinaryHeap::<(i64, gix::ObjectId)>::new();
    let mut traversed = HashSet::<gix::ObjectId>::new();
    let mut commits = IdMap::<Commit>::default();

    // Ids that appear in `untraversed`.
    let mut untraversed_ids = HashSet::<gix::ObjectId>::new();

    // Ids in `traversed` that do not have parents.
    let mut traversed_orphans = HashSet::<gix::ObjectId>::new();

    // The times and ids in `traversed` that do not appear in `excluded`, oldest first.
    let mut undecided = BTreeSet::<(i64, gix::ObjectId)>::new();

    // Ids known to be ancestors of commits that were in `undecided` when they were traversed.
    let mut below_undecided = HashSet::<gix::ObjectId>::new();

    let mut id_to_child_count = HashMap::<gix::ObjectId, u64>::new();

    let mut pushed_to_heads = HashSet::<gix::ObjectId>::new();
    let mut heads: VecDeque<_> = heads
        .into_iter()
        .filter(|id| pushed_to_heads.insert(*id))
        .collect();
    let excludes: Vec<_> = excludes.into_iter().collect();
    let mut excluded: HashSet<_> = excludes.iter().copied().collect();
    let mut to_enqueue: Vec<_> = heads.iter().chain(&excludes).copied().collect();

    loop {
        for id in to_enqueue.drain(..) {
            if untraversed_ids.insert(id) {
                let commit = match commits.entry(id) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => entry.insert(Commit::read(repo, id, shallow_boundary)?),
                };
                untraversed.push((commit.time_seconds, id));
            }
        }

        let has_work = traversed_orphans.iter().any(|id| !excluded.contains(id))
            || untraversed_ids.iter().any(|id| !excluded.contains(id))
            || heads.iter().any(|id| untraversed_ids.contains(id))
            || undecided.first().is_some_and(|(oldest_time_seconds, _)| {
                untraversed
                    .iter()
                    .any(|(time, id)| time >= oldest_time_seconds && !below_undecided.contains(id))
            });
        if !has_work {
            break;
        }
        let Some((time_seconds, id)) = untraversed.pop() else {
            break;
        };
        untraversed_ids.remove(&id);
        // `pushed_to_heads` only holds the heads while walking.
        if limit.is_some_and(|limit| traversed.len() >= limit) && !pushed_to_heads.contains(&id) {
            continue;
        }
        let parents = commits[&id].parents_to_walk();
        if parents.is_empty() {
            traversed_orphans.insert(id);
        }
        let is_below_undecided = !excluded.contains(&id) || below_undecided.contains(&id);
        for parent_id in parents {
            *id_to_child_count.entry(*parent_id).or_default() += 1;
            if !traversed.contains(parent_id) {
                to_enqueue.push(*parent_id);
            }
            if excluded.contains(&id) {
                // Propagate to all known ancestors (untraversed or traversed)
                let mut ancestor_ids = vec![*parent_id];
                while let Some(ancestor_id) = ancestor_ids.pop() {
                    if excluded.insert(ancestor_id) && traversed.contains(&ancestor_id) {
                        undecided.remove(&(commits[&ancestor_id].time_seconds, ancestor_id));
                        ancestor_ids.extend(commits[&ancestor_id].parents_to_walk());
                    }
                }
            }
            if is_below_undecided {
                // Propagate to all known ancestors (untraversed or traversed)
                let mut ancestor_ids = vec![*parent_id];
                while let Some(ancestor_id) = ancestor_ids.pop() {
                    if below_undecided.insert(ancestor_id) && traversed.contains(&ancestor_id) {
                        ancestor_ids.extend(commits[&ancestor_id].parents_to_walk());
                    }
                }
            }
        }
        if !excluded.contains(&id) {
            undecided.insert((time_seconds, id));
        }
        traversed.insert(id);
        for head_id in heads_on_reaching.get(&id).into_iter().flatten() {
            if pushed_to_heads.insert(*head_id) {
                heads.push_back(*head_id);
                if !traversed.contains(head_id) {
                    to_enqueue.push(*head_id);
                }
            }
        }
    }

    // An excluded commit no head reaches, like a target commit its branch was rewound past,
    // starts the outcome too, or the walked commits below it would never be returned.
    for id in excludes {
        if traversed.contains(&id)
            && id_to_child_count.get(&id).is_none_or(|count| *count == 0)
            && pushed_to_heads.insert(id)
        {
            heads.push_back(id);
        }
    }
    let mut order = Vec::with_capacity(traversed.len());
    while let Some(position) = heads.iter().position(|id| {
        id_to_child_count
            .get(id)
            .is_none_or(|child_count| *child_count == 0)
    }) {
        let id = heads
            .remove(position)
            .context("BUG: we just got position from position()")?;
        anyhow::ensure!(
            traversed.remove(&id),
            "BUG: each head and queued parent was traversed"
        );
        for parent_id in commits[&id].parents_to_walk().iter().rev() {
            *id_to_child_count
                .get_mut(parent_id)
                .context("BUG: traversed without corresponding entry in `id_to_child_count`")? -= 1;
            if traversed.contains(parent_id) && pushed_to_heads.insert(*parent_id) {
                heads.push_front(*parent_id);
            }
        }
        order.push(id);
    }
    Ok(Walk { order, commits })
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

    fn summaries(repo: &gix::Repository, walk: super::Walk) -> anyhow::Result<Vec<String>> {
        walk.order
            .into_iter()
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
