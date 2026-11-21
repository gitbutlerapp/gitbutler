use anyhow::Context;
use gix::{hashtable::hash_map::Entry, revwalk::graph::IdMap};
use std::collections::{BTreeSet, BinaryHeap, HashMap, VecDeque};

pub(crate) struct Walk {
    /// The commits the walk visited, children first.
    pub order: Vec<gix::ObjectId>,
    /// Includes the immediate parents at which the walk stopped.
    pub commits: Commits,
}

#[derive(Debug, Default)]
pub(crate) struct Commits {
    pub by_id: IdMap<Commit>,
    /// Parent lists in Git order, including the recorded parents of shallow commits.
    parent_ids: Vec<gix::ObjectId>,
}

impl Commits {
    pub fn parents(&self, commit: &Commit) -> &[gix::ObjectId] {
        &self.parent_ids[commit.parents.clone()]
    }

    pub fn parents_to_walk(&self, id: &gix::ObjectId) -> &[gix::ObjectId] {
        let commit = &self.by_id[id];
        if commit.is_shallow {
            &[]
        } else {
            self.parents(commit)
        }
    }
}

#[derive(Debug)]
pub(crate) struct Commit {
    time_seconds: i64,
    parents: std::ops::Range<usize>,
    pub is_workspace: bool,
    pub is_shallow: bool,
}

impl Commit {
    fn read(
        repo: &gix::Repository,
        id: gix::ObjectId,
        shallow_boundary: Option<&gix::shallow::Commits>,
        parent_ids: &mut Vec<gix::ObjectId>,
    ) -> anyhow::Result<Self> {
        use gix::objs::commit::ref_iter::Token;

        let commit = repo.find_commit(id)?;
        let mut tokens = commit.iter();
        let first_parent = parent_ids.len();
        let time_seconds = loop {
            match tokens
                .next()
                .transpose()?
                .with_context(|| format!("commit {id} is missing its committer"))?
            {
                Token::Parent { id } => parent_ids.push(id),
                Token::Committer { signature } => break signature.time()?.seconds,
                _ => {}
            }
        };
        Ok(Self {
            time_seconds,
            parents: first_parent..parent_ids.len(),
            is_workspace: crate::workspace::commit::is_managed_workspace_by_message(
                tokens.message()?,
            ),
            is_shallow: shallow_boundary
                .is_some_and(|boundary| boundary.binary_search(&id).is_ok()),
        })
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
    // Each discovered id has one visit record, including the parents where the walk stops.
    #[derive(Default)]
    struct Visit {
        children: usize,
        queued: bool,
        traversed: bool,
        excluded: bool,
        /// An ancestor of a commit that was undecided when traversed.
        below_undecided: bool,
        /// A requested head, or a parent queued during topological sorting.
        pushed_to_heads: bool,
    }
    impl Visit {
        fn push_to_heads(&mut self) -> bool {
            !std::mem::replace(&mut self.pushed_to_heads, true)
        }
    }
    let mut visits = IdMap::<Visit>::default();
    let mut untraversed = BinaryHeap::<(i64, gix::ObjectId)>::new();
    let mut num_traversed = 0;
    let mut commits = Commits::default();
    let mut traversed_orphans = Vec::new();
    // The times and ids of traversed commits outside the excluded ancestry, oldest first.
    let mut undecided = BTreeSet::<(i64, gix::ObjectId)>::new();
    let mut heads: VecDeque<_> = heads
        .into_iter()
        .filter(|id| visits.entry(*id).or_default().push_to_heads())
        .collect();
    let excludes: Vec<_> = excludes.into_iter().collect();
    for id in &excludes {
        visits.entry(*id).or_default().excluded = true;
    }
    let mut to_enqueue: Vec<_> = heads.iter().chain(&excludes).copied().collect();

    let mut ancestor_ids = Vec::new();
    loop {
        for id in to_enqueue.drain(..) {
            let visit = visits.entry(id).or_default();
            if !visit.queued {
                visit.queued = true;
                let commit = match commits.by_id.entry(id) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => entry.insert(Commit::read(
                        repo,
                        id,
                        shallow_boundary,
                        &mut commits.parent_ids,
                    )?),
                };
                untraversed.push((commit.time_seconds, id));
            }
        }

        let has_work = traversed_orphans.iter().any(|id| !visits[id].excluded)
            || untraversed.iter().any(|(_, id)| {
                let visit = &visits[id];
                !visit.excluded || visit.pushed_to_heads
            })
            || undecided.first().is_some_and(|(oldest_time_seconds, _)| {
                untraversed
                    .iter()
                    .any(|(time, id)| time >= oldest_time_seconds && !visits[id].below_undecided)
            });
        if !has_work {
            break;
        }
        let Some((time_seconds, id)) = untraversed.pop() else {
            break;
        };
        let visit = visits.get_mut(&id).expect("queued commit has a visit");
        visit.queued = false;
        if limit.is_some_and(|limit| num_traversed >= limit) && !visit.pushed_to_heads {
            continue;
        }
        let parents = commits.parents_to_walk(&id);
        if parents.is_empty() {
            traversed_orphans.push(id);
        }
        let is_excluded = visit.excluded;
        let is_below_undecided = !is_excluded || visit.below_undecided;
        visit.traversed = true;
        num_traversed += 1;
        for parent_id in parents {
            let parent_visit = visits.entry(*parent_id).or_default();
            parent_visit.children += 1;
            if !parent_visit.traversed {
                to_enqueue.push(*parent_id);
            }
            if is_excluded {
                // Propagate to all known ancestors (untraversed or traversed)
                ancestor_ids.push(*parent_id);
                while let Some(ancestor_id) = ancestor_ids.pop() {
                    let ancestor = visits.get_mut(&ancestor_id).expect("known ancestor");
                    if !std::mem::replace(&mut ancestor.excluded, true) && ancestor.traversed {
                        undecided.remove(&(commits.by_id[&ancestor_id].time_seconds, ancestor_id));
                        ancestor_ids.extend(commits.parents_to_walk(&ancestor_id));
                    }
                }
            }
            if is_below_undecided {
                // Propagate to all known ancestors (untraversed or traversed)
                ancestor_ids.push(*parent_id);
                while let Some(ancestor_id) = ancestor_ids.pop() {
                    let ancestor = visits.get_mut(&ancestor_id).expect("known ancestor");
                    if !std::mem::replace(&mut ancestor.below_undecided, true) && ancestor.traversed
                    {
                        ancestor_ids.extend(commits.parents_to_walk(&ancestor_id));
                    }
                }
            }
        }
        if !is_excluded {
            undecided.insert((time_seconds, id));
        }
        for head_id in heads_on_reaching.get(&id).into_iter().flatten() {
            let head = visits.entry(*head_id).or_default();
            if head.push_to_heads() {
                heads.push_back(*head_id);
                if !head.traversed {
                    to_enqueue.push(*head_id);
                }
            }
        }
    }

    // An excluded commit no head reaches, like a target commit its branch was rewound past,
    // starts the outcome too, or the walked commits below it would never be returned.
    for id in excludes {
        let visit = visits.get_mut(&id).expect("excluded id has a visit");
        if visit.traversed && visit.children == 0 && visit.push_to_heads() {
            heads.push_back(id);
        }
    }
    let mut order = Vec::with_capacity(num_traversed);
    while let Some(position) = heads.iter().position(|id| visits[id].children == 0) {
        let id = heads
            .remove(position)
            .context("BUG: we just got position from position()")?;
        anyhow::ensure!(
            visits
                .get_mut(&id)
                .is_some_and(|visit| std::mem::take(&mut visit.traversed)),
            "BUG: each head and queued parent was traversed"
        );
        for parent_id in commits.parents_to_walk(&id).iter().rev() {
            let parent = visits
                .get_mut(parent_id)
                .context("BUG: parent of traversed commit missing")?;
            parent.children -= 1;
            if parent.traversed && parent.push_to_heads() {
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
    fn reads_octopus_parents_and_workspace_message_after_extra_headers() -> anyhow::Result<()> {
        let repo = scenario("history-below-target")?.with_object_memory();
        let mut commit = repo.head_commit()?.decode()?.to_owned()?;
        commit.parents = ["A~2", "A", "A~1"]
            .into_iter()
            .map(|rev| Ok(repo.rev_parse_single(rev)?.detach()))
            .collect::<anyhow::Result<_>>()?;
        commit.committer.time.seconds += 60;
        commit
            .extra_headers
            .push(("gpgsig".into(), "first line\nsecond line".into()));
        commit.message = "GitButler Workspace Commit\n\nbody".into();
        let id = repo.write_object(&commit)?.detach();
        let mut commits = super::Commits::default();
        let read = super::Commit::read(&repo, id, None, &mut commits.parent_ids)?;
        assert_eq!(
            commits.parents(&read),
            commit.parents.as_slice(),
            "all parents are kept in Git order"
        );
        assert_eq!(
            read.time_seconds, commit.committer.time.seconds,
            "the walk orders by committer time"
        );
        assert!(
            read.is_workspace,
            "the message follows the whole multiline header"
        );
        Ok(())
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
    fn a_parent_skipped_at_the_limit_can_become_a_head_later() -> anyhow::Result<()> {
        let repo = scenario("history-below-target")?.with_object_memory();
        let head = repo.head_commit()?;
        let parent = head.parent_ids().next().unwrap().detach();
        let mut trigger = head.decode()?.to_owned()?;
        trigger.parents.clear();
        trigger.committer.time.seconds -= 60;
        trigger.message = "trigger".into();
        let trigger = repo.write_object(&trigger)?.detach();
        let heads_on_reaching = HashMap::from([(trigger, vec![parent, parent])]);
        let walk = super::walk(
            &repo,
            [head.id, trigger],
            &heads_on_reaching,
            [],
            Some(1),
            None,
        )?;
        assert_eq!(summaries(&repo, walk)?, ["A", "trigger", "M"]);
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
