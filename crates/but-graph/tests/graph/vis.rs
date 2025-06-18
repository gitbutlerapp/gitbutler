//! Tests for visualizing the graph data structure.

use crate::graph_tree;
use but_core::ref_metadata;
use but_graph::{CommitFlags, Graph, LocalCommit, Segment, SegmentMetadata};

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
        id: 0,
        ref_name: Some("refs/heads/main".try_into()?),
        remote_tracking_ref_name: Some("refs/remotes/origin/main".try_into()?),
        metadata: Some(SegmentMetadata::Workspace(ref_metadata::Workspace {
            ref_info: Default::default(),
            stacks: vec![],
            target_ref: None,
        })),
        ..Default::default()
    };

    let local_target = graph.insert_root(local_target);
    // TODO: another two branches on top of base, empty to be filled.
    graph.connect_new_segment(
        local_target,
        None,
        // A newly created branch which appears at the workspace base.
        Segment {
            id: 1,
            ref_name: Some("refs/heads/new-stack".try_into()?),
            ..Default::default()
        },
        0,
        None,
    );

    let remote_to_local_target = Segment {
        id: 2,
        ref_name: Some("refs/remotes/origin/main".try_into()?),
        commits: vec![local_commit(commit(
            id("c"),
            "remote: on top of main",
            Some(init_commit_id),
            CommitFlags::empty(),
        ))],
        ..Default::default()
    };
    graph.connect_new_segment(local_target, None, remote_to_local_target, 0, None);

    let branch = Segment {
        id: 3,
        ref_name: Some("refs/heads/A".try_into()?),
        remote_tracking_ref_name: Some("refs/remotes/origin/A".try_into()?),
        commits: vec![
            LocalCommit {
                has_conflicts: true,
                ..local_commit(commit(
                    id("a"),
                    "2 in A",
                    Some(init_commit_id),
                    CommitFlags::InWorkspace,
                ))
            },
            local_commit(commit(
                init_commit_id,
                "1 in A",
                None,
                CommitFlags::InWorkspace,
            )),
        ],
        // Empty as we didn't process commits yet, right after graph traversal
        commits_unique_in_remote_tracking_branch: vec![],
        metadata: None,
    };
    let branch = graph.connect_new_segment(local_target, None, branch, 0, None);

    let remote_to_root_branch = Segment {
        id: 4,
        ref_name: Some("refs/remotes/origin/A".try_into()?),
        commits: vec![
            local_commit(commit(
                id("b"),
                "remote: on top of 1A",
                Some(init_commit_id),
                CommitFlags::empty(),
            )),
            // Note that the initial commit was assigned to the base segment already,
            // and we are connected to it.
            // This also means that local branches absorb commits preferably and that commit-traversal
            // may need to include commits from connected segments, depending on logical constraints.
        ],
        ..Default::default()
    };
    graph.connect_new_segment(branch, 1, remote_to_root_branch, 0, None);

    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►►►:0:main <> origin/main
        ├── ►:3:A <> origin/A
        │   ├── 🟣💥aaaaaaa (🏘️)❱"2 in A"
        │   └── 🟣febafeb (🏘️)❱"1 in A"
        │       └── ►:4:origin/A
        │           └── ✂️🟣bbbbbbb❱"remote: on top of 1A"
        ├── ►:2:origin/main
        │   └── ✂️🟣ccccccc❱"remote: on top of main"
        └── ►:1:new-stack
    "#);

    Ok(())
}

#[test]
fn detached_head() {
    let mut graph = Graph::default();
    graph.insert_root(Segment {
        commits: vec![local_commit(commit(
            id("a"),
            "init",
            None,
            CommitFlags::empty(),
        ))],
        ..Default::default()
    });
    insta::assert_snapshot!(graph_tree(&graph), @r#"
    └── 👉►:0:anon:
        └── 🟣aaaaaaa❱"init"
    "#);
}

#[test]
fn unborn_head() {
    insta::assert_snapshot!(graph_tree(&Graph::default()), @"<UNBORN>");
}

mod utils {
    use but_graph::{Commit, CommitFlags, LocalCommit};
    use gix::ObjectId;
    use std::str::FromStr;

    pub fn commit(
        id: ObjectId,
        message: &str,
        parent_ids: impl IntoIterator<Item = ObjectId>,
        flags: CommitFlags,
    ) -> Commit {
        Commit {
            id,
            parent_ids: parent_ids.into_iter().collect(),
            message: message.into(),
            author: author(),
            refs: Vec::new(),
            flags,
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
use utils::{commit, id, local_commit};
