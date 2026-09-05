use anyhow::Context;
use smallvec::SmallVec;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

#[derive(Eq, PartialEq, Ord, PartialOrd)]
struct Untraversed {
    // The latest untraversed commit will be popped from the BinaryHeap.
    time_seconds: i64,
    id: gix::ObjectId,
    parents_cache: SmallVec<[gix::ObjectId; 1]>,
}
struct Traversed {
    parents: SmallVec<[gix::ObjectId; 1]>,
}

fn insert_untraversed(
    repo: &gix::Repository,
    id: gix::ObjectId,
    untraversed: &mut BinaryHeap<Untraversed>,
    untraversed_ids: &mut HashSet<gix::ObjectId>,
) -> anyhow::Result<()> {
    if !untraversed_ids.insert(id) {
        return Ok(());
    }
    let commit = repo.find_commit(id)?;
    untraversed.push(Untraversed {
        time_seconds: commit.time()?.seconds,
        id,
        parents_cache: commit.parent_ids().map(gix::Id::detach).collect(),
    });
    Ok(())
}

pub fn walk(
    repo: &gix::Repository,
    heads: impl IntoIterator<Item = gix::ObjectId>,
    excludes: impl IntoIterator<Item = gix::ObjectId>,
) -> anyhow::Result<Vec<gix::ObjectId>> {
    // Invariants:
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

    let mut id_to_child_count = HashMap::<gix::ObjectId, u64>::new();

    let mut pushed_to_heads = HashSet::<gix::ObjectId>::new();
    let mut heads: VecDeque<gix::ObjectId> = {
        let mut new_heads = VecDeque::<gix::ObjectId>::new();
        for id in heads {
            if pushed_to_heads.insert(id) {
                new_heads.push_back(id);
                insert_untraversed(repo, id, &mut untraversed, &mut untraversed_ids)?;
            }
        }
        new_heads
    };
    for id in excludes.into_iter() {
        insert_untraversed(repo, id, &mut untraversed, &mut untraversed_ids)?;
        excluded.insert(id);
    }

    while (traversed_orphans.iter().any(|id| !excluded.contains(id))
        || untraversed_ids.iter().any(|id| !excluded.contains(id)))
        && let Some(Untraversed {
            time_seconds: _,
            id,
            parents_cache,
        }) = untraversed.pop()
    {
        untraversed_ids.remove(&id);
        if parents_cache.is_empty() {
            traversed_orphans.insert(id);
        }
        for parent_id in &parents_cache {
            *id_to_child_count.entry(*parent_id).or_default() += 1;
            if !id_to_traversed.contains_key(parent_id) {
                insert_untraversed(repo, *parent_id, &mut untraversed, &mut untraversed_ids)?;
            }
            if excluded.contains(&id) {
                // Propagate to all known ancestors (untraversed or traversed)
                let mut ancestor_ids = vec![*parent_id];
                while let Some(ancestor_id) = ancestor_ids.pop() {
                    if excluded.insert(ancestor_id) {
                        if let Some(traversed) = id_to_traversed.get(&ancestor_id) {
                            ancestor_ids.extend(traversed.parents.iter().cloned());
                        }
                    }
                }
            }
        }
        id_to_traversed.insert(
            id,
            Traversed {
                parents: parents_cache,
            },
        );
    }

    let mut outcome = Vec::new();
    heads.retain(|id| !excluded.contains(id));
    while let Some(position) = heads.iter().position(|id| {
        id_to_child_count
            .get(id)
            .is_none_or(|child_count| *child_count == 0)
    }) {
        let id = heads
            .remove(position)
            .context("BUG: we just got position from position()")?;
        if let Some(Traversed { parents }) = id_to_traversed.remove(&id) {
            for parent_id in parents.into_iter().rev() {
                *id_to_child_count.get_mut(&parent_id).context(
                    "BUG: traversed without corresponding entry in `id_to_child_count`",
                )? -= 1;
                if !excluded.contains(&parent_id) {
                    if pushed_to_heads.insert(parent_id) {
                        heads.push_front(parent_id);
                    }
                }
            }
        }
        outcome.push(id);
    }
    Ok(outcome)
}
