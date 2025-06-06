/// Tests for visualizing the graph data structure.
mod vis {
    use crate::graph_tree;
    use but_graph::{Graph, LocalCommit, LocalCommitRelation, Segment};

    /// Simulate a graph data structure after the first pass, i.e., right after the walk.
    /// There is no pruning of 'empty' branches, just a perfect representation of the graph as is,
    /// with *some* logic applied.
    #[test]
    fn post_graph_traversal() -> anyhow::Result<()> {
        let mut graph = Graph::default();
        let init_commit_id = id("feba");
        // The local target branch sets right at the base and typically doesn't have commits,
        // these are in the segments above it.
        let local_target = Segment {
            ref_name: Some("refs/heads/main".try_into()?),
            remote_tracking_ref_name: Some("refs/remotes/origin/main".try_into()?),
            ..Default::default()
        };

        let local_target = graph.insert_root(local_target);
        // TODO: another two branches on top of base, empty to be filled.
        graph.fork_on_top(
            local_target,
            None,
            // A newly created branch which appears at the workspace base.
            Segment {
                ref_name: Some("refs/heads/new-stack".try_into()?),
                ..Default::default()
            },
        );

        let remote_to_local_target = Segment {
            ref_name: Some("refs/remotes/origin/main".try_into()?),
            commits_unique_from_tip: vec![local_commit(commit(
                id("c"),
                "remote: on top of main",
                Some(init_commit_id),
            ))],
            ..Default::default()
        };
        graph.fork_on_top(local_target, None, remote_to_local_target);

        let branch = Segment {
            ref_name: Some("refs/heads/A".try_into()?),
            remote_tracking_ref_name: Some("refs/remotes/origin/A".try_into()?),
            ref_location: None,
            commits_unique_from_tip: vec![
                LocalCommit {
                    has_conflicts: true,
                    ..local_commit(commit(id("a"), "2 in A", Some(init_commit_id)))
                },
                local_commit(commit(init_commit_id, "1 in A", None)),
            ],
            // Empty as we didn't process commits yet, right after graph traversal
            commits_unique_in_remote_tracking_branch: vec![],
            metadata: None,
        };
        let branch = graph.fork_on_top(local_target, None, branch);

        let remote_to_root_branch = Segment {
            ref_name: Some("refs/remotes/origin/A".try_into()?),
            commits_unique_from_tip: vec![
                local_commit(commit(
                    id("b"),
                    "remote: on top of 1A",
                    Some(init_commit_id),
                )),
                // Note that the initial commit was assigned to the base segment already,
                // and we are connected to it.
                // This also means that local branches absorb commits preferably and that commit-traversal
                // may need to include commits from connected segments, depending on logical constraints.
            ],
            ..Default::default()
        };
        graph.fork_on_top(branch, 1, remote_to_root_branch);

        insta::assert_snapshot!(graph_tree(&graph), @r#"
        └── ►refs/heads/main <> refs/remotes/origin/main
            ├── ►refs/heads/A <> refs/remotes/origin/A
            │   ├── 🔵febafeb❱"1 in A"
            │   │   └── ►refs/remotes/origin/A
            │   │       └── 🔵bbbbbbb❱"remote: on top of 1A"
            │   └── 🔵💥aaaaaaa❱"2 in A"
            ├── ►refs/remotes/origin/main
            │   └── 🔵ccccccc❱"remote: on top of main"
            └── ►refs/heads/new-stack
        "#);

        Ok(())
    }

    #[test]
    fn detached_head() {
        let mut graph = Graph::default();
        graph.insert_root(Segment {
            commits_unique_from_tip: vec![LocalCommit {
                inner: initial_commit(id("a")),
                relation: LocalCommitRelation::LocalOnly,
                has_conflicts: false,
            }],
            ..Default::default()
        });
        insta::assert_snapshot!(graph_tree(&graph), @r#"
        └── <unnamed>
            └── 🔵aaaaaaa❱"init"
        "#);
    }

    #[test]
    fn unborn_head() {
        insta::assert_snapshot!(graph_tree(&Graph::default()), @"<UNBORN>");
    }

    mod utils {
        use but_graph::{Commit, LocalCommit};
        use gix::ObjectId;
        use std::str::FromStr;

        pub fn initial_commit(init_commit_id: ObjectId) -> Commit {
            commit(init_commit_id, "init", None)
        }

        pub fn commit(
            id: ObjectId,
            message: &str,
            parent_ids: impl IntoIterator<Item = ObjectId>,
        ) -> Commit {
            Commit {
                id,
                parent_ids: parent_ids.into_iter().collect(),
                message: message.into(),
                author: author(),
            }
        }

        pub fn local_commit(commit: Commit) -> LocalCommit {
            LocalCommit {
                inner: commit,
                relation: Default::default(),
                has_conflicts: false,
            }
        }

        pub fn id(hex: &str) -> ObjectId {
            let hash_len = gix::hash::Kind::Sha1.len_in_hex();
            if hex.len() != hash_len {
                ObjectId::from_str(
                    &std::iter::repeat_n(hex, hash_len / hex.len())
                        .collect::<Vec<_>>()
                        .join(""),
                )
            } else {
                ObjectId::from_str(hex)
            }
            .unwrap()
        }

        fn author() -> gix::actor::Signature {
            gix::actor::Signature {
                name: "Name".into(),
                email: "name@example.com".into(),
                time: Default::default(),
            }
        }
    }
    use utils::{commit, id, initial_commit, local_commit};
}

mod utils {
    use but_graph::{LocalCommitRelation, RefLocation, SegmentIndex};
    use std::borrow::Cow;
    use std::collections::{BTreeMap, BTreeSet};
    use termtree::Tree;

    type SegmentTree = Tree<String>;

    /// Visualize `graph` as a tree.
    pub fn graph_tree(graph: &but_graph::Graph) -> SegmentTree {
        enum CommitKind {
            Local,
            Remote,
        }
        fn tree_for_commit<'a>(
            commit: &but_graph::Commit,
            extra: impl Into<Option<&'a str>>,
            has_conflicts: bool,
            kind: CommitKind,
        ) -> SegmentTree {
            let extra = extra.into();
            format!(
                "{kind}{conflict}{hex}{extra}❱{msg:?}",
                kind = match kind {
                    CommitKind::Local => {
                        "🔵"
                    }
                    CommitKind::Remote => {
                        "🟣"
                    }
                },
                conflict = if has_conflicts { "💥" } else { "" },
                extra = if let Some(extra) = extra {
                    Cow::Owned(format!(" [{extra}]"))
                } else {
                    "".into()
                },
                hex = commit.id.to_hex_with_len(7),
                msg = commit.message,
            )
            .into()
        }
        fn recurse_segment(
            graph: &but_graph::Graph,
            sidx: SegmentIndex,
            seen: &mut BTreeSet<SegmentIndex>,
        ) -> SegmentTree {
            if seen.contains(&sidx) {
                return format!(
                    "ERROR: Reached segment {sidx} for a second time: {name:?}",
                    sidx = sidx.index(),
                    name = graph[sidx].ref_name.as_ref().map(|n| n.as_bstr())
                )
                .into();
            }
            seen.insert(sidx);
            let segment = &graph[sidx];
            let on_top = {
                let mut m = BTreeMap::<_, Vec<_>>::new();
                for (cidx, sidx) in graph.segments_on_top(sidx) {
                    m.entry(cidx).or_default().push(sidx);
                }
                m
            };

            let mut root = Tree::new(format!(
                "{ref_name}{location}{remote}",
                ref_name = segment
                    .ref_name
                    .as_ref()
                    .map(|n| Cow::Owned(format!("►{}", n)))
                    .unwrap_or(Cow::Borrowed("<unnamed>")),
                location = if let Some(RefLocation::OutsideOfWorkspace) = segment.ref_location {
                    "(OUTSIDE)"
                } else {
                    ""
                },
                remote = if let Some(remote_ref_name) = segment.remote_tracking_ref_name.as_ref() {
                    Cow::Owned(format!(
                        " <> {remote_name}",
                        remote_name = remote_ref_name.as_bstr()
                    ))
                } else {
                    "".into()
                }
            ));
            for (cidx, commit) in segment.commits_unique_from_tip.iter().enumerate().rev() {
                let mut commit_tree = tree_for_commit(
                    commit,
                    if commit.relation == LocalCommitRelation::LocalOnly {
                        None
                    } else {
                        Some(commit.relation.display(commit.id))
                    },
                    commit.has_conflicts,
                    CommitKind::Local,
                );
                if let Some(segment_indices) = on_top.get(&Some(cidx)) {
                    for sidx in segment_indices {
                        commit_tree.push(recurse_segment(graph, *sidx, seen));
                    }
                }
                root.push(commit_tree);
            }
            // Get the segments that are directly connected.
            if let Some(segment_indices) = on_top.get(&None) {
                for sidx in segment_indices {
                    root.push(recurse_segment(graph, *sidx, seen));
                }
            }

            for commit in segment
                .commits_unique_in_remote_tracking_branch
                .iter()
                .rev()
            {
                root.push(tree_for_commit(
                    &commit.inner,
                    None,
                    commit.has_conflicts,
                    CommitKind::Remote,
                ));
            }

            root
        }

        let mut root = Tree::new("".to_string());
        let mut seen = Default::default();
        for sidx in graph.base_segments() {
            root.push(recurse_segment(graph, sidx, &mut seen));
        }
        let missing = graph.num_segments() - seen.len();
        if missing > 0 {
            let mut missing = Tree::new(format!(
                "ERROR: disconnected {missing} nodes unreachable through base"
            ));
            let mut newly_seen = Default::default();
            for sidx in graph.segments().filter(|sidx| !seen.contains(sidx)) {
                missing.push(recurse_segment(graph, sidx, &mut newly_seen));
            }
            root.push(missing);
            seen.extend(newly_seen);
        }

        if seen.is_empty() {
            "<UNBORN>".to_string().into()
        } else {
            root
        }
    }
}
pub use utils::graph_tree;

mod ref_metadata_legacy;
