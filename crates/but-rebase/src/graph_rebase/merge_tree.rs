use anyhow::{Context as _, Result};
use gix::revwalk::{PriorityQueue, graph};

pub(crate) struct SharedHistory {
    commits: Vec<HistoryCommit>,
    tips: Vec<usize>,
    floor: Option<usize>,
}

struct HistoryCommit {
    id: gix::ObjectId,
    parents: Vec<usize>,
}

impl SharedHistory {
    pub(crate) fn walk(repo: &gix::Repository, tips: &[gix::ObjectId]) -> Result<Self> {
        let cache = repo.commit_graph_if_enabled()?;
        let mut walk = Walk {
            graph: repo.revision_graph(cache.as_ref()),
            index_of: gix::hashtable::HashMap::default(),
            queue: PriorityQueue::new(),
            commits: Vec::new(),
        };
        let tips = tips
            .iter()
            .map(|tip| walk.visit(*tip))
            .collect::<Result<Vec<_>>>()?;
        while walk.queue.len() > 1 {
            let index = walk.queue.pop_value().expect("more than one queued");
            walk.expand(index)?;
        }
        Ok(SharedHistory {
            floor: walk.queue.pop_value(),
            commits: walk.commits,
            tips,
        })
    }

    pub(crate) fn ranks(&self) -> Vec<Rank> {
        let mut ranks: Vec<Option<Rank>> = vec![None; self.commits.len()];
        for start in 0..self.commits.len() {
            let mut stack = vec![start];
            while let Some(&index) = stack.last() {
                if ranks[index].is_some() {
                    stack.pop();
                    continue;
                }
                let parents = &self.commits[index].parents;
                let pending: Vec<_> = parents
                    .iter()
                    .copied()
                    .filter(|parent| ranks[*parent].is_none())
                    .collect();
                if !pending.is_empty() {
                    stack.extend(pending);
                    continue;
                }
                let parent_ranks: Vec<Rank> = parents
                    .iter()
                    .map(|parent| ranks[*parent].expect("parents come first"))
                    .collect();
                ranks[index] = Some(Rank {
                    reaches_floor: self.floor == Some(index)
                        || parent_ranks.iter().any(|rank| rank.reaches_floor),
                    generation: 1 + parent_ranks
                        .iter()
                        .map(|rank| rank.generation)
                        .max()
                        .unwrap_or(0),
                });
                stack.pop();
            }
        }
        ranks
            .into_iter()
            .map(|rank| rank.expect("every commit was reached"))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Rank {
    reaches_floor: bool,
    generation: usize,
}

struct Walk<'repo, 'cache> {
    graph: gix::revwalk::Graph<'repo, 'cache, graph::Commit<()>>,
    index_of: gix::hashtable::HashMap<gix::ObjectId, usize>,
    queue: PriorityQueue<(graph::Generation, gix::date::SecondsSinceUnixEpoch), usize>,
    commits: Vec<HistoryCommit>,
}

impl Walk<'_, '_> {
    fn visit(&mut self, id: gix::ObjectId) -> Result<usize> {
        if let Some(index) = self.index_of.get(&id) {
            return Ok(*index);
        }
        let commit = self.lookup(id)?;
        let key = (
            commit
                .generation
                .unwrap_or(gix::commitgraph::GENERATION_NUMBER_INFINITY),
            commit.commit_time,
        );
        let index = self.commits.len();
        self.commits.push(HistoryCommit {
            id,
            parents: Vec::new(),
        });
        self.index_of.insert(id, index);
        self.queue.insert(key, index);
        Ok(index)
    }

    fn expand(&mut self, index: usize) -> Result<()> {
        let parents = self.lookup(self.commits[index].id)?.parents.clone();
        for parent in parents {
            let parent = self.visit(parent)?;
            self.commits[index].parents.push(parent);
        }
        Ok(())
    }

    fn lookup(&mut self, id: gix::ObjectId) -> Result<&graph::Commit<()>> {
        Ok(self
            .graph
            .get_or_insert_full_commit(id, |_| {})?
            .with_context(|| format!("Commit {id} is missing from the repository"))?)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use anyhow::Result;

    use super::SharedHistory;

    struct Scenario {
        repo: gix::Repository,
        names: BTreeMap<gix::ObjectId, String>,
    }

    impl Scenario {
        fn open(name: &str) -> Result<Self> {
            let repo = but_testsupport::read_only_in_memory_scenario_named("ordered-merges", name)?;
            let mut names = BTreeMap::new();
            for reference in repo.references()?.tags()? {
                let mut reference = reference?;
                let id = reference.peel_to_commit()?.id;
                names.insert(id, reference.name().shorten().to_string());
            }
            Ok(Scenario { repo, names })
        }

        fn ids(&self, names: &[&str]) -> Result<Vec<gix::ObjectId>> {
            names
                .iter()
                .map(|name| Ok(self.repo.rev_parse_single(*name)?.detach()))
                .collect()
        }

        fn name(&self, id: gix::ObjectId) -> &str {
            &self.names[&id]
        }

        fn shared_history(&self, tips: &[&str]) -> Result<String> {
            let history = SharedHistory::walk(&self.repo, &self.ids(tips)?)?;
            let mut lines: Vec<_> = history
                .commits
                .iter()
                .map(|commit| {
                    let parents: Vec<_> = commit
                        .parents
                        .iter()
                        .map(|parent| self.name(history.commits[*parent].id))
                        .collect();
                    format!("{} -> {}", self.name(commit.id), parents.join(" "))
                })
                .collect();
            lines.sort();
            let floor = history
                .floor
                .map_or("none", |floor| self.name(history.commits[floor].id));
            let tips: Vec<_> = history
                .tips
                .iter()
                .map(|tip| self.name(history.commits[*tip].id))
                .collect();
            Ok(format!(
                "{}\ntips: {}\nfloor: {floor}\n",
                lines.join("\n"),
                tips.join(" ")
            ))
        }
        fn ranks(&self, tips: &[&str]) -> Result<String> {
            let history = SharedHistory::walk(&self.repo, &self.ids(tips)?)?;
            let ranks = history.ranks();
            let mut lines: Vec<_> = history
                .commits
                .iter()
                .zip(&ranks)
                .map(|(commit, rank)| (std::cmp::Reverse(*rank), self.name(commit.id)))
                .collect();
            lines.sort();
            Ok(lines
                .into_iter()
                .map(|(std::cmp::Reverse(rank), name)| {
                    let floor = if rank.reaches_floor { "floor" } else { "     " };
                    format!("{name:>2} {floor} {}\n", rank.generation)
                })
                .collect())
        }
    }

    #[test]
    fn shared_history_ends_at_the_last_commit_left_to_walk() -> Result<()> {
        let nested = Scenario::open("nested")?;
        // p1 and p3 meet at c2, which still leaves p2's line open until c1.
        snapbox::assert_data_eq!(
            nested.shared_history(&["p1", "p2", "p3"])?,
            snapbox::str![[r#"
c1 -> 
c2 -> c1
p1 -> c2
p2 -> c1
p3 -> c2
tips: p1 p2 p3
floor: c1

"#]]
        );
        // A tip inside another tip's history is visited like any other commit.
        snapbox::assert_data_eq!(
            nested.shared_history(&["p1", "p2", "c2"])?,
            snapbox::str![[r#"
c1 -> 
c2 -> c1
p1 -> c2
p2 -> c1
tips: p1 p2 c2
floor: c1

"#]]
        );
        // One tip, or the same tip twice, is its own floor.
        snapbox::assert_data_eq!(
            nested.shared_history(&["p1", "p1"])?,
            snapbox::str![[r#"
p1 -> 
tips: p1 p1
floor: p1

"#]]
        );

        let five_leaves = Scenario::open("five-leaves")?;
        // Both forks are walked, down to the commit they share.
        snapbox::assert_data_eq!(
            five_leaves.shared_history(&["c1", "c4", "c2", "c5", "c3"])?,
            snapbox::str![[r#"
A -> Z
B -> Z
Z -> 
c1 -> A
c2 -> A
c3 -> A
c4 -> B
c5 -> B
tips: c1 c4 c2 c5 c3
floor: Z

"#]]
        );

        let criss_cross = Scenario::open("criss-cross")?;
        // X and Y are both best merge-bases of a and b, so the walk goes on to Z.
        snapbox::assert_data_eq!(
            criss_cross.shared_history(&["a", "b"])?,
            snapbox::str![[r#"
X -> X1
X1 -> Z
Y -> Z
Z -> 
a -> Y X
b -> X Y
tips: a b
floor: Z

"#]]
        );

        let unrelated = Scenario::open("unrelated")?;
        // Unrelated histories never meet, so the floor is just the last root left, which r2 doesn't reach.
        snapbox::assert_data_eq!(
            unrelated.shared_history(&["r1", "r2"])?,
            snapbox::str![[r#"
r1 -> 
r2 -> 
tips: r1 r2
floor: r1

"#]]
        );

        let merged_root = Scenario::open("merged-root")?;
        // The subtree's line only ends at its root, but the walk still finds m2.
        snapbox::assert_data_eq!(
            merged_root.shared_history(&["a", "b"])?,
            snapbox::str![[r#"
a -> m2 s2
b -> m2
m2 -> 
s1 -> 
s2 -> s1
tips: a b
floor: m2

"#]]
        );
        Ok(())
    }

    #[test]
    fn ranks_are_generations_in_the_shared_history() -> Result<()> {
        // X is deeper than Y, so it ranks above it though neither contains the other.
        snapbox::assert_data_eq!(
            Scenario::open("criss-cross")?.ranks(&["a", "b"])?,
            snapbox::str![[r#"
 a floor 4
 b floor 4
 X floor 3
X1 floor 2
 Y floor 2
 Z floor 1

"#]]
        );
        // The subtree's line never reaches m2, so it ranks below everything that does.
        snapbox::assert_data_eq!(
            Scenario::open("merged-root")?.ranks(&["a", "b"])?,
            snapbox::str![[r#"
 a floor 3
 b floor 2
m2 floor 1
s2       2
s1       1

"#]]
        );
        Ok(())
    }
}
