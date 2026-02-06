use std::collections::{BTreeMap, HashSet};

use anyhow::Result;
use but_graph::{Commit, CommitFlags, Graph, Segment, SegmentIndex};
use petgraph::{Direction, visit::EdgeRef as _};

use crate::graph_rebase::{
    Checkout, Edge, Editor, Pick, RevisionHistory, Selector, Step, StepGraph, StepGraphIndex, SuccessfulRebase,
};

/// Provides an extension for creating an Editor out of the segment graph
pub trait GraphExt {
    /// Creates an editor.
    fn to_editor(&self, repo: &gix::Repository) -> Result<Editor>;
}

impl GraphExt for Graph {
    /// Creates an editor out of the segment graph.
    fn to_editor(&self, repo: &gix::Repository) -> Result<Editor> {
        // TODO(CTO): Look into traversing "in workspace" segments that are not reachable from the entrypoint
        // TODO(CTO): Look into stopping at the common base
        let entrypoint = self.lookup_entrypoint()?;

        // Commits in this list are ordered such that iterating in reverse will
        // have any relevant parent commits already inserted in the graph.
        let mut commits: Vec<Commit> = vec![];
        // References are ordered from child-most to parent-most
        let mut references: BTreeMap<gix::ObjectId, Vec<gix::refs::FullName>> = BTreeMap::new();

        let mut head_refname = None;
        let workspace_commit_id = self.managed_entrypoint_commit(repo)?.map(|c| c.id);

        let mut segment_ids = vec![];

        self.visit_all_segments_including_start_until(entrypoint.segment_index, Direction::Outgoing, |segment| {
            segment_ids.push(segment.id);

            // Make a note to create a reference for named segments
            if let Some(refname) = segment.ref_name()
                && let Some(commit) = find_nearest_commit(self, segment)
            {
                references
                    .entry(commit.id)
                    .and_modify(|rs| rs.push(refname.to_owned()))
                    .or_insert_with(|| vec![refname.to_owned()]);

                if head_refname.is_none() {
                    head_refname = Some(refname.to_owned());
                }
            }

            // Make a note to create a references that sit on commits
            for commit in &segment.commits {
                if !commit.refs.is_empty() {
                    commit.flags.contains(CommitFlags::InWorkspace);
                    let refs = commit.refs.iter().map(|r| r.ref_name.clone()).collect::<Vec<_>>();
                    if let Some(entry) = references.get_mut(&commit.id) {
                        entry.extend(refs);
                    } else {
                        references.insert(commit.id, refs);
                    }
                }
            }

            commits.extend(segment.commits.clone());

            false
        });

        // Used for linking up all the commits.
        // Each commit is considered to have a top and/or a bottom node. This is
        // because in the node-adding step we will link together chains of
        // ordered references on top of their related commits
        struct StepChain {
            top: StepGraphIndex,
            bottom: StepGraphIndex,
        }
        let mut steps_for_commits: BTreeMap<gix::ObjectId, StepChain> = BTreeMap::new();
        let mut graph = StepGraph::new();

        let commit_ids = commits.iter().map(|c| c.id).collect::<HashSet<_>>();

        let mut head_selectors = vec![];

        for c in &commits {
            let has_no_parents = c.parent_ids.is_empty();
            let missing_parent_steps = c.parent_ids.iter().any(|p| !commit_ids.contains(p));

            // If the commit has parents in the commit graph, but none of
            // them are in the graph, this means but-graph did a partial
            // traversal and we want to preserve the commit as it is.
            let preserved_parents = if !has_no_parents && missing_parent_steps {
                Some(c.parent_ids.clone())
            } else {
                None
            };

            let mut pick = if Some(c.id) == workspace_commit_id {
                Pick::new_workspace_pick(c.id)
            } else {
                Pick::new_pick(c.id)
            };
            pick.preserved_parents = preserved_parents;
            let mut ni = graph.add_node(Step::Pick(pick));
            let base_ni = ni;

            // Add and link references on top
            if let Some(refs) = references.get_mut(&c.id) {
                // We insert in reverse to preserve the child-most to
                // parent-most ordering that the frontend sees in the step graph
                for r in refs.iter().rev() {
                    let ref_ni = graph.add_node(Step::Reference { refname: r.clone() });
                    graph.add_edge(ref_ni, ni, Edge { order: 0 });
                    ni = ref_ni;

                    if Some(r) == head_refname.as_ref() {
                        head_selectors.push(Selector {
                            revision: 0,
                            id: ref_ni,
                        });
                    }
                }
            }

            steps_for_commits.insert(
                c.id,
                StepChain {
                    top: ni,
                    bottom: base_ni,
                },
            );
        }

        for sidx in segment_ids {
            let s = &self[sidx];
            for (idx, c) in s.commits.iter().enumerate() {
                let mut parent_ids = if idx == s.commits.len() - 1 {
                    find_segment_edge_commits(self, sidx)
                } else {
                    vec![CommitViaReference {
                        commit: s.commits[idx + 1].id,
                        // We always want to point to the references point to a
                        // commit within a segment.
                        via_reference: true,
                    }]
                };

                // It seems like the but-graph doesn't always result in the
                // parents being ordered correctly, so we need to correct this
                // from the commit graph.
                //
                // I'm not sure on the circumstances that cause this.
                parent_ids.sort_by_cached_key(|parent| {
                    c.parent_ids
                        .iter()
                        .enumerate()
                        .find(|id| *id.1 == parent.commit)
                        .map(|(idx, _)| idx)
                        .unwrap_or(usize::MAX)
                });

                // Unless it's the workspace commit which can have virtual
                // parents, if the parents inferred from the but graph with
                // their extra information doesn't match reality, then we should
                // ignore what the but-graph has told us and pull information
                // from the commit graph directly.
                if c.parent_ids != parent_ids.iter().map(|p| p.commit).collect::<Vec<_>>()
                    && Some(c.id) != workspace_commit_id
                {
                    if let Some(StepChain { bottom, .. }) = steps_for_commits.get(&c.id)
                        && let Step::Pick(Pick { preserved_parents, .. }) = &graph[*bottom]
                        && preserved_parents.is_some()
                    {
                        // Don't warn if preserved parents is set, we don't need
                        // to warn.
                    } else {
                        tracing::warn!(
                            "but-graph inconsistent with the commit graph.\nParents for commit {} do not match.\n\nFound:{:?}\nExpected:{:?}\n\nThese IDs may be in memory, but may be helpful for debugging.",
                            c.id,
                            parent_ids.iter().map(|p| p.commit.to_string()).collect::<Vec<_>>(),
                            c.parent_ids.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
                        );
                    }

                    parent_ids = c
                        .parent_ids
                        .iter()
                        .map(|p| CommitViaReference {
                            commit: *p,
                            via_reference: true,
                        })
                        .collect();
                }

                for (i, p) in parent_ids.iter().enumerate() {
                    if let (
                        Some(StepChain { bottom, .. }),
                        Some(StepChain {
                            top: top_p,
                            bottom: bottom_p,
                        }),
                    ) = (steps_for_commits.get(&c.id), steps_for_commits.get(&p.commit))
                    {
                        if p.via_reference {
                            graph.add_edge(*bottom, *top_p, Edge { order: i });
                        } else {
                            graph.add_edge(*bottom, *bottom_p, Edge { order: i });
                        }
                    }
                }
            }
        }

        Ok(Editor {
            graph,
            initial_references: references.values().flatten().cloned().collect(),
            // TODO(CTO): We need to eventually list all worktrees that we own
            // here so we can `safe_checkout` them too.
            checkouts: head_selectors.into_iter().map(Checkout::Head).collect(),
            repo: repo.clone().with_object_memory(),
            history: RevisionHistory::new(),
        })
    }
}

impl SuccessfulRebase {
    /// Converts a SuccessfulRebase back into another editor for multi-step operations
    pub fn to_editor(self) -> Editor {
        Editor {
            graph: self.graph,
            initial_references: self.initial_references,
            checkouts: self.checkouts,
            repo: self.repo,
            history: self.history,
        }
    }
}

/// Find the commit that is nearest to the top of the segment via a first parent
/// traversal.
fn find_nearest_commit<'graph>(graph: &'graph Graph, segment: &'graph Segment) -> Option<&'graph Commit> {
    let mut target = Some(segment);
    while let Some(s) = target {
        if let Some(c) = s.commits.first() {
            return Some(c);
        }

        target = graph.segments_below_in_order(s.id).next().map(|s| &graph[s.1]);
    }

    None
}

/// When we try to find the parent commits of the bottom commit in a given
/// segment, did we also encounter a reference that points to the commit.
struct CommitViaReference {
    commit: gix::ObjectId,
    /// Between the source sidx did we encounter a reference that points to the
    /// commit
    via_reference: bool,
}

/// For a given segment, find what the but_graph considers to be the parent
/// commits.
///
/// It also annotates whether a reference was found between the bottom of the
/// starting sidx and each parent commit.
fn find_segment_edge_commits(graph: &but_graph::Graph, sidx: SegmentIndex) -> Vec<CommitViaReference> {
    struct SegmentViaReference {
        sidx: SegmentIndex,
        via_reference: bool,
    }

    let mut potential_parents = graph
        .edges_directed(sidx, Direction::Outgoing)
        .map(|p| SegmentViaReference {
            sidx: p.target(),
            via_reference: graph[p.target()].ref_name().is_some(),
        })
        .collect::<Vec<_>>();

    let mut parents = vec![];

    while let Some(candidate) = potential_parents.pop() {
        if let Some(commit) = graph[candidate.sidx].commits.first() {
            parents.push(CommitViaReference {
                commit: commit.id,
                via_reference: candidate.via_reference || !commit.refs.is_empty(),
            });
            // Don't pursue the children
            continue;
        };

        for edge in graph.edges_directed(candidate.sidx, Direction::Outgoing) {
            potential_parents.push(SegmentViaReference {
                sidx: edge.target(),
                via_reference: candidate.via_reference || graph[edge.target()].ref_name().is_some(),
            });
        }
    }

    parents
}
