#![deny(missing_docs)]
//! Testing utilities for the editor graph: the `Testing` trait's `steps_ascii` draws the
//! graph as an ASCII DAG for snapshot tests. The rest of the module (group grouping,
//! head finding, topological order) supports that rendering.

use crate::graph_rebase::commits::ParentEntry;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use anyhow::Result;
use but_core::RefMetadata;
use renderdag::{Ancestor, GraphRowRenderer, Renderer as _};

use crate::graph_rebase::commits::CommitIndex;
use crate::graph_rebase::store::RefIndex;
use crate::graph_rebase::{
    Editor, EditorIndex, EditorStore, RebasedEditor, positions, workspace::Subgraph,
};

/// An extension trait that adds debugging output for graphs
pub trait Testing {
    /// Creates an ASCII graph similar to `git log --graph --oneline` with commit titles.
    ///
    /// Glyphs: `●` a commit, `◎` a reference riding at its position (refs on one commit
    /// stack as an ordered group, topmost first). A parent line entering at a `◎` (as in
    /// `├─╮` above one) is a parent entry passing through that group; entering at a `●`
    /// it bypasses the refs standing there.
    fn steps_ascii(&self) -> String;
}

impl<M: RefMetadata> Testing for Editor<'_, M> {
    fn steps_ascii(&self) -> String {
        render_ascii_graph(&self.store, |id| lookup_commit_title(&self.repo, id))
    }
}

impl<M: RefMetadata> Testing for RebasedEditor<'_, M> {
    fn steps_ascii(&self) -> String {
        render_ascii_graph(&self.store, |id| lookup_commit_title(&self.repo, id))
    }
}

/// Looks up the commit title (first line of message) for a given commit id
fn lookup_commit_title(repo: &gix::Repository, id: gix::ObjectId) -> Option<String> {
    let object = repo.find_object(id).ok()?;
    let commit = object.try_into_commit().ok()?;
    let message = commit.message().ok()?;
    Some(message.title.to_string().trim().to_string())
}

/// The renderer's one-cell description — symbol and label — from typed reads:
/// `●` commit, `◎` reference, `◌` tombstone.
fn cell(
    store: &EditorStore,
    entry: EditorIndex,
    get_title: &mut impl FnMut(gix::ObjectId) -> Option<String>,
) -> (char, String) {
    match entry {
        EditorIndex::Commit(_) => match store.commit_id(entry) {
            Some(id) => {
                let mut sha = id.to_string();
                sha.truncate(7);
                let label = match get_title(id) {
                    Some(title) => format!("{sha} {title}"),
                    None => sha,
                };
                ('●', label)
            }
            None => ('◌', "removed".to_string()),
        },
        EditorIndex::Ref(_) => match store.reference(entry) {
            Some((refname, mutable)) => {
                let name = refname.as_bstr().to_string();
                let label = if mutable {
                    name
                } else {
                    format!("{name} (immutable)")
                };
                ('◎', label)
            }
            None => ('◌', "removed".to_string()),
        },
    }
}

/// The reference groups, keyed by their (commit, entering-parent entries) position and ordered by depth —
/// the render's view of positioned refs as rows.
type GroupKey = (CommitIndex, Vec<ParentEntry>);

fn ref_groups(store: &EditorStore) -> HashMap<GroupKey, Vec<RefIndex>> {
    let mut out: HashMap<_, Vec<(usize, RefIndex)>> = HashMap::new();
    for entry in store.positioned_refs() {
        let Some(on) = store.positioned_on(entry) else {
            continue;
        };
        out.entry((on, positions::entering(store, entry)))
            .or_default()
            .push((positions::ref_depth(store, entry), entry));
    }
    out.into_iter()
        .map(|(key, mut members)| {
            members.sort_by_key(|(depth, _)| *depth);
            (key, members.into_iter().map(|(_, entry)| entry).collect())
        })
        .collect()
}

/// Find head rows: commits (and tombstones) without incoming parent entries, plus the tops of root
/// reference groups (positioned with nothing above them).
fn find_heads(store: &EditorStore) -> Vec<EditorIndex> {
    let mut has_incoming: HashSet<CommitIndex> = HashSet::new();
    for idx in store.commits.commit_indices() {
        has_incoming.extend(store.parents(idx));
    }
    let groups = ref_groups(store);
    // Commit rows first, then references (the render sorts heads deterministically, so
    // seed order does not reach snapshots): a commit or tombstone with no incoming parent entries, or
    // the top of a root reference group.
    store
        .commits
        .commit_indices()
        .map(EditorIndex::from)
        .chain(store.ref_indices().map(EditorIndex::from))
        .filter(|idx| match store.positioned_on(*idx) {
            Some(on) => {
                let entering = positions::entering(store, *idx);
                entering.is_empty()
                    && groups
                        .get(&(on, entering))
                        .and_then(|members| members.last())
                        .map(|&m| EditorIndex::from(m))
                        == Some(*idx)
            }
            // Positionless entries (commits, tombstones, and hand-built reference entries that never
            // got a stored position) keep the pre-position rule.
            None => !idx.as_commit().is_some_and(|n| has_incoming.contains(&n)),
        })
        .collect()
}

/// Rendered parents of `entry`, in order: a reference row points at the next group member
/// below it (or its commit); a commit's parent entries route through the group positioned on
/// that (parent, parent number), when one exists — reproducing the in-between rows references had
/// when they were nodes.
fn rendered_parents(store: &EditorStore, entry: EditorIndex) -> Vec<EditorIndex> {
    let groups = ref_groups(store);
    if let Some(on) = store.positioned_on(entry) {
        let group = groups
            .get(&(on, positions::entering(store, entry)))
            .map(Vec::as_slice)
            .unwrap_or_default();
        let below = group
            .iter()
            .position(|&n| EditorIndex::from(n) == entry)
            .and_then(|ix| ix.checked_sub(1))
            .map(|ix| EditorIndex::from(group[ix]))
            .unwrap_or(on.into());
        return vec![below];
    }
    store
        .parents(entry)
        .iter()
        .copied()
        .enumerate()
        .map(|(order, target)| {
            let entry_commit = entry.as_commit();
            groups
                .iter()
                .find(|((commit, entering), _)| {
                    *commit == target
                        && entry_commit.is_some_and(|n| {
                            entering.contains(&ParentEntry {
                                child: n,
                                number: order,
                            })
                        })
                })
                .and_then(|(_, group)| group.last().copied().map(EditorIndex::from))
                .unwrap_or(target.into())
        })
        .collect()
}

/// A deterministic ordering for the head entries so snapshots are stable: commits
/// before references, then by id / refname.
fn compare_heads(store: &EditorStore, a: EditorIndex, b: EditorIndex) -> Ordering {
    head_key(store, a).cmp(&head_key(store, b))
}

/// The sort key behind [`compare_heads`]: tombstones sort first, then commits by id,
/// then references by name.
fn head_key(store: &EditorStore, entry: EditorIndex) -> (u8, String) {
    match entry {
        EditorIndex::Commit(_) => match store.commit_id(entry) {
            Some(id) => (1, id.to_string()),
            None => (0, String::new()),
        },
        EditorIndex::Ref(_) => match store.reference(entry) {
            Some((refname, _)) => (2, refname.as_bstr().to_string()),
            None => (0, String::new()),
        },
    }
}

/// Children-first topological order over `entries`, seeded from `heads`.
///
/// Only parent entries between entries in `entries` are followed, so this works for a full
/// graph (where `entries` is every index) as well as a subgraph that doesn't
/// include its parents.
fn topological_order(
    store: &EditorStore,
    entries: &HashSet<EditorIndex>,
    heads: &[EditorIndex],
) -> Vec<EditorIndex> {
    // Incoming parent entries from *within* the entry set.
    let mut in_degree: HashMap<EditorIndex, usize> = entries.iter().map(|&n| (n, 0)).collect();
    for &n in entries {
        for parent in rendered_parents(store, n) {
            if let Some(deg) = in_degree.get_mut(&parent) {
                *deg += 1;
            }
        }
    }

    let mut result = Vec::new();
    let mut visited: HashSet<EditorIndex> = HashSet::new();

    fn dfs(
        entry: EditorIndex,
        store: &EditorStore,
        entries: &HashSet<EditorIndex>,
        visited: &mut HashSet<EditorIndex>,
        in_degree: &mut HashMap<EditorIndex, usize>,
        result: &mut Vec<EditorIndex>,
    ) {
        if visited.contains(&entry) || in_degree.get(&entry).is_some_and(|&d| d > 0) {
            return;
        }

        visited.insert(entry);
        result.push(entry);

        let parents: Vec<_> = rendered_parents(store, entry)
            .into_iter()
            .filter(|p| entries.contains(p))
            .collect();
        for parent in &parents {
            if let Some(deg) = in_degree.get_mut(parent) {
                *deg = deg.saturating_sub(1);
            }
        }
        for parent in parents {
            dfs(parent, store, entries, visited, in_degree, result);
        }
    }

    for &head in heads {
        dfs(
            head,
            store,
            entries,
            &mut visited,
            &mut in_degree,
            &mut result,
        );
    }

    result
}

/// Render a (sub)graph of steps as a box-drawing DAG (à la `git log --graph`)
/// using `sapling-renderdag`.
///
/// `entries` is the set of steps to draw and `heads` are the tips to seed the
/// ordering from; parents outside `entries` are simply dropped, so this renders
/// both full graphs and subgraphs.
fn render_store<F>(
    store: &EditorStore,
    entries: &HashSet<EditorIndex>,
    heads: &[EditorIndex],
    mut get_title: F,
) -> String
where
    F: FnMut(gix::ObjectId) -> Option<String>,
{
    let mut heads = heads.to_vec();
    // Row-view tops without a rendered child inside the subgraph — e.g. reference groups
    // positioned above a stack's head commit, entered only from outside — are heads too.
    let mut in_degree: HashMap<EditorIndex, usize> = entries.iter().map(|&n| (n, 0)).collect();
    for &n in entries {
        for parent in rendered_parents(store, n) {
            if let Some(deg) = in_degree.get_mut(&parent) {
                *deg += 1;
            }
        }
    }
    let mut extra: Vec<EditorIndex> = entries
        .iter()
        .copied()
        .filter(|n| in_degree.get(n).is_none_or(|&d| d == 0) && !heads.contains(n))
        // A positioned reference whose commit lies outside the set is a boundary group the
        // original walk never reached from this subgraph's heads — don't seed it.
        .filter(|n| {
            store.positioned_on(*n).is_none_or(|_| {
                store
                    .resolve_to_commit(*n)
                    .is_some_and(|commit| entries.contains(&commit.into()))
            })
        })
        .collect();
    extra.sort_by(|a, b| compare_heads(store, *a, *b));
    heads.retain(|h| in_degree.get(h).is_none_or(|&d| d == 0));
    heads.extend(extra);
    heads.sort_by(|a, b| compare_heads(store, *a, *b));

    let mut renderer = GraphRowRenderer::<EditorIndex>::new()
        .output()
        .with_min_row_height(1)
        .build_box_drawing();

    let mut out = String::new();
    for entry in topological_order(store, entries, &heads) {
        let (symbol, label) = cell(store, entry, &mut get_title);
        let parents = rendered_parents(store, entry)
            .into_iter()
            .filter(|p| entries.contains(p))
            .map(Ancestor::Parent)
            .collect();
        out.push_str(&renderer.next_row(entry, parents, symbol.to_string(), label));
    }
    out.trim_end().to_string()
}

/// Render the full editor graph as a box-drawing DAG.
pub(crate) fn render_ascii_graph<F>(store: &EditorStore, get_title: F) -> String
where
    F: FnMut(gix::ObjectId) -> Option<String>,
{
    let entries: HashSet<EditorIndex> = store
        .commits
        .commit_indices()
        .map(EditorIndex::from)
        .chain(store.ref_indices().map(EditorIndex::from))
        .collect();
    let heads = find_heads(store);
    render_store(store, &entries, &heads, get_title)
}

impl<M: RefMetadata> Editor<'_, M> {
    /// Render a [`Subgraph`] (e.g. one of the parts of [`Editor::graph_workspace`])
    /// as a box-drawing DAG, in the same style as [`Testing::steps_ascii`].
    pub fn subgraph_ascii(&self, subgraph: &Subgraph) -> String {
        let entries: HashSet<EditorIndex> = subgraph.entries.iter().copied().collect();
        let heads: Vec<EditorIndex> = subgraph.heads.to_vec();
        render_store(&self.store, &entries, &heads, |id| {
            lookup_commit_title(&self.repo, id)
        })
    }

    /// Render an entire [`Editor::graph_workspace`] projection for snapshot
    /// tests: the commits above the workspace, the workspace commit, then each
    /// stack in turn. Each section is rendered with [`Editor::subgraph_ascii`].
    pub fn graph_workspace_ascii(
        &self,
        stacks: &[but_graph::workspace::SegmentStack],
    ) -> Result<String> {
        let ws = self.graph_workspace(stacks)?;
        let body = |rendered: String| {
            if rendered.is_empty() {
                "(empty)".to_string()
            } else {
                rendered
            }
        };

        let mut sections = vec![format!(
            "# Above workspace\n{}",
            body(self.subgraph_ascii(&ws.above_workspace))
        )];

        let workspace_commit = ws.workspace_commit.map(|entry| Subgraph {
            heads: vec![entry],
            entries: [entry].into(),
        });
        sections.push(format!(
            "# Workspace commit\n{}",
            body(
                workspace_commit
                    .map(|s| self.subgraph_ascii(&s))
                    .unwrap_or_default()
            )
        ));

        for (i, stack) in ws.stacks.iter().enumerate() {
            sections.push(format!("# Stack {i}\n{}", body(self.subgraph_ascii(stack))));
        }

        Ok(sections.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use crate::graph_rebase::CommitSpec;

    fn make_spec(hex: &str) -> CommitSpec {
        CommitSpec::new(gix::ObjectId::from_str(hex).unwrap())
    }

    fn add_ref(store: &mut EditorStore, name: &str) -> RefIndex {
        store.add_reference(
            gix::refs::FullName::try_from(format!("refs/heads/{name}")).unwrap(),
            true,
            true,
        )
    }

    /// Add a reference positioned on `on`, the way the editor's own ref creation authors
    /// refs — a root group of one.
    fn place_ref(store: &mut EditorStore, name: &str, on: CommitIndex) -> RefIndex {
        let ix = add_ref(store, name);
        store.set_position(ix, on, &[], false, None);
        ix
    }

    /// Helper to append a parent entry; the stated `order` documents the intended parent
    /// number and is asserted against the push (arrays make insertion order the structure).
    fn add_parent_entry(store: &mut EditorStore, from: CommitIndex, to: CommitIndex, order: usize) {
        let parent_number = store.commits.push_parent(from, to);
        assert_eq!(
            parent_number, order,
            "test builder must push parents in parent_number order"
        );
    }

    #[test]
    fn linear_graph() {
        // Simple linear: main on B -> C -> D
        let mut store = EditorStore::default();
        let b = store
            .commits
            .add_commit(make_spec("1111111111111111111111111111111111111111"));
        let c = store
            .commits
            .add_commit(make_spec("2222222222222222222222222222222222222222"));
        let d = store
            .commits
            .add_commit(make_spec("3333333333333333333333333333333333333333"));
        let none = store.commits.add_tombstone();
        place_ref(&mut store, "main", b);

        add_parent_entry(&mut store, b, c, 0);
        add_parent_entry(&mut store, c, d, 0);
        add_parent_entry(&mut store, d, none, 0);

        let output = render_ascii_graph(&store, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●  1111111
●  2222222
●  3333333
◌  removed
"#]]
        );
    }

    #[test]
    fn two_way_merge() {
        // Two-way merge:
        //   M
        //  / \
        // A   B
        //  \ /
        //   C
        let mut store = EditorStore::default();
        let m = store
            .commits
            .add_commit(make_spec("9999999999999999999999999999999999999999"));
        let a = store
            .commits
            .add_commit(make_spec("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = store
            .commits
            .add_commit(make_spec("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = store
            .commits
            .add_commit(make_spec("cccccccccccccccccccccccccccccccccccccccc"));
        place_ref(&mut store, "main", m);

        // M has two parents: A (first) and B (second)
        add_parent_entry(&mut store, m, a, 0);
        add_parent_entry(&mut store, m, b, 1);
        // Both A and B have C as parent
        add_parent_entry(&mut store, a, c, 0);
        add_parent_entry(&mut store, b, c, 0);

        let output = render_ascii_graph(&store, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●    9999999
├─╮
● │  aaaaaaa
│ ●  bbbbbbb
├─╯
●  ccccccc
"#]]
        );
    }

    #[test]
    fn three_way_merge() {
        // Three-way merge:
        //     M
        //   / | \
        //  A  B  C
        //   \ | /
        //     D
        let mut store = EditorStore::default();
        let m = store
            .commits
            .add_commit(make_spec("9999999999999999999999999999999999999999"));
        let a = store
            .commits
            .add_commit(make_spec("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = store
            .commits
            .add_commit(make_spec("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = store
            .commits
            .add_commit(make_spec("cccccccccccccccccccccccccccccccccccccccc"));
        let d = store
            .commits
            .add_commit(make_spec("dddddddddddddddddddddddddddddddddddddddd"));
        place_ref(&mut store, "main", m);

        // M has three parents
        add_parent_entry(&mut store, m, a, 0);
        add_parent_entry(&mut store, m, b, 1);
        add_parent_entry(&mut store, m, c, 2);
        // All converge to D
        add_parent_entry(&mut store, a, d, 0);
        add_parent_entry(&mut store, b, d, 0);
        add_parent_entry(&mut store, c, d, 0);

        let output = render_ascii_graph(&store, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●      9999999
├─┬─╮
● │ │  aaaaaaa
│ ● │  bbbbbbb
├─╯ │
│   ●  ccccccc
├───╯
●  ddddddd
"#]]
        );
    }

    #[test]
    fn nested_merge_first_leg_forks_into_three() {
        // First leg of a 2-way merge forks into 3:
        //       M
        //      / \
        //     F   B
        //   / | \  \
        //  X  Y  Z  \
        //   \ | /   |
        //     C-----+
        let mut store = EditorStore::default();
        let m = store
            .commits
            .add_commit(make_spec("9999999999999999999999999999999999999999"));
        let f = store
            .commits
            .add_commit(make_spec("ffffffffffffffffffffffffffffffffffffffff")); // fork point
        let b = store
            .commits
            .add_commit(make_spec("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let x = store
            .commits
            .add_commit(make_spec("1111111111111111111111111111111111111111"));
        let y = store
            .commits
            .add_commit(make_spec("2222222222222222222222222222222222222222"));
        let z = store
            .commits
            .add_commit(make_spec("3333333333333333333333333333333333333333"));
        let c = store
            .commits
            .add_commit(make_spec("cccccccccccccccccccccccccccccccccccccccc"));
        place_ref(&mut store, "main", m);

        // M has two parents: F (first) and B (second)
        add_parent_entry(&mut store, m, f, 0);
        add_parent_entry(&mut store, m, b, 1);

        // F forks into X, Y, Z
        add_parent_entry(&mut store, f, x, 0);
        add_parent_entry(&mut store, f, y, 1);
        add_parent_entry(&mut store, f, z, 2);

        // X, Y, Z all converge to C
        add_parent_entry(&mut store, x, c, 0);
        add_parent_entry(&mut store, y, c, 0);
        add_parent_entry(&mut store, z, c, 0);

        // B also goes to C
        add_parent_entry(&mut store, b, c, 0);

        let output = render_ascii_graph(&store, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●    9999999
├─╮
● │      fffffff
├───┬─╮
● │ │ │  1111111
│ │ ● │  2222222
├───╯ │
│ │   ●  3333333
├─────╯
│ ●  bbbbbbb
├─╯
●  ccccccc
"#]]
        );
    }

    #[test]
    fn four_way_merge() {
        // Four-way merge
        let mut store = EditorStore::default();
        let m = store
            .commits
            .add_commit(make_spec("9999999999999999999999999999999999999999"));
        let a = store
            .commits
            .add_commit(make_spec("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = store
            .commits
            .add_commit(make_spec("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = store
            .commits
            .add_commit(make_spec("cccccccccccccccccccccccccccccccccccccccc"));
        let d = store
            .commits
            .add_commit(make_spec("dddddddddddddddddddddddddddddddddddddddd"));
        let base = store
            .commits
            .add_commit(make_spec("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
        place_ref(&mut store, "main", m);

        add_parent_entry(&mut store, m, a, 0);
        add_parent_entry(&mut store, m, b, 1);
        add_parent_entry(&mut store, m, c, 2);
        add_parent_entry(&mut store, m, d, 3);

        add_parent_entry(&mut store, a, base, 0);
        add_parent_entry(&mut store, b, base, 0);
        add_parent_entry(&mut store, c, base, 0);
        add_parent_entry(&mut store, d, base, 0);

        let output = render_ascii_graph(&store, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●        9999999
├─┬─┬─╮
● │ │ │  aaaaaaa
│ ● │ │  bbbbbbb
├─╯ │ │
│   ● │  ccccccc
├───╯ │
│     ●  ddddddd
├─────╯
●  eeeeeee
"#]]
        );
    }

    #[test]
    fn asymmetric_merge_long_first_branch() {
        // Asymmetric merge where first branch is longer:
        //   M
        //  / \
        // A1  B
        // |   |
        // A2  |
        // |   |
        // A3  |
        //  \ /
        //   C
        let mut store = EditorStore::default();
        let m = store
            .commits
            .add_commit(make_spec("9999999999999999999999999999999999999999"));
        let a1 = store
            .commits
            .add_commit(make_spec("a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1"));
        let a2 = store
            .commits
            .add_commit(make_spec("a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2"));
        let a3 = store
            .commits
            .add_commit(make_spec("a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3"));
        let b = store
            .commits
            .add_commit(make_spec("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = store
            .commits
            .add_commit(make_spec("cccccccccccccccccccccccccccccccccccccccc"));
        place_ref(&mut store, "main", m);

        add_parent_entry(&mut store, m, a1, 0);
        add_parent_entry(&mut store, m, b, 1);
        add_parent_entry(&mut store, a1, a2, 0);
        add_parent_entry(&mut store, a2, a3, 0);
        add_parent_entry(&mut store, a3, c, 0);
        add_parent_entry(&mut store, b, c, 0);

        let output = render_ascii_graph(&store, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●    9999999
├─╮
● │  a1a1a1a
● │  a2a2a2a
● │  a3a3a3a
│ ●  bbbbbbb
├─╯
●  ccccccc
"#]]
        );
    }

    #[test]
    fn consecutive_forks() {
        // A forks to B,C; B immediately forks to D,E; all merge to F
        //       A
        //      / \
        //     B   C
        //    / \   \
        //   D   E   |
        //    \ /    |
        //     F-----+
        let mut store = EditorStore::default();
        let a = store
            .commits
            .add_commit(make_spec("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = store
            .commits
            .add_commit(make_spec("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = store
            .commits
            .add_commit(make_spec("cccccccccccccccccccccccccccccccccccccccc"));
        let d = store
            .commits
            .add_commit(make_spec("dddddddddddddddddddddddddddddddddddddddd"));
        let e = store
            .commits
            .add_commit(make_spec("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
        let f = store
            .commits
            .add_commit(make_spec("ffffffffffffffffffffffffffffffffffffffff"));
        place_ref(&mut store, "main", a);

        // A forks to B, C
        add_parent_entry(&mut store, a, b, 0);
        add_parent_entry(&mut store, a, c, 1);

        // B forks to D, E
        add_parent_entry(&mut store, b, d, 0);
        add_parent_entry(&mut store, b, e, 1);

        // D, E, C all converge to F
        add_parent_entry(&mut store, d, f, 0);
        add_parent_entry(&mut store, e, f, 0);
        add_parent_entry(&mut store, c, f, 0);

        let output = render_ascii_graph(&store, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●    aaaaaaa
├─╮
● │    bbbbbbb
├───╮
● │ │  ddddddd
│ │ ●  eeeeeee
├───╯
│ ●  ccccccc
├─╯
●  fffffff
"#]]
        );
    }

    #[test]
    fn wide_merge_with_first_branch_forking_into_three() {
        // M 3-way merges F,B,C; the first branch F itself forks into X,Y,Z,
        // and everything converges back at D.
        //          M
        //        / | \
        //       F  B  C
        //      /|\  \ |
        //     X Y Z  \|
        //      \|/    |
        //       D-----+
        let mut store = EditorStore::default();
        let m = store
            .commits
            .add_commit(make_spec("9999999999999999999999999999999999999999"));
        let f = store
            .commits
            .add_commit(make_spec("ffffffffffffffffffffffffffffffffffffffff"));
        let b = store
            .commits
            .add_commit(make_spec("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = store
            .commits
            .add_commit(make_spec("cccccccccccccccccccccccccccccccccccccccc"));
        let x = store
            .commits
            .add_commit(make_spec("1111111111111111111111111111111111111111"));
        let y = store
            .commits
            .add_commit(make_spec("2222222222222222222222222222222222222222"));
        let z = store
            .commits
            .add_commit(make_spec("3333333333333333333333333333333333333333"));
        let d = store
            .commits
            .add_commit(make_spec("dddddddddddddddddddddddddddddddddddddddd"));
        place_ref(&mut store, "main", m);

        // M forks to F, B, C
        add_parent_entry(&mut store, m, f, 0);
        add_parent_entry(&mut store, m, b, 1);
        add_parent_entry(&mut store, m, c, 2);

        // F forks to X, Y, Z
        add_parent_entry(&mut store, f, x, 0);
        add_parent_entry(&mut store, f, y, 1);
        add_parent_entry(&mut store, f, z, 2);

        // X, Y, Z, B, C all converge to D
        add_parent_entry(&mut store, x, d, 0);
        add_parent_entry(&mut store, y, d, 0);
        add_parent_entry(&mut store, z, d, 0);
        add_parent_entry(&mut store, b, d, 0);
        add_parent_entry(&mut store, c, d, 0);

        let output = render_ascii_graph(&store, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●      9999999
├─┬─╮
● │ │      fffffff
├─────┬─╮
● │ │ │ │  1111111
│ │ │ ● │  2222222
├─────╯ │
│ │ │   ●  3333333
├───────╯
│ ● │  bbbbbbb
├─╯ │
│   ●  ccccccc
├───╯
●  ddddddd
"#]]
        );
    }

    #[test]
    fn fork_target_shared_with_a_sibling_branch() {
        // A fork (D -> E, F) where one target (F) is also reached by a sibling
        // branch (C), so F has two children in different stacks.
        //
        //       M
        //      /|\
        //     A B C
        //     |   |
        //     D   |   <- D continues from A
        //    / \ /
        //   E   F     <- D forks to E and F, F is shared with C
        //    \ /
        //     base
        let mut store = EditorStore::default();
        let m = store
            .commits
            .add_commit(make_spec("9999999999999999999999999999999999999999"));
        let a = store
            .commits
            .add_commit(make_spec("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = store
            .commits
            .add_commit(make_spec("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = store
            .commits
            .add_commit(make_spec("cccccccccccccccccccccccccccccccccccccccc"));
        let d = store
            .commits
            .add_commit(make_spec("dddddddddddddddddddddddddddddddddddddddd"));
        let e = store
            .commits
            .add_commit(make_spec("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
        let f = store
            .commits
            .add_commit(make_spec("ffffffffffffffffffffffffffffffffffffffff"));
        let base = store
            .commits
            .add_commit(make_spec("0000000000000000000000000000000000000000"));
        place_ref(&mut store, "main", m);

        // M forks to A, B, C
        add_parent_entry(&mut store, m, a, 0);
        add_parent_entry(&mut store, m, b, 1);
        add_parent_entry(&mut store, m, c, 2);

        // A -> D
        add_parent_entry(&mut store, a, d, 0);

        // C -> F (C's branch leads to F)
        add_parent_entry(&mut store, c, f, 0);

        // D forks to E and F
        add_parent_entry(&mut store, d, e, 0);
        add_parent_entry(&mut store, d, f, 1);

        // B -> base, E -> base, F -> base
        add_parent_entry(&mut store, b, base, 0);
        add_parent_entry(&mut store, e, base, 0);
        add_parent_entry(&mut store, f, base, 0);

        let output = render_ascii_graph(&store, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●      9999999
├─┬─╮
● │ │  aaaaaaa
● │ │    ddddddd
├─────╮
● │ │ │  eeeeeee
│ ● │ │  bbbbbbb
├─╯ │ │
│   ● │  ccccccc
│   ├─╯
│   ●  fffffff
├───╯
●  0000000
"#]]
        );
    }

    #[test]
    fn fork_with_multiple_branches_merging_to_same_point() {
        // Tests a diamond pattern where multiple branches merge to a single point.
        // D forks to E and G, where G is also the merge target for B and C.
        // Then E and G merge at F.
        //
        //        M
        //      / | \
        //     A  B  C
        //     |  |  |
        //     D  |  |   <- D is on A's branch
        //    / \ |  |
        //   E   \|  |   <- D forks to E and G
        //   |    \ /
        //   |     G     <- B, C, and D's second branch merge at G
        //    \   /
        //      F        <- E and G merge at F
        let mut store = EditorStore::default();
        let m = store
            .commits
            .add_commit(make_spec("1111111111111111111111111111111111111111"));
        let a = store
            .commits
            .add_commit(make_spec("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = store
            .commits
            .add_commit(make_spec("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = store
            .commits
            .add_commit(make_spec("cccccccccccccccccccccccccccccccccccccccc"));
        let d = store
            .commits
            .add_commit(make_spec("dddddddddddddddddddddddddddddddddddddddd"));
        let e = store
            .commits
            .add_commit(make_spec("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
        let g = store
            .commits
            .add_commit(make_spec("9999999999999999999999999999999999999999"));
        let f = store
            .commits
            .add_commit(make_spec("ffffffffffffffffffffffffffffffffffffffff"));
        place_ref(&mut store, "main", m);

        // M forks to A, B, C
        add_parent_entry(&mut store, m, a, 0);
        add_parent_entry(&mut store, m, b, 1);
        add_parent_entry(&mut store, m, c, 2);

        // A -> D
        add_parent_entry(&mut store, a, d, 0);

        // D forks to E and G
        add_parent_entry(&mut store, d, e, 0);
        add_parent_entry(&mut store, d, g, 1);

        // B, C merge to G
        add_parent_entry(&mut store, b, g, 0);
        add_parent_entry(&mut store, c, g, 0);

        // E and G merge to F
        add_parent_entry(&mut store, e, f, 0);
        add_parent_entry(&mut store, g, f, 0);

        let output = render_ascii_graph(&store, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●      1111111
├─┬─╮
● │ │  aaaaaaa
● │ │    ddddddd
├─────╮
● │ │ │  eeeeeee
│ ● │ │  bbbbbbb
│ ├───╯
│ │ ●  ccccccc
│ ├─╯
│ ●  9999999
├─╯
●  fffffff
"#]]
        );
    }

    #[test]
    fn three_way_fork_with_a_shared_target() {
        // A 3-way fork (D -> E, F, shared) where one target (shared) is also
        // reached by a sibling branch (C).
        //
        //      M
        //     /|\
        //    A B C
        //    |   |
        //    D   |    <- D continues from A
        //   /|\ /
        //  E F shared <- D forks to E, F, shared where shared comes from C
        //   \|/
        //    base
        let mut store = EditorStore::default();
        let m = store
            .commits
            .add_commit(make_spec("9999999999999999999999999999999999999999"));
        let a = store
            .commits
            .add_commit(make_spec("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = store
            .commits
            .add_commit(make_spec("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let c = store
            .commits
            .add_commit(make_spec("cccccccccccccccccccccccccccccccccccccccc"));
        let d = store
            .commits
            .add_commit(make_spec("dddddddddddddddddddddddddddddddddddddddd"));
        let e = store
            .commits
            .add_commit(make_spec("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"));
        let f = store
            .commits
            .add_commit(make_spec("ffffffffffffffffffffffffffffffffffffffff"));
        let shared = store
            .commits
            .add_commit(make_spec("1111111111111111111111111111111111111111"));
        let base = store
            .commits
            .add_commit(make_spec("0000000000000000000000000000000000000000"));
        place_ref(&mut store, "main", m);

        // M forks to A, B, C
        add_parent_entry(&mut store, m, a, 0);
        add_parent_entry(&mut store, m, b, 1);
        add_parent_entry(&mut store, m, c, 2);

        // A -> D
        add_parent_entry(&mut store, a, d, 0);

        // C -> shared
        add_parent_entry(&mut store, c, shared, 0);

        // D forks to E, F, shared (shared is also reached via C)
        add_parent_entry(&mut store, d, e, 0);
        add_parent_entry(&mut store, d, f, 1);
        add_parent_entry(&mut store, d, shared, 2);

        // E, F, shared merge to base; B is left as a dangling tip.
        add_parent_entry(&mut store, e, base, 0);
        add_parent_entry(&mut store, f, base, 0);
        add_parent_entry(&mut store, shared, base, 0);

        let output = render_ascii_graph(&store, |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
◎  refs/heads/main
●      9999999
├─┬─╮
● │ │  aaaaaaa
● │ │      ddddddd
├─────┬─╮
● │ │ │ │  eeeeeee
│ │ │ ● │  fffffff
├─────╯ │
│ ● │   │  bbbbbbb
│   ●   │  ccccccc
│   ├───╯
│   ●  1111111
├───╯
●  0000000
"#]]
        );
    }

    #[test]
    fn subgraph_drops_parents_outside_the_commit_set() {
        // main on a -> b -> base, rendering only the subgraph {a, b}.
        // `main` (positioned on `a`) and `base` (a parent of `b`) are outside
        // the set, so neither is drawn and `b` renders as a root.
        let mut store = EditorStore::default();
        let a = store
            .commits
            .add_commit(make_spec("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        let b = store
            .commits
            .add_commit(make_spec("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"));
        let base = store
            .commits
            .add_commit(make_spec("0000000000000000000000000000000000000000"));
        place_ref(&mut store, "main", a);

        add_parent_entry(&mut store, a, b, 0);
        add_parent_entry(&mut store, b, base, 0);

        let entries: HashSet<EditorIndex> = [a.into(), b.into()].into_iter().collect();
        let output = render_store(&store, &entries, &[a.into()], |_| None);
        snapbox::assert_data_eq!(
            output,
            snapbox::str![[r#"
●  aaaaaaa
●  bbbbbbb
"#]]
        );
    }
}
