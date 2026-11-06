use anyhow::{Context as _, Result};
use but_core::RepositoryExt as _;
use gix::revwalk::{PriorityQueue, graph};

use super::cherry_pick::peel_to_tree_or_empty;

pub(crate) struct SharedHistory {
    commits: Vec<HistoryCommit>,
    tips: Vec<usize>,
    floor: Option<usize>,
}

struct HistoryCommit {
    id: gix::ObjectId,
    parents: Vec<usize>,
    expanded: bool,
    reached_by: Bits,
}

impl SharedHistory {
    pub(crate) fn walk(repo: &gix::Repository, tips: &[gix::ObjectId]) -> Result<Self> {
        let cache = repo.commit_graph_if_enabled()?;
        let mut walk = Walk {
            graph: repo.revision_graph(cache.as_ref()),
            shallow: repo.shallow_commits()?,
            index_of: gix::hashtable::HashMap::default(),
            queue: PriorityQueue::new(),
            commits: Vec::new(),
        };
        let mut every_tip = Bits::default();
        let tips = tips
            .iter()
            .enumerate()
            .map(|(tip, id)| {
                every_tip.insert(tip);
                let index = walk.visit(*id)?;
                walk.commits[index].reached_by.insert(tip);
                Ok(index)
            })
            .collect::<Result<Vec<_>>>()?;
        while let Some(index) = walk.next_to_expand(&every_tip) {
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

#[derive(Debug, Clone, Default, PartialEq)]
struct Bits(Vec<u64>);

impl Bits {
    fn insert(&mut self, index: usize) {
        let word = index / 64;
        if self.0.len() <= word {
            self.0.resize(word + 1, 0);
        }
        self.0[word] |= 1 << (index % 64);
    }

    fn union(&mut self, other: &Bits) -> bool {
        if self.0.len() < other.0.len() {
            self.0.resize(other.0.len(), 0);
        }
        let mut grew = false;
        for (word, other_word) in self.0.iter_mut().zip(&other.0) {
            grew |= *other_word & !*word != 0;
            *word |= other_word;
        }
        grew
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Rank {
    reaches_floor: bool,
    generation: usize,
}

struct Walk<'repo, 'cache> {
    graph: gix::revwalk::Graph<'repo, 'cache, graph::Commit<()>>,
    shallow: Option<gix::shallow::Commits>,
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
            expanded: false,
            reached_by: Bits::default(),
        });
        self.index_of.insert(id, index);
        self.queue.insert(key, index);
        Ok(index)
    }

    fn next_to_expand(&mut self, every_tip: &Bits) -> Option<usize> {
        let floor = match self.queue.peek() {
            Some((_, last)) if self.queue.len() == 1 => *last,
            _ => return self.queue.pop_value(),
        };
        if self.commits[floor].reached_by == *every_tip {
            None
        } else {
            self.queue.pop_value()
        }
    }

    fn expand(&mut self, index: usize) -> Result<()> {
        self.commits[index].expanded = true;
        let id = self.commits[index].id;
        if self.is_shallow_boundary(id) {
            return Ok(());
        }
        let parents = self.lookup(id)?.parents.clone();
        for parent in parents {
            let parent = self.visit(parent)?;
            self.commits[index].parents.push(parent);
            self.spread(index, parent);
        }
        Ok(())
    }

    fn spread(&mut self, child: usize, parent: usize) {
        let mut edges = vec![(child, parent)];
        while let Some((child, parent)) = edges.pop() {
            let reached_by = self.commits[child].reached_by.clone();
            let parent_commit = &mut self.commits[parent];
            if parent_commit.reached_by.union(&reached_by) && parent_commit.expanded {
                edges.extend(
                    parent_commit
                        .parents
                        .iter()
                        .map(|grandparent| (parent, *grandparent)),
                );
            }
        }
    }

    fn is_shallow_boundary(&self, id: gix::ObjectId) -> bool {
        self.shallow
            .as_ref()
            .is_some_and(|boundary| boundary.binary_search(&id).is_ok())
    }

    fn lookup(&mut self, id: gix::ObjectId) -> Result<&graph::Commit<()>> {
        Ok(self
            .graph
            .get_or_insert_full_commit(id, |_| {})?
            .with_context(|| format!("Commit {id} is missing from the repository"))?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Tip {
    pub(crate) position: usize,
    pub(crate) id: gix::ObjectId,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MergeTree {
    Tip(Tip),
    Fork {
        base: Option<gix::ObjectId>,
        branches: Vec<MergeTree>,
    },
}

impl MergeTree {
    pub(crate) fn new(repo: &gix::Repository, tips: &[gix::ObjectId]) -> Result<Option<Self>> {
        if tips.is_empty() {
            return Ok(None);
        }
        let history = SharedHistory::walk(repo, tips)?;
        let ranks = history.ranks();
        let mut groups = Groups(
            tips.iter()
                .enumerate()
                .map(|(first_tip, id)| Group {
                    tree: Some(MergeTree::Tip(Tip {
                        position: first_tip,
                        id: *id,
                    })),
                    first_tip,
                    joined: None,
                })
                .collect(),
        );
        let mut arrived = vec![Vec::new(); history.commits.len()];
        for (group, tip) in history.tips.iter().enumerate() {
            arrived[*tip].push(group);
        }

        let mut order: Vec<usize> = (0..history.commits.len()).collect();
        order.sort_by_key(|index| std::cmp::Reverse(ranks[*index]));
        for equal_rank in order.chunk_by(|a, b| ranks[*a] == ranks[*b]) {
            let mut equal_rank = equal_rank.to_vec();
            equal_rank.sort_by_cached_key(|index| {
                let first_tip = groups
                    .roots(&arrived[*index])
                    .first()
                    .map(|root| groups.0[*root].first_tip);
                (first_tip, history.commits[*index].id)
            });
            for index in equal_rank {
                if groups.live() == 1 {
                    break;
                }
                let roots = groups.roots(&arrived[index]);
                let group = match roots.as_slice() {
                    [] => continue,
                    [root] => *root,
                    roots => groups.join(Some(history.commits[index].id), roots),
                };
                for parent in &history.commits[index].parents {
                    arrived[*parent].push(group);
                }
            }
        }

        let roots = groups.roots(&(0..tips.len()).collect::<Vec<_>>());
        let root = match roots.as_slice() {
            [root] => *root,
            roots => groups.join(None, roots),
        };
        Ok(groups.0[root].tree.take())
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "no caller asks yet"))]
    pub(crate) fn conflicts_with(
        &self,
        repo: &gix::Repository,
        tip: Tip,
        options: &gix::merge::tree::Options,
        conflicts: gix::merge::tree::TreatAsUnresolved,
    ) -> Result<Vec<Tip>> {
        let ours = peel_to_tree_or_empty(repo, Some(tip.id))?;
        let mut conflicting = Vec::new();
        for meeting in self.meetings(tip) {
            let output = repo.merge_trees(
                peel_to_tree_or_empty(repo, meeting.base)?,
                ours,
                peel_to_tree_or_empty(repo, Some(meeting.tip.id))?,
                repo.default_merge_labels(),
                options.clone(),
            )?;
            if output.has_unresolved_conflicts(conflicts) {
                conflicting.push(meeting.tip);
            }
        }
        Ok(conflicting)
    }

    fn meetings(&self, tip: Tip) -> Vec<Meeting> {
        let MergeTree::Fork { base, branches } = self else {
            return Vec::new();
        };
        let Some(own) = branches
            .iter()
            .position(|branch| branch.tips().contains(&tip))
        else {
            return Vec::new();
        };
        let mut meetings = branches[own].meetings(tip);
        for (index, branch) in branches.iter().enumerate() {
            if index != own {
                meetings.extend(
                    branch
                        .tips()
                        .into_iter()
                        .map(|tip| Meeting { tip, base: *base }),
                );
            }
        }
        meetings.sort_by_key(|meeting| meeting.tip.position);
        meetings
    }

    fn tips(&self) -> Vec<Tip> {
        match self {
            MergeTree::Tip(tip) => vec![*tip],
            MergeTree::Fork { branches, .. } => branches.iter().flat_map(MergeTree::tips).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Meeting {
    tip: Tip,
    base: Option<gix::ObjectId>,
}

struct Group {
    tree: Option<MergeTree>,
    first_tip: usize,
    joined: Option<usize>,
}

struct Groups(Vec<Group>);

impl Groups {
    fn root(&self, mut group: usize) -> usize {
        while let Some(joined) = self.0[group].joined {
            group = joined;
        }
        group
    }

    fn roots(&self, groups: &[usize]) -> Vec<usize> {
        let mut roots: Vec<_> = groups.iter().map(|group| self.root(*group)).collect();
        roots.sort_by_key(|root| self.0[*root].first_tip);
        roots.dedup();
        roots
    }

    fn live(&self) -> usize {
        self.0.iter().filter(|group| group.tree.is_some()).count()
    }

    fn join(&mut self, base: Option<gix::ObjectId>, roots: &[usize]) -> usize {
        let joined = self.0.len();
        let branches = roots
            .iter()
            .map(|root| {
                self.0[*root].joined = Some(joined);
                self.0[*root].tree.take().expect("roots are live")
            })
            .collect();
        self.0.push(Group {
            tree: Some(MergeTree::Fork { base, branches }),
            first_tip: self.0[roots[0]].first_tip,
            joined: None,
        });
        joined
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use anyhow::Result;
    use but_core::RepositoryExt as _;

    use super::{MergeTree, SharedHistory, Tip};

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
        fn merge_tree(&self, tips: &[&str]) -> Result<String> {
            let tree = MergeTree::new(&self.repo, &self.ids(tips)?)?.expect("tips were given");
            Ok(self.render(&tree))
        }

        fn meetings(&self, tips: &[&str], position: usize) -> Result<String> {
            let ids = self.ids(tips)?;
            let tree = MergeTree::new(&self.repo, &ids)?.expect("tips were given");
            let tip = Tip {
                position,
                id: ids[position],
            };
            Ok(tree
                .meetings(tip)
                .iter()
                .map(|meeting| {
                    format!(
                        "{}:{}@{}",
                        self.name(meeting.tip.id),
                        meeting.tip.position,
                        meeting.base.map_or("none", |base| self.name(base))
                    )
                })
                .collect::<Vec<_>>()
                .join(" "))
        }

        fn conflicts_with(&self, tips: &[&str], position: usize) -> Result<String> {
            let ids = self.ids(tips)?;
            let tree = MergeTree::new(&self.repo, &ids)?.expect("tips were given");
            let tip = Tip {
                position,
                id: ids[position],
            };
            let (options, conflicts) = self.repo.merge_options_fail_fast()?;
            Ok(tree
                .conflicts_with(&self.repo, tip, &options, conflicts)?
                .iter()
                .map(|tip| format!("{}:{}", self.name(tip.id), tip.position))
                .collect::<Vec<_>>()
                .join(" "))
        }

        fn render(&self, tree: &MergeTree) -> String {
            match tree {
                MergeTree::Tip(tip) => format!("{}:{}", self.name(tip.id), tip.position),
                MergeTree::Fork { base, branches } => format!(
                    "fork {} [{}]",
                    base.map_or("none", |base| self.name(base)),
                    branches
                        .iter()
                        .map(|branch| self.render(branch))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        }
    }

    #[test]
    fn tips_meet_every_other_tip_at_their_shared_fork() -> Result<()> {
        let nested = Scenario::open("nested")?;
        // p3 meets p1 at c2, while p2 only joins them at c1.
        snapbox::assert_data_eq!(
            nested.meetings(&["p1", "p2", "p3"], 0)?,
            snapbox::str!["p2:1@c1 p3:2@c2"]
        );
        snapbox::assert_data_eq!(
            nested.meetings(&["p1", "p2", "p3"], 1)?,
            snapbox::str!["p1:0@c1 p3:2@c1"]
        );
        // The same tip twice meets itself.
        snapbox::assert_data_eq!(nested.meetings(&["p1", "p1"], 1)?, snapbox::str!["p1:0@p1"]);
        // A single tip meets nothing.
        snapbox::assert_data_eq!(nested.meetings(&["p1"], 0)?, snapbox::str![""]);
        // Unrelated tips meet without a base.
        snapbox::assert_data_eq!(
            Scenario::open("unrelated")?.meetings(&["r1", "r2"], 0)?,
            snapbox::str!["r2:1@none"]
        );
        Ok(())
    }

    #[test]
    fn tips_conflict_with_the_tips_they_cannot_merge_with_at_their_shared_fork() -> Result<()> {
        let conflicting = Scenario::open("conflicting")?;
        // p1 and p3 merge cleanly at c2, though at c1 they would conflict.
        snapbox::assert_data_eq!(
            conflicting.conflicts_with(&["p1", "p2", "p3"], 0)?,
            snapbox::str!["p2:1"]
        );
        snapbox::assert_data_eq!(
            conflicting.conflicts_with(&["p1", "p2", "p3"], 1)?,
            snapbox::str!["p1:0 p3:2"]
        );
        snapbox::assert_data_eq!(
            conflicting.conflicts_with(&["p1", "p2", "p3"], 2)?,
            snapbox::str!["p2:1"]
        );
        // A tip never conflicts with itself.
        snapbox::assert_data_eq!(
            conflicting.conflicts_with(&["p1", "p1"], 0)?,
            snapbox::str![""]
        );
        Ok(())
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
        // Unrelated histories never meet, so they're walked to their roots without a floor.
        snapbox::assert_data_eq!(
            unrelated.shared_history(&["r1", "r2"])?,
            snapbox::str![[r#"
r1 -> 
r2 -> 
tips: r1 r2
floor: none

"#]]
        );

        let newer_ancestor = Scenario::open("newer-ancestor")?;
        // base is walked first for its newer date, but B is left alone only once it reaches base.
        snapbox::assert_data_eq!(
            newer_ancestor.shared_history(&["B", "base"])?,
            snapbox::str![[r#"
B -> base
base -> 
tips: B base
floor: none

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

        let shallow = Scenario::open("shallow")?;
        // S is walked first for its newer date, but its missing parents are never asked for.
        snapbox::assert_data_eq!(
            shallow.shared_history(&["a", "b"])?,
            snapbox::str![[r#"
S -> 
a -> S
b -> S
tips: a b
floor: none

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

    #[test]
    fn merge_trees_join_the_groups_sharing_the_most_history_first() -> Result<()> {
        let nested = Scenario::open("nested")?;
        // p1 and p3 share c2, so they merge there before p2 joins them at c1.
        snapbox::assert_data_eq!(
            nested.merge_tree(&["p1", "p2", "p3"])?,
            snapbox::str!["fork c1 [fork c2 [p1:0, p3:2], p2:1]"]
        );
        // Branches keep the order of their first tip.
        snapbox::assert_data_eq!(
            nested.merge_tree(&["p2", "p1", "p3"])?,
            snapbox::str!["fork c1 [p2:0, fork c2 [p1:1, p3:2]]"]
        );
        // A tip inside another tip's history forks at itself.
        snapbox::assert_data_eq!(
            nested.merge_tree(&["p1", "p2", "c2"])?,
            snapbox::str!["fork c1 [fork c2 [p1:0, c2:2], p2:1]"]
        );
        // The same tip twice forks at itself too.
        snapbox::assert_data_eq!(
            nested.merge_tree(&["p1", "p1"])?,
            snapbox::str!["fork p1 [p1:0, p1:1]"]
        );
        // A single tip needs no merge at all.
        snapbox::assert_data_eq!(nested.merge_tree(&["p1"])?, snapbox::str!["p1:0"]);

        // Each fork is merged on its own before the two are merged at Z.
        snapbox::assert_data_eq!(
            Scenario::open("five-leaves")?.merge_tree(&["c1", "c4", "c2", "c5", "c3"])?,
            snapbox::str!["fork Z [fork A [c1:0, c2:2, c3:4], fork B [c4:1, c5:3]]"]
        );
        // Of the two best merge-bases, X is the deeper.
        snapbox::assert_data_eq!(
            Scenario::open("criss-cross")?.merge_tree(&["a", "b"])?,
            snapbox::str!["fork X [a:0, b:1]"]
        );
        // Unrelated histories merge without a base.
        snapbox::assert_data_eq!(
            Scenario::open("unrelated")?.merge_tree(&["r1", "r2"])?,
            snapbox::str!["fork none [r1:0, r2:1]"]
        );
        // A tip inside another's history meets it even when walked first.
        snapbox::assert_data_eq!(
            Scenario::open("newer-ancestor")?.merge_tree(&["B", "base"])?,
            snapbox::str!["fork base [B:0, base:1]"]
        );
        // The merged-in root line doesn't hide m2.
        snapbox::assert_data_eq!(
            Scenario::open("merged-root")?.merge_tree(&["a", "b"])?,
            snapbox::str!["fork m2 [a:0, b:1]"]
        );
        // A shallow boundary walked first ends the walk there.
        snapbox::assert_data_eq!(
            Scenario::open("shallow")?.merge_tree(&["a", "b"])?,
            snapbox::str!["fork S [a:0, b:1]"]
        );
        Ok(())
    }
}
