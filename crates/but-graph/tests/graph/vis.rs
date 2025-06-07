//! Tests for visualizing the graph data structure.
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
    graph.connect_new_segment(
        local_target,
        None,
        // A newly created branch which appears at the workspace base.
        Segment {
            ref_name: Some("refs/heads/new-stack".try_into()?),
            ..Default::default()
        },
        0,
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
    graph.connect_new_segment(local_target, None, remote_to_local_target, 0);

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
    let branch = graph.connect_new_segment(local_target, None, branch, 0);

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
    graph.connect_new_segment(branch, 1, remote_to_root_branch, 0);

    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── ►refs/heads/main <> refs/remotes/origin/main
        ├── ►refs/heads/A <> refs/remotes/origin/A
        │   ├── 🔵💥aaaaaaa❱"2 in A"
        │   └── 🔵febafeb❱"1 in A"
        │       └── ►refs/remotes/origin/A
        │           └── 🔵bbbbbbb❱"remote: on top of 1A"
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
    └── <anon>
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
            refs: Vec::new(),
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
