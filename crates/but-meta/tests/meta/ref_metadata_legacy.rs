use std::{ops::Deref, path::PathBuf, str::FromStr};

use but_core::ref_metadata::{
    StackId,
    WorkspaceCommitRelation::{Merged, Outside},
    WorkspaceStack, WorkspaceStackBranch,
};
use but_meta::virtual_branches_legacy_types::{Stack as LegacyStack, StackBranch};
use but_testsupport::{
    debug_str,
    gix_testtools::tempfile::{TempDir, tempdir},
    sanitize_uuids_and_timestamps, sanitize_uuids_and_timestamps_with_mapping,
};
use snapbox::prelude::*;

#[test]
fn journey() -> anyhow::Result<()> {
    let (mut db, tmp) = vb_store_rw("virtual-branches-01")?;
    roundtrip_journey(&mut db)?;
    drop(db);
    let db = but_db::DbHandle::new_in_directory(tmp.path())?;
    assert!(
        db.meta()?.branches().next().is_none(),
        "metadata removals survive reopening"
    );
    assert!(
        !tmp.path().join("vb.toml").exists(),
        "metadata writes never create a TOML mirror"
    );
    Ok(())
}

#[test]
fn managed_workspace_order_is_available_to_ad_hoc_workspaces() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let mut store = but_db::DbHandle::new_in_directory(tmp.path())?;
    let workspace_name: gix::refs::FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let mut workspace = store.meta()?.workspace(workspace_name.as_ref())?;
    let refs = ["C", "A", "D", "B"]
        .map(|name| format!("refs/heads/{name}").try_into())
        .into_iter()
        .collect::<Result<Vec<gix::refs::FullName>, _>>()?;
    workspace.stacks = vec![
        WorkspaceStack {
            id: StackId::from_number_for_testing(1),
            workspacecommit_relation: Merged,
            branches: refs[..3]
                .iter()
                .cloned()
                .map(|ref_name| WorkspaceStackBranch {
                    ref_name,
                    archived: false,
                })
                .collect(),
        },
        WorkspaceStack {
            id: StackId::from_number_for_testing(2),
            workspacecommit_relation: Merged,
            branches: vec![WorkspaceStackBranch {
                ref_name: refs[3].clone(),
                archived: false,
            }],
        },
    ];

    store.meta_mut()?.set_workspace(&workspace)?;

    assert_eq!(
        store.meta()?.branch_stack_order(refs[1].as_ref())?,
        Some(refs[..3].to_vec()),
        "managed stack order should be reusable after checking out its middle branch"
    );
    assert_eq!(
        store.meta()?.branch_stack_order(refs[3].as_ref())?,
        Some(vec![refs[3].clone()]),
        "independent stacks should remain independent"
    );
    Ok(())
}

#[test]
fn read_only() -> anyhow::Result<()> {
    let (mut store, _tmp) = vb_store_rw("virtual-branches-01")?;
    let ws = store
        .meta()?
        .workspace("refs/heads/gitbutler/workspace".try_into()?)?;
    assert!(!ws.is_default(), "value read from file");
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&ws.stacks));
    snapbox::assert_data_eq!(
        actual,
        snapbox::str![[r#"
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
"#]]
    );

    for uuid in uuids.keys() {
        assert_ne!(
            ws.stacks.iter().find(|s| s.id == uuid.parse().unwrap()),
            None,
            "each UUID is available as workspace stack."
        );
    }

    let meta = store.meta()?;
    let branches = ws
        .stacks
        .iter()
        .flat_map(|stack| &stack.branches)
        .map(|branch| {
            let b = meta
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
    snapbox::assert_data_eq!(
        branches.to_debug(),
        snapbox::str![[r#"
[
    (
        1,
        FullName(
            "refs/heads/A",
        ),
        Branch {
            ref_info: RefInfo { created_at: None, updated_at: None },
            review: Review { pull_request: 12, review_id: None },
        },
    ),
    (
        2,
        FullName(
            "refs/heads/B-top",
        ),
        Branch,
    ),
    (
        2,
        FullName(
            "refs/heads/B",
        ),
        Branch,
    ),
    (
        2,
        FullName(
            "refs/heads/C-top-empty",
        ),
        Branch,
    ),
    (
        2,
        FullName(
            "refs/heads/C-empty",
        ),
        Branch,
    ),
    (
        3,
        FullName(
            "refs/heads/C-top",
        ),
        Branch,
    ),
    (
        3,
        FullName(
            "refs/heads/C-middle",
        ),
        Branch,
    ),
    (
        3,
        FullName(
            "refs/heads/C",
        ),
        Branch,
    ),
    (
        3,
        FullName(
            "refs/heads/D-top-empty",
        ),
        Branch,
    ),
    (
        3,
        FullName(
            "refs/heads/D-middle-empty",
        ),
        Branch,
    ),
    (
        3,
        FullName(
            "refs/heads/D-empty",
        ),
        Branch,
    ),
    (
        4,
        FullName(
            "refs/heads/D-top",
        ),
        Branch,
    ),
    (
        4,
        FullName(
            "refs/heads/D",
        ),
        Branch,
    ),
    (
        5,
        FullName(
            "refs/heads/E",
        ),
        Branch,
    ),
]

"#]]
    );

    let was_deleted = store
        .meta_mut()?
        .remove("refs/heads/gitbutler/workspace".try_into()?)?;
    assert!(was_deleted, "deleting workspace metadata clears its stacks");

    // Asking for the workspace
    let ws = store
        .meta()?
        .workspace("refs/heads/gitbutler/integration".try_into()?)?;
    assert!(
        ws.is_default(),
        "The workspace was deleted so it doesn't exist anymore"
    );

    let was_deleted = store
        .meta_mut()?
        .remove("refs/heads/gitbutler/workspace".try_into()?)?;
    assert!(
        !was_deleted,
        "and clearing out everything can only happen once"
    );
    assert_eq!(
        store.meta()?.workspaces().count() + store.meta()?.branches().count(),
        0,
        "deleting the workspace deletes all stacks, at least in this backend"
    );

    Ok(())
}

#[test]
fn create_workspace_and_stacks_with_branches_from_scratch_with_workspace_and_unapply()
-> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;

    let ws_ref = "refs/heads/gitbutler/workspace".try_into()?;
    let mut ws_md = store.meta()?.workspace(ws_ref)?;
    snapbox::assert_data_eq!(
        ws_md.deref().to_debug(),
        snapbox::str![[r#"
Workspace {
    ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
    stacks: [],
}

"#]]
    );

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
    store.meta_mut()?.set_workspace(&ws_md)?;

    let ws_md = store.meta()?.workspace(ws_ref)?;
    snapbox::assert_data_eq!(
        ws_md.deref().to_debug(),
        snapbox::str![[r#"
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
}

"#]]
    );

    let database_path = but_db::DbHandle::db_file_path(_tmp.path());
    drop(store);

    let mut store = but_db::DbHandle::new_at_path(&database_path)?;
    let mut ws_md = store.meta()?.workspace(ws_ref)?;
    snapbox::assert_data_eq!(
        ws_md.deref().to_debug(),
        snapbox::str![[r#"
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
}

"#]]
    );

    ws_md.stacks[0].workspacecommit_relation = Outside;
    ws_md.stacks[1].workspacecommit_relation = Merged;

    // It's totally possible to change 'in_workspace' directly.
    store.meta_mut()?.set_workspace(&ws_md)?;
    let mut ws_md = store.meta()?.workspace(ws_ref)?;
    snapbox::assert_data_eq!(
        ws_md.deref().to_debug(),
        snapbox::str![[r#"
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
}

"#]]
    );

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
    store.meta_mut()?.set_workspace(&ws_md)?;

    // We are NOT able to retrieve the original names as the backend can't capture it thanks to partial names and the
    // assumption that we never use remote branches directly.
    let ws_md = store.meta()?.workspace(ws_ref)?;
    snapbox::assert_data_eq!(
        ws_md.deref().to_debug(),
        snapbox::str![[r#"
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
}

"#]]
    );

    Ok(())
}

#[test]
fn set_workspace_stack_only_changes_persist() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;
    let mut data = payload(&store)?;
    let first_stack_id = StackId::from_number_for_testing(1);
    let second_stack_id = StackId::from_number_for_testing(2);
    let first_head = gix::ObjectId::from_str("1111111111111111111111111111111111111111")?;
    let second_head = gix::ObjectId::from_str("2222222222222222222222222222222222222222")?;

    let mut first_stack = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head: first_head,
            name: "first".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        0,
        true,
    );
    first_stack.id = first_stack_id;
    let mut second_stack = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head: second_head,
            name: "second".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        1,
        true,
    );
    second_stack.id = second_stack_id;

    data.branches.insert(first_stack_id, first_stack);
    data.branches.insert(second_stack_id, second_stack);
    set_payload(&mut store, &data)?;

    let database_path = but_db::DbHandle::db_file_path(_tmp.path());
    drop(store);

    let ws_ref: gix::refs::FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let mut store = but_db::DbHandle::new_at_path(&database_path)?;
    let mut ws = store.meta()?.workspace(ws_ref.as_ref())?;
    assert_eq!(ws.stacks.len(), 2, "fixture starts with both stacks");
    ws.stacks.retain(|stack| stack.id == first_stack_id);
    store.meta_mut()?.set_workspace(&ws)?;
    drop(store);

    let store = but_db::DbHandle::new_at_path(&database_path)?;
    let ws = store.meta()?.workspace(ws_ref.as_ref())?;
    assert_eq!(
        ws.stacks.len(),
        1,
        "stack-only workspace metadata changes must be persisted"
    );
    assert_eq!(ws.stacks[0].id, first_stack_id);
    Ok(())
}

#[test]
fn create_workspace_and_stacks_with_branches_from_scratch() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;

    let database_path = but_db::DbHandle::db_file_path(_tmp.path());
    let branch_name: gix::refs::FullName = "refs/heads/feat".try_into()?;
    let mut branch = store.meta()?.branch(branch_name.as_ref())?;
    assert!(branch.is_default(), "nothing was there yet");
    assert!(
        store.virtual_branches().get_snapshot()?.is_none(),
        "opening the database does not manufacture metadata"
    );
    assert_eq!(branch.stack_id(), None, "default values have no stack-id");

    branch.review = but_core::ref_metadata::Review {
        pull_request: Some(42),
        review_id: Some("review-id".into()),
    };
    store.meta_mut()?.set_branch(&branch)?;
    let branch = store.meta()?.branch(branch_name.as_ref())?;
    let id = branch.stack_id().expect("now a stack-id was generated");

    let workspace_name: gix::refs::FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let mut ws = store.meta()?.workspace(workspace_name.as_ref())?;
    assert!(
        !ws.is_default(),
        "the branch is auto-added to the workspace - even though it's not 'in_workspace'"
    );
    let actual = sanitize_uuids_and_timestamps(debug_str(&ws.stacks));
    snapbox::assert_data_eq!(
        actual,
        snapbox::str![[r#"
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
"#]]
    );
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
        .meta_mut()?
        .set_workspace(&ws)
        .expect("This is the way to add branches");
    assert_eq!(ws.stack_id(), None);

    // Assure `ws` is what we think it should be - a single stack with one branch.
    let mut ws = store.meta()?.workspace(workspace_name.as_ref())?;
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&ws.stacks));
    snapbox::assert_data_eq!(
        actual,
        snapbox::str![[r#"
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
"#]]
    );
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
        .meta_mut()?
        .set_workspace(&ws)
        .expect("This is the way to add branches");

    let mut ws = store.meta()?.workspace(workspace_name.as_ref())?;
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&ws.stacks));
    snapbox::assert_data_eq!(
        actual,
        snapbox::str![[r#"
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
"#]]
    );
    assert!(
        uuids.contains_key(&id.to_string()),
        "the stack is still named after the first branch"
    );

    let archived = toml::to_string(&payload(&store)?)?;
    drop(store);
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(archived);
    snapbox::assert_data_eq!(
        actual,
        snapbox::str![[r#"
[branches.1]
id = "1"
order = 0
in_workspace = true
notes = ""
ownership = ""
allow_rebasing = true
post_commits = false
tree = "0000000000000000000000000000000000000000"
created_timestamp_ms = "0"
updated_timestamp_ms = "0"
name = ""
head = "0000000000000000000000000000000000000000"

[[branches.1.heads]]
name = "feat"
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

"#]]
    );
    assert!(
        uuids.contains_key(&id.to_string()),
        "the written file also contains the id we have set for the first branch, which is a stack now."
    );

    let mut store = but_db::DbHandle::new_at_path(&database_path)?;
    let new_ws = store.meta()?.workspace(workspace_name.as_ref())?;
    assert_eq!(
        new_ws.deref(),
        ws.deref(),
        "It's still what it was before - it was persisted"
    );
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&new_ws.stacks));
    snapbox::assert_data_eq!(
        actual,
        snapbox::str![[r#"
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
"#]]
    );
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
    store.meta_mut()?.set_workspace(&ws)?;
    let mut ws = store.meta()?.workspace(workspace_name.as_ref())?;
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&ws.stacks));
    snapbox::assert_data_eq!(
        actual,
        snapbox::str![[r#"
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
"#]]
    );
    assert!(uuids.contains_key(&id.to_string()));

    ws.stacks[0].branches[1].archived = false;
    store.meta_mut()?.set_workspace(&ws)?;
    let ws = store.meta()?.workspace(ws.as_ref())?;
    assert!(
        !ws.stacks[0].branches[1].archived,
        "it's possible to turn the archived flag off on existing branches"
    );

    let second_stack: gix::refs::FullName = "refs/heads/second-stack".try_into()?;
    let mut branch = store.meta()?.branch(second_stack.as_ref())?;
    branch.review.pull_request = Some(23);
    store.meta_mut()?.set_branch(&branch)?;
    let branch = store.meta()?.branch(second_stack.as_ref())?;

    let mut ws = store.meta()?.workspace(ws.as_ref())?;
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
    store.meta_mut()?.set_workspace(&ws)?;
    let mut ws = store.meta()?.workspace(ws.as_ref())?;
    // Two stacks are present now.
    let (actual, uuids) = sanitize_uuids_and_timestamps_with_mapping(debug_str(&ws.stacks));
    snapbox::assert_data_eq!(
        actual,
        snapbox::str![[r#"
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
"#]]
    );
    assert_eq!(uuids.len(), 2);
    assert!(uuids.contains_key(&id.to_string()));
    assert!(uuids.contains_key(&second_id.to_string()));

    ws.stacks.pop();
    store.meta_mut()?.set_workspace(&ws)?;
    let mut ws = store.meta()?.workspace(ws.as_ref())?;
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
    store.meta_mut()?.set_workspace(&ws)?;
    let ws = store.meta()?.workspace(ws.as_ref())?;
    assert_eq!(
        ws.stacks.len(),
        2,
        "re-added second stack to be able to remove it again"
    );

    assert!(store.meta_mut()?.remove(second_stack.as_ref())?);
    let ws = store.meta()?.workspace(ws.as_ref())?;
    assert_eq!(
        ws.stacks.len(),
        1,
        "second stack must have been removed -  a specialty of stacks implicitly defining the workspace."
    );

    // Remove everything
    assert!(
        store.meta_mut()?.remove(stacked_branch_name.as_ref())?,
        "there was something to remove"
    );
    assert!(
        !store.meta_mut()?.remove(stacked_branch_name.as_ref())?,
        "nothing left to remove"
    );
    assert!(
        store.meta_mut()?.remove(branch_name.as_ref())?,
        "there was something to remove, still"
    );
    assert!(
        !store.meta_mut()?.remove(branch_name.as_ref())?,
        "nothing left to remove"
    );
    assert!(store.meta_mut()?.remove(archived_branch.as_ref())?);

    let ws = store.meta()?.workspace(workspace_name.as_ref())?;
    assert!(
        ws.is_default(),
        "it's empty, so no difference to a default one"
    );
    snapbox::assert_data_eq!(
        ws.deref().to_debug(),
        snapbox::str![[r#"
Workspace {
    ref_info: RefInfo { created_at: "2023-01-31 14:55:57 +0000", updated_at: None },
    stacks: [],
}

"#]]
    );

    drop(store);
    let store = but_db::DbHandle::new_at_path(&database_path)?;
    assert!(
        store.meta()?.branches().next().is_none(),
        "all branches remain deleted after reopening"
    );

    Ok(())
}

#[test]
fn create_workspace_from_scratch_workspace_first() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;
    let workspace_name = "refs/heads/gitbutler/integration".try_into()?;
    let mut ws = store.meta()?.workspace(workspace_name)?;
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
    snapbox::assert_data_eq!(
        ws.stacks.to_debug(),
        snapbox::str![[r#"
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

"#]]
    );
    store.meta_mut()?.set_workspace(&ws)?;
    let stored_ws = store.meta()?.workspace(workspace_name)?;
    assert_eq!(stored_ws.deref(), ws.deref());

    // Pop archived branch.
    ws.stacks[0].branches.pop();
    store.meta_mut()?.set_workspace(&ws)?;
    let mut ws = store.meta()?.workspace(workspace_name)?;
    snapbox::assert_data_eq!(
        ws.stacks.to_debug(),
        snapbox::str![[r#"
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

"#]]
    );

    // Remove the last branch, but leave the stack.
    ws.stacks[1].branches.pop();

    let err = store.meta_mut()?.set_workspace(&ws).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Cannot save an empty metadata stack",
        "empty stacks must be removed instead of persisted"
    );
    ws.stacks.pop();
    assert_eq!(ws.stacks.len(), 1);

    // The workspace is empty now, no sack left
    ws.stacks.pop();
    store.meta_mut()?.set_workspace(&ws)?;

    let stored_ws = store.meta()?.workspace(workspace_name)?;
    assert_eq!(
        stored_ws.deref(),
        ws.deref(),
        "this state reproduces when queried, so no stack is left"
    );

    let database_path = but_db::DbHandle::db_file_path(_tmp.path());
    drop(store);

    // Stacks are still there, but not in workspace, they carry data. But can't test it due to hashmap-instability.
    let mut store = but_db::DbHandle::new_at_path(database_path)?;
    let stored_ws = store.meta()?.workspace(workspace_name)?;
    assert_eq!(
        stored_ws.deref(),
        ws.deref(),
        "this state reproduces when queried after storage was reread, so no stack is left"
    );

    let below_top: &gix::refs::FullNameRef = "refs/heads/one-below-top".try_into()?;
    let branch = store.meta()?.branch(below_top)?;
    assert!(
        branch.is_default(),
        "Workspace branches have been deleted, so they remain gone, and this branch was recreate."
    );
    // The stack with the branch now exists, and it is NOT in the workspace by default - this is a feature of
    // the implementation under test here, this data is disjoint otherwise.
    // By making it not in the workspace, users should be forced to not rely on this.
    store.meta_mut()?.set_branch(&branch)?;
    snapbox::assert_data_eq!(
        sanitize_uuids_and_timestamps(format!(
            "{:#?}",
            store.meta()?.workspace(workspace_name)?.deref()
        )),
        snapbox::str![[r#"
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
}
"#]]
    );

    // Create a branch implicitly, but turn it into a dependent branch later.
    let another_branch: &gix::refs::FullNameRef = "refs/heads/two-below-top".try_into()?;
    let branch = store.meta()?.branch(another_branch)?;
    store.meta_mut()?.set_branch(&branch)?;

    let mut ws = store.meta()?.workspace(workspace_name)?;
    let branch = ws.stacks[1].branches.pop().expect("exactly one branch");
    ws.stacks.pop();
    // Ordering also works
    ws.stacks[0].branches.insert(0, branch);
    store
        .meta_mut()?
        .set_workspace(&ws)
        .expect("setting the data works, despite having changed the branch association");
    snapbox::assert_data_eq!(
        sanitize_uuids_and_timestamps(format!(
            "{:#?}",
            store.meta()?.workspace(workspace_name)?.deref()
        )),
        snapbox::str![[r#"
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
}
"#]]
    );

    Ok(())
}

fn vb_fixture(name: &str) -> PathBuf {
    format!("tests/fixtures/legacy/{name}.toml").into()
}

fn vb_store_rw(name: &str) -> anyhow::Result<(but_db::DbHandle, TempDir)> {
    let (mut store, tmp) = empty_vb_store_rw()?;
    let data = toml::from_str(&std::fs::read_to_string(vb_fixture(name))?)?;
    set_payload(&mut store, &data)?;
    Ok((store, tmp))
}

#[test]
fn legacy_target_is_ignored_and_not_written() -> anyhow::Result<()> {
    let data: but_meta::virtual_branches_legacy_types::VirtualBranches =
        toml::from_str(&std::fs::read_to_string(vb_fixture("virtual-branches-01"))?)?;
    let canonical = toml::to_string(&data)?;
    assert!(
        !canonical.contains("[default_target]"),
        "canonical TOML omits the legacy project target"
    );
    Ok(())
}

#[test]
fn rename_onto_an_existing_branch_is_rejected() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;

    let a: gix::refs::FullName = "refs/heads/a".try_into()?;
    let b: gix::refs::FullName = "refs/heads/b".try_into()?;

    // Persist two distinct branches (each ends up in its own stack).
    for name in [&a, &b] {
        let mut branch = store.meta()?.branch(name.as_ref())?;
        branch.review.pull_request = Some(1);
        store.meta_mut()?.set_branch(&branch)?;
    }

    // Renaming `a` onto the existing `b` must be rejected rather than creating a duplicate head.
    let err = store
        .meta_mut()?
        .rename(a.as_ref(), b.as_ref())
        .expect_err("cannot rename onto an existing branch");
    assert!(err.to_string().contains("already exists"), "{err}");
    assert!(store.meta()?.branch_opt(a.as_ref())?.is_some());
    assert!(store.meta()?.branch_opt(b.as_ref())?.is_some());

    // Renaming onto a fresh name works and moves the metadata in place.
    let c: gix::refs::FullName = "refs/heads/c".try_into()?;
    store.meta_mut()?.rename(a.as_ref(), c.as_ref())?;
    assert!(store.meta()?.branch_opt(a.as_ref())?.is_none());
    assert!(store.meta()?.branch_opt(c.as_ref())?.is_some());

    // Renaming a branch onto its own name is a no-op, not a self-conflict.
    store.meta_mut()?.rename(c.as_ref(), c.as_ref())?;
    assert!(store.meta()?.branch_opt(c.as_ref())?.is_some());

    Ok(())
}

fn empty_vb_store_rw() -> anyhow::Result<(but_db::DbHandle, TempDir)> {
    let tmp = tempdir()?;
    let store = but_db::DbHandle::new_in_directory(tmp.path())?;
    Ok((store, tmp))
}

fn payload(
    db: &but_db::DbHandle,
) -> anyhow::Result<but_meta::virtual_branches_legacy_types::VirtualBranches> {
    but_meta::legacy_storage::snapshot_to_legacy(
        &db.virtual_branches().get_snapshot()?.unwrap_or_default(),
    )
}

fn set_payload(
    db: &mut but_db::DbHandle,
    data: &but_meta::virtual_branches_legacy_types::VirtualBranches,
) -> anyhow::Result<()> {
    db.meta_mut()?
        .replace_snapshot(&but_meta::legacy_storage::legacy_to_snapshot(data)?)?;
    Ok(())
}

fn roundtrip_journey(db: &mut but_db::DbHandle) -> anyhow::Result<()> {
    let metadata = db.meta()?;
    for (name, expected) in metadata.workspaces() {
        let workspace = db.meta()?.workspace(name.as_ref())?;
        db.meta_mut()?.set_workspace(&workspace)?;
        assert_eq!(
            *db.meta()?.workspace(name.as_ref())?,
            expected,
            "workspace roundtrip preserves data"
        );
    }
    for (name, expected) in metadata.branches() {
        let branch = db.meta()?.branch(name.as_ref())?;
        db.meta_mut()?.set_branch(&branch)?;
        assert_eq!(
            *db.meta()?.branch(name.as_ref())?,
            expected,
            "branch roundtrip preserves data"
        );
    }
    for (name, _) in metadata.workspaces() {
        db.meta_mut()?.remove(name.as_ref())?;
    }
    assert!(
        db.meta()?.branches().next().is_none(),
        "workspace deletion removes its branches"
    );
    Ok(())
}

#[test]
fn legacy_change_id_deserializes_as_null_sha() -> anyhow::Result<()> {
    // The fixture contains a legacy ChangeId which should deserialize as a null SHA.
    // This allows old toml files with ChangeId entries to be loaded without errors.
    let (store, _tmp) = vb_store_rw("legacy-change-id")?;

    // Use a valid UUID for the stack ID that matches the fixture
    let test_stack_id = "12345678-1234-5678-1234-567812345678";

    // Verify that the legacy ChangeId was deserialized as a null SHA
    let data = payload(&store)?;
    let stack = data
        .branches
        .get(&but_core::ref_metadata::StackId::from_str(test_stack_id).unwrap())
        .expect("stack should exist");

    assert_eq!(stack.heads.len(), 1, "should have deserialized one head");

    assert_eq!(
        stack.heads[0].head,
        gix::hash::Kind::Sha1.null(),
        "legacy ChangeId should deserialize as null SHA to allow loading old toml files"
    );

    Ok(())
}

#[test]
fn garbage_collect_removes_outside_workspace_stack_at_target() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("dlib-standin")?;
    let target = repo.head_id()?.detach();
    let (mut store, _tmp) = empty_vb_store_rw()?;
    let mut data = payload(&store)?;

    let workspace_stack = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head: target,
            name: "kept".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        0,
        true,
    );
    let outside_stack = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head: target,
            name: "collected".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        1,
        false,
    );
    let workspace_stack_id = workspace_stack.id;
    let outside_stack_id = outside_stack.id;
    data.branches.insert(workspace_stack_id, workspace_stack);
    data.branches.insert(outside_stack_id, outside_stack);

    set_payload(&mut store, &data)?;
    but_meta::garbage_collect(&repo, &project_meta(target), &mut store.connection_mut())?;

    assert!(payload(&store)?.branches.contains_key(&workspace_stack_id));
    assert!(!payload(&store)?.branches.contains_key(&outside_stack_id));
    Ok(())
}

#[test]
fn garbage_collect_removes_outside_workspace_stack_with_missing_head() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("dlib-standin")?;
    let target = repo.head_id()?.detach();
    let missing_head = gix::ObjectId::from_hex(b"30696678319e0fa3a20e54f22d47fc8cf1ceaade")?;
    let (mut store, _tmp) = empty_vb_store_rw()?;
    let mut data = payload(&store)?;
    let outside_stack = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head: missing_head,
            name: "missing".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        0,
        false,
    );
    let outside_stack_id = outside_stack.id;
    data.branches.insert(outside_stack_id, outside_stack);

    set_payload(&mut store, &data)?;
    but_meta::garbage_collect(&repo, &project_meta(target), &mut store.connection_mut())?;

    assert!(!payload(&store)?.branches.contains_key(&outside_stack_id));
    Ok(())
}

#[test]
fn garbage_collect_removes_outside_workspace_stack_with_broken_ref() -> anyhow::Result<()> {
    let (mut repo, _tmp) = but_testsupport::writable_scenario("dlib-standin");
    let target = repo.head_id()?.detach();
    let missing_head = gix::ObjectId::from_hex(b"30696678319e0fa3a20e54f22d47fc8cf1ceaade")?;
    repo.reference(
        "refs/heads/missing",
        missing_head,
        gix::refs::transaction::PreviousValue::Any,
        "test",
    )?;
    repo.reload()?;
    let (mut store, _tmp) = empty_vb_store_rw()?;
    let mut data = payload(&store)?;
    let outside_stack = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head: target,
            name: "missing".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        0,
        false,
    );
    let outside_stack_id = outside_stack.id;
    data.branches.insert(outside_stack_id, outside_stack);

    set_payload(&mut store, &data)?;
    but_meta::garbage_collect(&repo, &project_meta(target), &mut store.connection_mut())?;

    assert!(!payload(&store)?.branches.contains_key(&outside_stack_id));
    Ok(())
}

#[test]
fn garbage_collection_preserves_retained_database_rows() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("dlib-standin")?;
    let target = repo.head_id()?.detach();
    let mut store = but_testsupport::in_memory_db();
    let kept_id = StackId::from_number_for_testing(1).to_string();
    let removed_id = StackId::from_number_for_testing(2).to_string();
    let kept = but_db::VbStack {
        id: kept_id.clone(),
        source_refname: Some("opaque historical reference".into()),
        upstream_remote_name: Some("origin".into()),
        upstream_branch_name: Some("kept".into()),
        sort_order: 37,
        in_workspace: true,
        legacy_name: "kept".into(),
        legacy_notes: "historical notes".into(),
        legacy_ownership: "opaque historical ownership".into(),
        legacy_allow_rebasing: false,
        legacy_post_commits: true,
        legacy_tree_sha: "opaque historical tree".into(),
        legacy_head_sha: "opaque historical head".into(),
        legacy_created_timestamp_ms: "opaque historical timestamp".into(),
        legacy_updated_timestamp_ms: "another historical timestamp".into(),
    };
    let kept_head = but_db::VbStackHead {
        stack_id: kept_id.clone(),
        position: 7,
        name: "kept".into(),
        head_sha: target.to_string(),
        pr_number: Some(42),
        archived: true,
        review_id: Some("review".into()),
    };
    let mut snapshot = but_db::VirtualBranchesSnapshot {
        state: but_db::VbState {
            initialized: true,
            last_pushed_base_sha: Some(target.to_string()),
            toml_last_seen_mtime_ns: Some(123),
            toml_last_seen_sha256: Some("historical sync hash".into()),
        },
        stacks: vec![
            kept.clone(),
            but_db::VbStack {
                id: removed_id.clone(),
                in_workspace: false,
                ..kept
            },
        ],
        heads: vec![
            kept_head.clone(),
            but_db::VbStackHead {
                stack_id: removed_id,
                name: "collected".into(),
                ..kept_head
            },
        ],
    };
    store.meta_mut()?.replace_snapshot(&snapshot)?;

    but_meta::garbage_collect(&repo, &project_meta(target), &mut store.connection_mut())?;

    snapshot.stacks.retain(|stack| stack.id == kept_id);
    snapshot.heads.retain(|head| head.stack_id == kept_id);
    assert_eq!(
        store.virtual_branches().get_snapshot()?,
        Some(snapshot),
        "collection removes only the obsolete stack and its heads without decoding or normalizing retained fields"
    );
    Ok(())
}

/// A stale unapplied stack may still list a branch that meanwhile lives in an applied
/// stack - `reconcile_projected_stacks` tolerates such duplicates as stale hints.
/// Writing the workspace back must not move the applied stack's copy into the stale
/// stack; that regrouping made snapshots record every branch below the tip as
/// unapplied, so restoring them stranded those branches (#15573).
#[test]
fn duplicate_names_in_unapplied_stacks_do_not_steal_applied_branches() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;
    let mut data = payload(&store)?;
    let lower_head = gix::ObjectId::from_str("1111111111111111111111111111111111111111")?;
    let tip_head = gix::ObjectId::from_str("2222222222222222222222222222222222222222")?;
    let stale_head = gix::ObjectId::from_str("3333333333333333333333333333333333333333")?;

    let applied = LegacyStack::new_with_just_heads(
        vec![
            StackBranch {
                head: lower_head,
                name: "lower".into(),
                pr_number: None,
                archived: false,
                review_id: None,
            },
            StackBranch {
                head: tip_head,
                name: "tip".into(),
                pr_number: None,
                archived: false,
                review_id: None,
            },
        ],
        0,
        true,
    );
    let stale = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head: stale_head,
            name: "lower".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        1,
        false,
    );
    let applied_id = applied.id;
    let stale_id = stale.id;
    data.branches.insert(applied_id, applied);
    data.branches.insert(stale_id, stale);

    // Reading the workspace and writing it back unchanged must not regroup branches.
    set_payload(&mut store, &data)?;
    let workspace_name: gix::refs::FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let ws = store.meta()?.workspace(workspace_name.as_ref())?;
    store.meta_mut()?.set_workspace(&ws)?;

    let names = |stack_id: StackId| {
        payload(&store)
            .unwrap()
            .branches
            .get(&stack_id)
            .map(|stack| {
                stack
                    .heads
                    .iter()
                    .map(|head| head.name.clone())
                    .collect::<Vec<_>>()
            })
    };
    assert_eq!(
        names(applied_id),
        Some(vec!["lower".to_string(), "tip".to_string()]),
        "the applied stack keeps all its branches",
    );
    assert_eq!(
        names(stale_id),
        Some(vec!["lower".to_string()]),
        "the stale stack keeps only its own copy",
    );
    assert_eq!(
        payload(&store)?.branches[&stale_id].heads[0].head,
        stale_head,
        "the stale copy is untouched",
    );
    Ok(())
}

/// Two applied stacks may share a segment, so both legitimately list the same
/// branch name. A workspace roundtrip must leave each stack's own copy in place
/// instead of moving the lower-ordered one into whichever stack is written last.
#[test]
fn shared_segment_names_stay_in_their_own_applied_stacks() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;
    let mut data = payload(&store)?;
    let shared_head = gix::ObjectId::from_str("1111111111111111111111111111111111111111")?;

    let names = |heads: &[&str]| {
        heads
            .iter()
            .map(|name| StackBranch {
                head: shared_head,
                name: (*name).into(),
                pr_number: None,
                archived: false,
                review_id: None,
            })
            .collect::<Vec<_>>()
    };
    let stack_a = LegacyStack::new_with_just_heads(names(&["shared", "A"]), 0, true);
    let stack_b = LegacyStack::new_with_just_heads(names(&["shared", "B"]), 1, true);
    let stack_a_id = stack_a.id;
    let stack_b_id = stack_b.id;
    data.branches.insert(stack_a_id, stack_a);
    data.branches.insert(stack_b_id, stack_b);

    set_payload(&mut store, &data)?;
    let workspace_name: gix::refs::FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let ws = store.meta()?.workspace(workspace_name.as_ref())?;
    store.meta_mut()?.set_workspace(&ws)?;

    let head_names = |stack_id: StackId| {
        payload(&store)
            .unwrap()
            .branches
            .get(&stack_id)
            .map(|stack| {
                stack
                    .heads
                    .iter()
                    .map(|head| head.name.clone())
                    .collect::<Vec<_>>()
            })
    };
    assert_eq!(
        head_names(stack_a_id),
        Some(vec!["shared".to_string(), "A".to_string()]),
        "the first applied stack keeps its copy of the shared segment",
    );
    assert_eq!(
        head_names(stack_b_id),
        Some(vec!["shared".to_string(), "B".to_string()]),
        "the second applied stack keeps its copy of the shared segment",
    );
    Ok(())
}

fn project_meta(target_commit_id: gix::ObjectId) -> but_core::ref_metadata::ProjectMeta {
    but_core::ref_metadata::ProjectMeta {
        target_commit_id: Some(target_commit_id),
        ..Default::default()
    }
}
