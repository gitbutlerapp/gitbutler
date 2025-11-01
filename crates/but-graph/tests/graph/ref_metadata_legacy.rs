use std::{ops::Deref, path::PathBuf};

use but_core::{
    RefMetadata,
    ref_metadata::{
        StackId, ValueInfo,
        WorkspaceCommitRelation::{Merged, Outside},
        WorkspaceStack, WorkspaceStackBranch,
    },
};
use but_graph::{VirtualBranchesTomlMetadata, virtual_branches_legacy_types::Target};
use but_testsupport::{
    debug_str,
    gix_testtools::tempfile::{TempDir, tempdir},
    sanitize_uuids_and_timestamps, sanitize_uuids_and_timestamps_with_mapping,
};
use gitbutler_reference::RemoteRefname;

#[test]
fn journey() -> anyhow::Result<()> {
    let (mut store, _tmp) = vb_store_rw("virtual-branches-01")?;

    assert_eq!(store.iter().count(), 15, "There are items to test on");
    roundtrip_journey(&mut store)?;
    let writable_toml_path = store.path().to_owned();
    drop(store);

    assert!(
        !writable_toml_path.exists(),
        "The file is deleted when the workspace is removed"
    );
    let store = VirtualBranchesTomlMetadata::from_path(&writable_toml_path)?;
    assert_eq!(
        store.iter().count(),
        0,
        "on drop we write the file immediately"
    );
    drop(store);
    assert!(
        !writable_toml_path.exists(),
        "default content isn't written back either"
    );

    Ok(())
}

#[test]
fn read_only() -> anyhow::Result<()> {
    let (mut store, _tmp) = vb_store_rw("virtual-branches-01")?;
    let ws = store.workspace("refs/heads/gitbutler/workspace".try_into()?)?;
    assert!(!ws.is_default(), "value read from file");
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&ws.stacks));
    insta::assert_snapshot!(actual, @r#"
    [
        WorkspaceStack {
            id: 1,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/A",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
        WorkspaceStack {
            id: 2,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/B-top",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/B",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/C-top-empty",
                    archived: true,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/C-empty",
                    archived: true,
                },
            ],
            workspacecommit_relation: Merged,
        },
        WorkspaceStack {
            id: 3,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/C-top",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/C-middle",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/C",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/D-top-empty",
                    archived: true,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/D-middle-empty",
                    archived: true,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/D-empty",
                    archived: true,
                },
            ],
            workspacecommit_relation: Merged,
        },
        WorkspaceStack {
            id: 4,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/D-top",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/D",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
        WorkspaceStack {
            id: 5,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/E",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ]
    "#);

    for uuid in uuids.keys() {
        assert_ne!(
            ws.stacks.iter().find(|s| s.id == uuid.parse().unwrap()),
            None,
            "each UUID is available as workspace stack."
        );
    }

    let branches = ws
        .stacks
        .iter()
        .flat_map(|stack| &stack.branches)
        .map(|branch| {
            let b = store
                .branch(branch.ref_name.as_ref())
                .expect("branch is present for each refs mentioned in workspace");
            let b_id = b
                .stack_id()
                .expect("each branch has the stack-id of the stack its in");
            (
                uuids
                    .get(&b_id.to_string())
                    .expect("nothing is generated, all is known."),
                b.as_ref().to_owned(),
                b.clone(),
            )
        })
        .collect::<Vec<_>>();

    // Stack-ids are duplicated just to indicate in which each branch-segment actually is.
    insta::assert_debug_snapshot!(branches, @r#"
    [
        (
            1,
            FullName(
                "refs/heads/A",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:59:17 +0000" },
                description: None,
                review: Review { pull_request: 12, review_id: None },
            },
        ),
        (
            2,
            FullName(
                "refs/heads/B-top",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:58:47 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            2,
            FullName(
                "refs/heads/B",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:58:47 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            2,
            FullName(
                "refs/heads/C-top-empty",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:58:47 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            2,
            FullName(
                "refs/heads/C-empty",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:58:47 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            3,
            FullName(
                "refs/heads/C-top",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:57:50 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            3,
            FullName(
                "refs/heads/C-middle",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:57:50 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            3,
            FullName(
                "refs/heads/C",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:57:50 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            3,
            FullName(
                "refs/heads/D-top-empty",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:57:50 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            3,
            FullName(
                "refs/heads/D-middle-empty",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:57:50 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            3,
            FullName(
                "refs/heads/D-empty",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:57:50 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            4,
            FullName(
                "refs/heads/D-top",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:59:48 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            4,
            FullName(
                "refs/heads/D",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 10:59:48 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
        (
            5,
            FullName(
                "refs/heads/E",
            ),
            Branch {
                ref_info: RefInfo { created_at: None, updated_at: "2025-02-24 11:00:01 +0000" },
                description: None,
                review: Review { pull_request: None, review_id: None },
            },
        ),
    ]
    "#);

    let toml_path = store.path().to_owned();
    assert!(toml_path.exists(), "the file is still present");
    let was_deleted = store.remove("refs/heads/gitbutler/workspace".try_into()?)?;
    assert!(was_deleted, "This basically clears out everything");
    assert!(!toml_path.exists(), "implemented brutally by file deletion");

    // Asking for the workspace
    let workspace = store.workspace("refs/heads/gitbutler/integration".try_into()?)?;
    assert!(
        workspace.is_default(),
        "The workspace was deleted so it doesn't exist anymore"
    );

    let was_deleted = store.remove("refs/heads/gitbutler/workspace".try_into()?)?;
    assert!(
        !was_deleted,
        "and clearing out everything can only happen once"
    );
    assert_eq!(
        store.iter().count(),
        0,
        "deleting the workspace deletes all stacks, at least in this backend"
    );

    drop(store);

    assert!(
        !toml_path.exists(),
        "It won't recreate a previously deleted file"
    );
    Ok(())
}

#[test]
fn create_workspace_and_stacks_with_branches_from_scratch_with_workspace_and_unapply()
-> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;
    store.data_mut().default_target = None;

    let ws_ref = "refs/heads/gitbutler/workspace".try_into()?;
    let mut ws_md = store.workspace(ws_ref)?;
    insta::assert_debug_snapshot!(ws_md.deref(), @r#"
    Workspace {
        ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
        stacks: [],
        target_ref: None,
        push_remote: None,
    }
    "#);

    let branch1: gix::refs::FullName = "refs/heads/in-workspace".try_into()?;
    let stack_id1 = StackId::from_number_for_testing(1);
    let branch2: gix::refs::FullName = "refs/heads/outside-workspace".try_into()?;
    let stack_id2 = StackId::from_number_for_testing(2);
    ws_md.stacks.push(WorkspaceStack {
        id: stack_id1,
        workspacecommit_relation: Merged,
        branches: vec![WorkspaceStackBranch {
            ref_name: branch1.clone(),
            archived: false,
        }],
    });
    ws_md.stacks.push(WorkspaceStack {
        id: stack_id2,
        workspacecommit_relation: Outside,
        branches: vec![WorkspaceStackBranch {
            ref_name: branch2.clone(),
            archived: false,
        }],
    });
    store.set_workspace(&ws_md)?;

    let ws_md = store.workspace(ws_ref)?;
    insta::assert_debug_snapshot!(ws_md.deref(), @r#"
    Workspace {
        ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
        stacks: [
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000001,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/in-workspace",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Merged,
            },
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000002,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/outside-workspace",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Outside,
            },
        ],
        target_ref: None,
        push_remote: None,
    }
    "#);

    let toml_path = store.path().to_owned();
    drop(store);

    let mut store = VirtualBranchesTomlMetadata::from_path(&toml_path)?;
    let mut ws_md = store.workspace(ws_ref)?;
    insta::assert_debug_snapshot!(ws_md.deref(), @r#"
    Workspace {
        ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
        stacks: [
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000001,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/in-workspace",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Merged,
            },
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000002,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/outside-workspace",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Outside,
            },
        ],
        target_ref: None,
        push_remote: None,
    }
    "#);

    ws_md.stacks[0].workspacecommit_relation = Outside;
    ws_md.stacks[1].workspacecommit_relation = Merged;

    // It's totally possible to change 'in_workspace' directly.
    store.set_workspace(&ws_md)?;
    let mut ws_md = store.workspace(ws_ref)?;
    insta::assert_debug_snapshot!(ws_md.deref(), @r#"
    Workspace {
        ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
        stacks: [
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000001,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/in-workspace",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Outside,
            },
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000002,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/outside-workspace",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Merged,
            },
        ],
        target_ref: None,
        push_remote: None,
    }
    "#);

    // Remotes can be part of the workspace as well.
    ws_md.stacks.clear();
    for (number, ref_name) in [
        (3, "refs/remotes/origin/feature"),
        (4, "refs/remotes/fork/other-feature"),
    ] {
        ws_md.stacks.push(WorkspaceStack {
            id: StackId::from_number_for_testing(number),
            workspacecommit_relation: Merged,
            branches: vec![WorkspaceStackBranch {
                ref_name: ref_name.try_into()?,
                archived: false,
            }],
        });
    }
    store.set_workspace(&ws_md)?;

    // We are NOT able to retrieve the original names as the backend can't capture it thanks to partial names and the
    // assumption that we never use remote branches directly.
    let ws_md = store.workspace(ws_ref)?;
    insta::assert_debug_snapshot!(ws_md.deref(), @r#"
    Workspace {
        ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
        stacks: [
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000003,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/origin/feature",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Merged,
            },
            WorkspaceStack {
                id: 00000000-0000-0000-0000-000000000004,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/fork/other-feature",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Merged,
            },
        ],
        target_ref: None,
        push_remote: None,
    }
    "#);

    Ok(())
}

#[test]
fn create_workspace_and_stacks_with_branches_from_scratch() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;
    store.data_mut().default_target = None;

    let toml_path = store.path().to_owned();
    let branch_name: gix::refs::FullName = "refs/heads/feat".try_into()?;
    let mut branch = store.branch(branch_name.as_ref())?;
    assert!(branch.is_default(), "nothing was there yet");
    assert!(!toml_path.exists(), "file wasn't written yet");
    assert_eq!(branch.stack_id(), None, "default values have no stack-id");

    branch.description = Some("mine".into());
    branch.review = but_core::ref_metadata::Review {
        pull_request: Some(42),
        review_id: Some("review-id".into()),
    };
    store.set_branch(&branch)?;
    let id = branch.stack_id().expect("now a stack-id was generated");

    let workspace_name: gix::refs::FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let mut ws = store.workspace(workspace_name.as_ref())?;
    assert!(
        !ws.is_default(),
        "the branch is auto-added to the workspace - even though it's not 'in_workspace'"
    );
    let actual = sanitize_uuids_and_timestamps(debug_str(&ws.stacks));
    insta::assert_snapshot!(actual, @r#"
    [
        WorkspaceStack {
            id: 1,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat",
                    archived: false,
                },
            ],
            workspacecommit_relation: Outside,
        },
    ]
    "#);
    // add the first branch to the workspace.
    let ignored_id = StackId::from_number_for_testing(2);
    ws.stacks.push(WorkspaceStack {
        id: ignored_id,
        workspacecommit_relation: Merged,
        branches: vec![WorkspaceStackBranch {
            ref_name: branch_name.clone(),
            archived: false,
        }],
    });
    store
        .set_workspace(&ws)
        .expect("This is the way to add branches");
    assert_eq!(ws.stack_id(), None);

    // Assure `ws` is what we think it should be - a single stack with one branch.
    let mut ws = store.workspace(workspace_name.as_ref())?;
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&ws.stacks));
    insta::assert_snapshot!(actual, @r#"
    [
        WorkspaceStack {
            id: 1,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ]
    "#);
    assert!(
        !uuids.contains_key(&ignored_id.to_string()),
        "it really is ignore"
    );
    assert!(
        uuids.contains_key(&id.to_string()),
        "the generated branch id was present though, it's the id of the stack"
    );

    // Put a new branch on top, changing the stack name
    let stacked_branch_name: gix::refs::FullName = "refs/heads/feat-on-top".try_into()?;
    ws.stacks[0].branches.insert(
        0,
        WorkspaceStackBranch {
            ref_name: stacked_branch_name.clone(),
            archived: false,
        },
    );
    assert_eq!(ws.stacks[0].ref_name(), Some(&stacked_branch_name));
    store
        .set_workspace(&ws)
        .expect("This is the way to add branches");

    let mut ws = store.workspace(workspace_name.as_ref())?;
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&ws.stacks));
    insta::assert_snapshot!(actual, @r#"
    [
        WorkspaceStack {
            id: 1,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat-on-top",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ]
    "#);
    assert!(
        uuids.contains_key(&id.to_string()),
        "the stack is still named after the first branch"
    );

    drop(store);

    assert!(toml_path.exists(), "file was written due to change");
    let (actual, uuids) =
        sanitize_uuids_and_timestamps_with_mapping(std::fs::read_to_string(&toml_path)?);
    insta::assert_snapshot!(actual, @r#"
    [branch_targets]

    [branches.1]
    id = "1"
    name = "feat-on-top"
    notes = ""
    created_timestamp_ms = 12345
    updated_timestamp_ms = 12345
    tree = "0000000000000000000000000000000000000000"
    head = "0000000000000000000000000000000000000000"
    ownership = ""
    order = 0
    allow_rebasing = true
    in_workspace = true
    post_commits = false

    [[branches.1.heads]]
    name = "feat"
    description = "mine"
    pr_number = 42
    archived = false
    review_id = "review-id"

    [branches.1.heads.head]
    CommitId = "0000000000000000000000000000000000000000"

    [[branches.1.heads]]
    name = "feat-on-top"
    archived = false

    [branches.1.heads.head]
    CommitId = "0000000000000000000000000000000000000000"
    "#);
    assert!(
        uuids.contains_key(&id.to_string()),
        "the written file also contains the id we have set for the first branch, which is a stack now."
    );

    let mut store = VirtualBranchesTomlMetadata::from_path(&toml_path)?;
    let new_ws = store.workspace(workspace_name.as_ref())?;
    assert_eq!(
        new_ws.deref(),
        ws.deref(),
        "It's still what it was before - it was persisted"
    );
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&new_ws.stacks));
    insta::assert_snapshot!(actual, @r#"
    [
        WorkspaceStack {
            id: 1,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat-on-top",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ]
    "#);
    assert!(
        uuids.contains_key(&id.to_string()),
        "after reading it back, the id is still used"
    );

    // Archived middle branch
    let archived_branch: gix::refs::FullName = "refs/heads/feat-in-middle".try_into()?;
    ws.stacks[0].branches.insert(
        1,
        WorkspaceStackBranch {
            ref_name: archived_branch.clone(),
            archived: true,
        },
    );
    store.set_workspace(&ws)?;
    let mut ws = store.workspace(workspace_name.as_ref())?;
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&ws.stacks));
    insta::assert_snapshot!(actual, @r#"
    [
        WorkspaceStack {
            id: 1,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat-on-top",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat-in-middle",
                    archived: true,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ]
    "#);
    assert!(uuids.contains_key(&id.to_string()));

    ws.stacks[0].branches[1].archived = false;
    store.set_workspace(&ws)?;
    let ws = store.workspace(ws.as_ref())?;
    assert!(
        !ws.stacks[0].branches[1].archived,
        "it's possible to turn the archived flag off on existing branches"
    );

    let second_stack: gix::refs::FullName = "refs/heads/second-stack".try_into()?;
    let mut branch = store.branch(second_stack.as_ref())?;
    branch.review.pull_request = Some(23);
    store.set_branch(&branch)?;

    let mut ws = store.workspace(ws.as_ref())?;
    assert_eq!(
        ws.stacks.len(),
        2,
        "The workspace is automatically updated, as we see out-of-workspace stacks"
    );
    // insert it as archived just because.
    let second_id = branch
        .stack_id()
        .expect("can also set a valid id, it doesn't matter");
    ws.stacks.push(WorkspaceStack {
        id: second_id,
        workspacecommit_relation: Merged,
        branches: vec![WorkspaceStackBranch {
            ref_name: branch.as_ref().into(), /* always a matching name */
            archived: true,
        }],
    });
    store.set_workspace(&ws)?;
    let mut ws = store.workspace(ws.as_ref())?;
    // Two stacks are present now.
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&ws.stacks));
    insta::assert_snapshot!(actual, @r#"
    [
        WorkspaceStack {
            id: 1,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat-on-top",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat-in-middle",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/feat",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
        WorkspaceStack {
            id: 2,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/second-stack",
                    archived: true,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ]
    "#);
    assert_eq!(uuids.len(), 2);
    assert!(uuids.contains_key(&id.to_string()));
    assert!(uuids.contains_key(&second_id.to_string()));

    ws.stacks.pop();
    store.set_workspace(&ws)?;
    let mut ws = store.workspace(ws.as_ref())?;
    assert_eq!(
        ws.stacks.len(),
        1,
        "The stack is still gone because we just removed it"
    );

    // Add it again, then remove it by removing the branch.
    ws.stacks.push(WorkspaceStack {
        id: StackId::from_number_for_testing(2),
        workspacecommit_relation: Merged,
        branches: vec![WorkspaceStackBranch {
            ref_name: second_stack.clone(),
            archived: true,
        }],
    });
    store.set_workspace(&ws)?;
    let ws = store.workspace(ws.as_ref())?;
    assert_eq!(
        ws.stacks.len(),
        2,
        "re-added second stack to be able to remove it again"
    );

    assert!(store.remove(second_stack.as_ref())?);
    let ws = store.workspace(ws.as_ref())?;
    assert_eq!(
        ws.stacks.len(),
        1,
        "second stack must have been removed -  a specialty of stacks implicitly defining the workspace."
    );

    // Remove everything
    assert!(
        store.remove(stacked_branch_name.as_ref())?,
        "there was something to remove"
    );
    assert!(
        !store.remove(stacked_branch_name.as_ref())?,
        "nothing left to remove"
    );
    assert!(
        store.remove(branch_name.as_ref())?,
        "there was something to remove, still"
    );
    assert!(
        !store.remove(branch_name.as_ref())?,
        "nothing left to remove"
    );
    assert!(store.remove(archived_branch.as_ref())?);

    let ws = store.workspace(workspace_name.as_ref())?;
    assert!(
        ws.is_default(),
        "it's empty, so no difference to a default one"
    );
    insta::assert_debug_snapshot!(ws.deref(), @r#"
    Workspace {
        ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
        stacks: [],
        target_ref: None,
        push_remote: None,
    }
    "#);

    drop(store);
    assert!(
        !toml_path.exists(),
        "if everything is just the default, the file is deleted on write"
    );

    let mut store = VirtualBranchesTomlMetadata::from_path(&toml_path)?;
    store.data_mut().default_target = Some(default_target());

    let toml_path = store.path().to_owned();
    let mut ws = store.workspace(workspace_name.as_ref())?;

    ws.push_remote = Some("push-remote".into());
    ws.target_ref = Some(gix::refs::FullName::try_from(
        "refs/remotes/new-origin/new-target",
    )?);
    store.set_workspace(&ws)?;

    drop(store);
    let (actual, _uuids) =
        sanitize_uuids_and_timestamps_with_mapping(std::fs::read_to_string(&toml_path)?);
    insta::assert_snapshot!(actual, @r#"
    [default_target]
    branchName = "new-target"
    remoteName = "new-origin"
    remoteUrl = "https://example.com/example-org/example-repo"
    sha = "0000000000000000000000000000000000000000"
    pushRemoteName = "push-remote"

    [branch_targets]

    [branches]
    "#);

    Ok(())
}

#[test]
fn target_journey() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;
    let ws_name = "refs/heads/gitbutler/workspace".try_into()?;
    let mut ws = store.workspace(ws_name)?;
    assert_eq!(
        ws.target_ref,
        Some("refs/remotes/origin/sub-name/main".try_into()?)
    );

    let expected_target: gix::refs::FullName = "refs/remotes/origin/main".try_into()?;
    ws.target_ref = Some(expected_target.clone());
    store.set_workspace(&ws)?;

    let mut ws = store.workspace(ws_name)?;
    assert_eq!(
        ws.target_ref,
        Some(expected_target.clone()),
        "can change the name as well"
    );

    ws.target_ref = None;
    store.set_workspace(&ws)?;

    let mut ws = store.workspace(ws_name)?;
    ws.target_ref = Some(expected_target);

    let err = store.set_workspace(&ws).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Cannot reasonably set a target in the old data structure as we don't have repo access here",
        "cannot do that as the data structures are too incompatible, can't set all values and certainly shouldn't make it up"
    );

    Ok(())
}

#[test]
fn create_workspace_from_scratch_workspace_first() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;
    let workspace_name = "refs/heads/gitbutler/integration".try_into()?;
    let mut ws = store.workspace(workspace_name)?;
    ws.stacks.push(WorkspaceStack {
        id: StackId::from_number_for_testing(1),
        workspacecommit_relation: Outside,
        branches: vec![
            WorkspaceStackBranch {
                ref_name: "refs/heads/top".try_into()?,
                archived: false,
            },
            WorkspaceStackBranch {
                ref_name: "refs/heads/one-below-top".try_into()?,
                archived: true,
            },
            WorkspaceStackBranch {
                ref_name: "refs/heads/base".try_into()?,
                archived: true,
            },
        ],
    });
    ws.stacks.push(WorkspaceStack {
        id: StackId::from_number_for_testing(2),
        workspacecommit_relation: Merged,
        branches: vec![WorkspaceStackBranch {
            ref_name: "refs/heads/second-branch".try_into()?,
            archived: false,
        }],
    });

    // This is still what was defined in memory, including our test-stack ids
    // which are respected.
    insta::assert_debug_snapshot!(ws.stacks, @r#"
    [
        WorkspaceStack {
            id: 00000000-0000-0000-0000-000000000001,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/top",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/one-below-top",
                    archived: true,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/base",
                    archived: true,
                },
            ],
            workspacecommit_relation: Outside,
        },
        WorkspaceStack {
            id: 00000000-0000-0000-0000-000000000002,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/second-branch",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ]
    "#);
    store.set_workspace(&ws)?;
    let stored_ws = store.workspace(workspace_name)?;
    assert_eq!(stored_ws.deref(), ws.deref());

    // Pop archived branch.
    ws.stacks[0].branches.pop();
    store.set_workspace(&ws)?;
    let mut ws = store.workspace(workspace_name)?;
    insta::assert_debug_snapshot!(ws.stacks, @r#"
    [
        WorkspaceStack {
            id: 00000000-0000-0000-0000-000000000001,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/top",
                    archived: false,
                },
                WorkspaceStackBranch {
                    ref_name: "refs/heads/one-below-top",
                    archived: true,
                },
            ],
            workspacecommit_relation: Outside,
        },
        WorkspaceStack {
            id: 00000000-0000-0000-0000-000000000002,
            branches: [
                WorkspaceStackBranch {
                    ref_name: "refs/heads/second-branch",
                    archived: false,
                },
            ],
            workspacecommit_relation: Merged,
        },
    ]
    "#);

    // Remove the last branch, but leave the stack.
    ws.stacks[1].branches.pop();

    let err = store.set_workspace(&ws).unwrap_err();
    assert_eq!(
        err.to_string(),
        "BUG: incoming stack is probably empty, caller should have removed the whole stack"
    );
    ws.stacks.pop();
    assert_eq!(ws.stacks.len(), 1);

    // The workspace is empty now, no sack left
    ws.stacks.pop();
    store.set_workspace(&ws)?;

    let stored_ws = store.workspace(workspace_name)?;
    assert_eq!(
        stored_ws.deref(),
        ws.deref(),
        "this state reproduces when queried, so no stack is left"
    );

    let toml_path = store.path().to_owned();
    drop(store);

    // Stacks are still there, but not in workspace, they carry data. But can't test it due to hashmap-instability.
    let mut store = VirtualBranchesTomlMetadata::from_path(toml_path)?;
    let stored_ws = store.workspace(workspace_name)?;
    assert_eq!(
        stored_ws.deref(),
        ws.deref(),
        "this state reproduces when queried after storage was reread, so no stack is left"
    );

    let below_top: &gix::refs::FullNameRef = "refs/heads/one-below-top".try_into()?;
    let branch = store.branch(below_top)?;
    assert!(
        branch.is_default(),
        "Workspace branches have been deleted, so they remain gone, and this branch was recreate."
    );
    // The stack with the branch now exists, and it is NOT in the workspace by default - this is a feature of
    // the implementation under test here, this data is disjoint otherwise.
    // By making it not in the workspace, users should be forced to not rely on this.
    store.set_branch(&branch)?;
    insta::assert_snapshot!(sanitize_uuids_and_timestamps(format!("{:#?}", store.workspace(workspace_name)?.deref())), @r#"
    Workspace {
        ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
        stacks: [
            WorkspaceStack {
                id: 1,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/one-below-top",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Outside,
            },
        ],
        target_ref: "refs/remotes/origin/sub-name/main",
        push_remote: None,
    }
    "#);

    // Create a branch implicitly, but turn it into a dependent branch later.
    let another_branch: &gix::refs::FullNameRef = "refs/heads/two-below-top".try_into()?;
    let branch = store.branch(another_branch)?;
    store.set_branch(&branch)?;

    let mut ws = store.workspace(workspace_name)?;
    let branch = ws.stacks[1].branches.pop().expect("exactly one branch");
    ws.stacks.pop();
    // Ordering also works
    ws.stacks[0].branches.insert(0, branch);
    store
        .set_workspace(&ws)
        .expect("setting the data works, despite having changed the branch association");
    insta::assert_snapshot!(sanitize_uuids_and_timestamps(format!("{:#?}", store.workspace(workspace_name)?.deref())), @r#"
    Workspace {
        ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
        stacks: [
            WorkspaceStack {
                id: 1,
                branches: [
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/two-below-top",
                        archived: false,
                    },
                    WorkspaceStackBranch {
                        ref_name: "refs/heads/one-below-top",
                        archived: false,
                    },
                ],
                workspacecommit_relation: Outside,
            },
        ],
        target_ref: "refs/remotes/origin/sub-name/main",
        push_remote: None,
    }
    "#);

    Ok(())
}

fn vb_fixture(name: &str) -> PathBuf {
    format!("tests/fixtures/legacy/{name}.toml").into()
}

fn vb_store_rw(name: &str) -> anyhow::Result<(VirtualBranchesTomlMetadata, TempDir)> {
    let tmp = TempDir::new()?;
    let writable_toml_path = tmp.path().join("vb.toml");
    std::fs::copy(vb_fixture(name), &writable_toml_path)?;

    let store = VirtualBranchesTomlMetadata::from_path(&writable_toml_path)?;
    Ok((store, tmp))
}

fn empty_vb_store_rw() -> anyhow::Result<(VirtualBranchesTomlMetadata, TempDir)> {
    let tmp = tempdir()?;
    let mut store = VirtualBranchesTomlMetadata::from_path(tmp.path().join("vb.toml"))?;
    store.data_mut().default_target = Some(default_target());
    Ok((store, tmp))
}

fn default_target() -> Target {
    Target {
        branch: RemoteRefname::new("origin/sub-name", "main"),
        remote_url: "https://example.com/example-org/example-repo".to_string(),
        sha: gix::hash::Kind::Sha1.null(),
        push_remote_name: None,
    }
}

/// Assure everything can round-trip and the data looks consistent, independently of the actual data,
/// from a store that already contains data.
fn roundtrip_journey(metadata: &mut impl RefMetadata) -> anyhow::Result<()> {
    // TODO: retrieve and set tests for all items, round-tripping
    let all_items = metadata.iter().map(Result::unwrap).collect::<Vec<_>>();
    for (ref_name, md) in &all_items {
        if let Some(ws_from_iter) = md.downcast_ref::<but_core::ref_metadata::Workspace>() {
            let ws = metadata.workspace(ref_name.as_ref())?;
            assert!(!ws.is_default(), "default data won't be iterated");
            if let Err(err) = metadata.set_workspace(&ws)
                && err.to_string().contains("unsupported")
            {
                continue;
            }
            assert_eq!(
                metadata.workspace(ref_name.as_ref())?.deref(),
                ws_from_iter,
                "nothing should change, it's a no-op"
            );
        } else if let Some(br_from_iter) = md.downcast_ref::<but_core::ref_metadata::Branch>() {
            let br = metadata.branch(ref_name.as_ref())?;
            assert!(!br.is_default(), "default data won't be iterated");
            metadata
                .set_branch(&br)
                .expect("updates have no reason to fail, even if no-op");
            assert_eq!(
                metadata.branch(ref_name.as_ref())?.deref(),
                br_from_iter,
                "nothing should change, it's a no-op"
            );
        }
    }

    for (ref_name, _md) in all_items {
        metadata.remove(ref_name.as_ref())?;
    }
    assert_eq!(metadata.iter().count(), 0, "Nothing is left after deletion");
    Ok(())
}
