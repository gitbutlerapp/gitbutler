//! Tests for visualizing the graph data structure.

use std::str::FromStr;

use but_core::ref_metadata;
use but_graph::{Commit, CommitFlags, Graph, RefInfo, Segment, SegmentIndex, SegmentMetadata};
use but_testsupport::graph_tree;
use gix::ObjectId;

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
        id: 0.into(),
        ref_info: Some(RefInfo {
            ref_name: "refs/heads/main".try_into()?,
            commit_id: None,
            worktree: None,
        }),
        remote_tracking_ref_name: Some("refs/remotes/origin/main".try_into()?),
        metadata: Some(SegmentMetadata::Workspace(ref_metadata::Workspace::new(
            Default::default(),
            vec![],
            ref_metadata::ProjectMeta::default(),
        ))),
        ..Default::default()
    };

    let local_target = graph.insert_segment_set_entrypoint(local_target);
    graph.connect_new_segment(
        local_target,
        None,
        // A newly created branch which appears at the workspace base.
        Segment {
            id: 1.into(),
            ref_info: Some(RefInfo {
                ref_name: "refs/heads/new-stack".try_into()?,
                commit_id: None,
                worktree: None,
            }),
            ..Default::default()
        },
        0,
        None,
        0,
    );

    let remote_to_local_target = Segment {
        id: 2.into(),
        ref_info: Some(RefInfo {
            ref_name: "refs/remotes/origin/main".try_into()?,
            commit_id: None,
            worktree: None,
        }),
        commits: vec![commit(id("c"), Some(init_commit_id), CommitFlags::empty())],
        ..Default::default()
    };
    graph.connect_new_segment(local_target, None, remote_to_local_target, 0, None, 0);

    let branch = Segment {
        id: 3.into(),
        generation: 2,
        ref_info: Some(RefInfo {
            ref_name: "refs/heads/A".try_into()?,
            commit_id: None,
            worktree: None,
        }),
        remote_tracking_ref_name: Some("refs/remotes/origin/A".try_into()?),
        sibling_segment_id: None,
        remote_tracking_branch_segment_id: Some(SegmentIndex::from(1)),
        commits: vec![
            commit(id("a"), Some(init_commit_id), CommitFlags::InWorkspace),
            commit(init_commit_id, None, CommitFlags::InWorkspace),
        ],
        metadata: None,
    };
    let branch = graph.connect_new_segment(local_target, None, branch, 0, None, 0);

    let remote_to_root_branch = Segment {
        id: 4.into(),
        ref_info: Some(RefInfo {
            ref_name: "refs/remotes/origin/A".try_into()?,
            commit_id: None,
            worktree: None,
        }),
        commits: vec![
            commit(id("b"), Some(init_commit_id), CommitFlags::empty()),
            // Note that the initial commit was assigned to the base segment already,
            // and we are connected to it.
            // This also means that local branches absorb commits preferably and that commit-traversal
            // may need to include commits from connected segments, depending on logical constraints.
        ],
        ..Default::default()
    };
    graph.connect_new_segment(branch, 1, remote_to_root_branch, 0, None, 0);

    insta::assert_snapshot!(graph_tree(&graph), @"

    └── 👉📕►►►:0[0]:main <> origin/main
        ├── ►:1[0]:new-stack
        ├── ►:2[0]:origin/main
        │   └── ✂🟣ccccccc
        └── ►:3[2]:A <> origin/A →:1:
            ├── 🟣aaaaaaa (🏘)
            └── 🟣febafeb (🏘)
                └── ►:4[0]:origin/A
                    └── ✂🟣bbbbbbb
    ");

    Ok(())
}

#[test]
fn detached_head() {
    let mut graph = Graph::default();
    graph.insert_segment_set_entrypoint(Segment {
        commits: vec![commit(id("a"), None, CommitFlags::empty())],
        ..Default::default()
    });
    insta::assert_snapshot!(graph_tree(&graph), @"

    └── ►:0[0]:anon:
        └── 👉🏁🟣aaaaaaa
    ");
}

fn id(hex: &str) -> ObjectId {
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

fn commit(
    id: ObjectId,
    parent_ids: impl IntoIterator<Item = ObjectId>,
    flags: CommitFlags,
) -> Commit {
    Commit {
        id,
        parent_ids: parent_ids.into_iter().collect(),
        refs: Vec::new(),
        flags,
    }
}

#[test]
fn unborn_head() {
    insta::assert_snapshot!(graph_tree(&Graph::default()), @"<UNBORN>");
}
