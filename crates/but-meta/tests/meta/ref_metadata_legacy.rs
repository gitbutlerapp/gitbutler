use std::{ops::Deref, path::PathBuf, str::FromStr};

use but_core::{
    RefMetadata,
    ref_metadata::{
        StackId, ValueInfo,
        WorkspaceCommitRelation::{Merged, Outside},
        WorkspaceStack, WorkspaceStackBranch,
    },
};
use but_meta::{
    VirtualBranchesTomlMetadata,
    virtual_branches_legacy_types::{Stack as LegacyStack, StackBranch},
};
use but_testsupport::{
    debug_str,
    gix_testtools::tempfile::{TempDir, tempdir},
    sanitize_uuids_and_timestamps, sanitize_uuids_and_timestamps_with_mapping,
};
use snapbox::prelude::*;

#[test]
fn journey() -> anyhow::Result<()> {
    let (mut store, _tmp) = vb_store_rw("virtual-branches-01")?;

    assert_eq!(store.iter().count(), 15, "There are items to test on");
    roundtrip_journey(&mut store)?;
    let writable_toml_path = store.path().to_owned();
    drop(store);
    // The file exists, but is empty and valid. This is handled correctly by code
    // that cares about the file.
    snapbox::assert_data_eq!(
        std::fs::read_to_string(&writable_toml_path)?,
        snapbox::str![[r#"
[branches]

"#]]
    );

    let store = VirtualBranchesTomlMetadata::from_path(&writable_toml_path)?;
    assert_eq!(
        store.iter().count(),
        0,
        "on drop we write the file immediately"
    );
    drop(store);
    assert!(
        writable_toml_path.exists(),
        "default content is mirrored back to TOML"
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(&writable_toml_path)?,
        snapbox::str![[r#"
[branches]

"#]]
    );

    Ok(())
}

#[test]
fn read_only_store_does_not_write_on_drop() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;
    let writable_toml_path = tmp.path().join("vb.toml");
    std::fs::copy(vb_fixture("virtual-branches-01"), &writable_toml_path)?;
    let original = std::fs::read_to_string(&writable_toml_path)?;

    {
        let mut store = VirtualBranchesTomlMetadata::from_path_read_only(&writable_toml_path)?;
        store.data_mut().branches.clear();
        store.set_changed_to_necessitate_write();
    }

    assert_eq!(
        std::fs::read_to_string(&writable_toml_path)?,
        original,
        "read-only metadata is projection input and must not reconcile or persist on drop"
    );
    Ok(())
}

#[test]
fn writable_store_does_not_reconcile_on_drop() -> anyhow::Result<()> {
    let (repo, _tmp) = but_testsupport::writable_scenario("dlib-standin");
    let path = repo.path().join("virtual_branches.toml");
    std::fs::copy(vb_fixture("non-unique-branches"), &path)?;

    let mut store = VirtualBranchesTomlMetadata::from_path(&path)?;
    store.set_changed_to_necessitate_write();
    store.write_unreconciled()?;
    let unreconciled = std::fs::read_to_string(&path)?;
    store.set_changed_to_necessitate_write();
    drop(store);

    assert_eq!(
        std::fs::read_to_string(path)?,
        unreconciled,
        "dropping writable metadata must persist without reconciling it against the workspace"
    );
    Ok(())
}

#[test]
fn read_only() -> anyhow::Result<()> {
    let (mut store, _tmp) = vb_store_rw("virtual-branches-01")?;
    let ws = store.workspace("refs/heads/gitbutler/workspace".try_into()?)?;
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

    let toml_path = store.path().to_owned();
    assert!(toml_path.exists(), "the file is still present");
    let toml_content = std::fs::read_to_string(&toml_path)?;
    let was_deleted = store.remove("refs/heads/gitbutler/workspace".try_into()?)?;
    assert!(was_deleted, "This basically clears out everything");
    assert!(
        toml_path.exists(),
        "workspace clear keeps an empty TOML mirror"
    );
    // The file exists, but is empty and valid. This is handled correctly by code
    // that cares about the file.
    assert_eq!(
        std::fs::read_to_string(&toml_path)?,
        toml_content,
        "The content of the toml file didn't change as syncing happens on drop"
    );

    // Asking for the workspace
    let ws = store.workspace("refs/heads/gitbutler/integration".try_into()?)?;
    assert!(
        ws.is_default(),
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

    assert!(toml_path.exists(), "the TOML mirror stays available");
    snapbox::assert_data_eq!(
        std::fs::read_to_string(&toml_path)?,
        snapbox::str![[r#"
[branches]

"#]]
    );

    Ok(())
}

#[test]
fn create_workspace_and_stacks_with_branches_from_scratch_with_workspace_and_unapply()
-> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;

    let ws_ref = "refs/heads/gitbutler/workspace".try_into()?;
    let mut ws_md = store.workspace(ws_ref)?;
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
    store.set_workspace(&ws_md)?;

    let ws_md = store.workspace(ws_ref)?;
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

    let toml_path = store.path().to_owned();
    drop(store);

    let mut store = VirtualBranchesTomlMetadata::from_path(&toml_path)?;
    let mut ws_md = store.workspace(ws_ref)?;
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
    store.set_workspace(&ws_md)?;
    let mut ws_md = store.workspace(ws_ref)?;
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
    store.set_workspace(&ws_md)?;

    // We are NOT able to retrieve the original names as the backend can't capture it thanks to partial names and the
    // assumption that we never use remote branches directly.
    let ws_md = store.workspace(ws_ref)?;
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
fn set_workspace_stack_only_changes_are_written_on_drop() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;
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

    store
        .data_mut()
        .branches
        .insert(first_stack_id, first_stack);
    store
        .data_mut()
        .branches
        .insert(second_stack_id, second_stack);
    store.set_changed_to_necessitate_write();
    store.write_unreconciled()?;

    let toml_path = store.path().to_owned();
    drop(store);

    let ws_ref: gix::refs::FullName = "refs/heads/gitbutler/workspace".try_into()?;
    let mut store = VirtualBranchesTomlMetadata::from_path(&toml_path)?;
    let mut ws = store.workspace(ws_ref.as_ref())?;
    assert_eq!(ws.stacks.len(), 2, "fixture starts with both stacks");
    ws.stacks.retain(|stack| stack.id == first_stack_id);
    store.set_workspace(&ws)?;
    drop(store);

    let store = VirtualBranchesTomlMetadata::from_path(&toml_path)?;
    let ws = store.workspace(ws_ref.as_ref())?;
    assert_eq!(
        ws.stacks.len(),
        1,
        "stack-only workspace metadata changes must be persisted on drop"
    );
    assert_eq!(ws.stacks[0].id, first_stack_id);
    Ok(())
}

#[test]
fn create_workspace_and_stacks_with_branches_from_scratch() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;

    let toml_path = store.path().to_owned();
    let branch_name: gix::refs::FullName = "refs/heads/feat".try_into()?;
    let mut branch = store.branch(branch_name.as_ref())?;
    assert!(branch.is_default(), "nothing was there yet");
    assert!(toml_path.exists(), "TOML mirror exists from initialization");
    snapbox::assert_data_eq!(
        std::fs::read_to_string(&toml_path)?,
        snapbox::str![[r#"
[branches]

"#]]
    );
    assert_eq!(branch.stack_id(), None, "default values have no stack-id");

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
        .set_workspace(&ws)
        .expect("This is the way to add branches");
    assert_eq!(ws.stack_id(), None);

    // Assure `ws` is what we think it should be - a single stack with one branch.
    let mut ws = store.workspace(workspace_name.as_ref())?;
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
        .set_workspace(&ws)
        .expect("This is the way to add branches");

    let mut ws = store.workspace(workspace_name.as_ref())?;
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

    drop(store);

    assert!(toml_path.exists(), "file was written due to change");
    let (actual, uuids) =
        sanitize_uuids_and_timestamps_with_mapping(std::fs::read_to_string(&toml_path)?);
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

    let mut store = VirtualBranchesTomlMetadata::from_path(&toml_path)?;
    let new_ws = store.workspace(workspace_name.as_ref())?;
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
    store.set_workspace(&ws)?;
    let mut ws = store.workspace(workspace_name.as_ref())?;
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
    snapbox::assert_data_eq!(
        std::fs::read_to_string(&toml_path)?,
        snapbox::str![[r#"
[branches]

"#]]
    );
    assert!(
        toml_path.exists(),
        "default state is still mirrored into TOML"
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
    store.set_workspace(&ws)?;
    let stored_ws = store.workspace(workspace_name)?;
    assert_eq!(stored_ws.deref(), ws.deref());

    // Pop archived branch.
    ws.stacks[0].branches.pop();
    store.set_workspace(&ws)?;
    let mut ws = store.workspace(workspace_name)?;
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
    snapbox::assert_data_eq!(
        sanitize_uuids_and_timestamps(format!("{:#?}", store.workspace(workspace_name)?.deref())),
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
    snapbox::assert_data_eq!(
        sanitize_uuids_and_timestamps(format!("{:#?}", store.workspace(workspace_name)?.deref())),
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

fn vb_store_rw(name: &str) -> anyhow::Result<(VirtualBranchesTomlMetadata, TempDir)> {
    let tmp = TempDir::new()?;
    let writable_toml_path = tmp.path().join("vb.toml");
    std::fs::copy(vb_fixture(name), &writable_toml_path)?;

    let store = VirtualBranchesTomlMetadata::from_path(&writable_toml_path)?;
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
        let mut branch = store.branch(name.as_ref())?;
        branch.review.pull_request = Some(1);
        store.set_branch(&branch)?;
    }

    // Renaming `a` onto the existing `b` must be rejected rather than creating a duplicate head.
    let err = store
        .rename(a.as_ref(), b.as_ref())
        .expect_err("cannot rename onto an existing branch");
    assert!(err.to_string().contains("already exists"), "{err}");
    assert!(store.branch_opt(a.as_ref())?.is_some());
    assert!(store.branch_opt(b.as_ref())?.is_some());

    // Renaming onto a fresh name works and moves the metadata in place.
    let c: gix::refs::FullName = "refs/heads/c".try_into()?;
    store.rename(a.as_ref(), c.as_ref())?;
    assert!(store.branch_opt(a.as_ref())?.is_none());
    assert!(store.branch_opt(c.as_ref())?.is_some());

    // Renaming a branch onto its own name is a no-op, not a self-conflict.
    store.rename(c.as_ref(), c.as_ref())?;
    assert!(store.branch_opt(c.as_ref())?.is_some());

    Ok(())
}

fn empty_vb_store_rw() -> anyhow::Result<(VirtualBranchesTomlMetadata, TempDir)> {
    let tmp = tempdir()?;
    let store = VirtualBranchesTomlMetadata::from_path(tmp.path().join("vb.toml"))?;
    Ok((store, tmp))
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

#[test]
fn legacy_change_id_deserializes_as_null_sha() -> anyhow::Result<()> {
    // The fixture contains a legacy ChangeId which should deserialize as a null SHA.
    // This allows old toml files with ChangeId entries to be loaded without errors.
    let (store, _tmp) = vb_store_rw("legacy-change-id")?;

    // Use a valid UUID for the stack ID that matches the fixture
    let test_stack_id = "12345678-1234-5678-1234-567812345678";

    // Verify that the legacy ChangeId was deserialized as a null SHA
    let stack = store
        .data()
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
fn removes_duplicate_heads_even_if_they_point_to_the_same_commit() -> anyhow::Result<()> {
    let (mut store, _tmp) = empty_vb_store_rw()?;
    let head = gix::ObjectId::from_str("1111111111111111111111111111111111111111")?;

    let first = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head,
            name: "shared".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        0,
        true,
    );
    let second = LegacyStack::new_with_just_heads(
        vec![StackBranch {
            head,
            name: "shared".into(),
            pr_number: None,
            archived: false,
            review_id: None,
        }],
        1,
        true,
    );
    let first_id = first.id;
    let second_id = second.id;
    store.data_mut().branches.insert(first_id, first);
    store.data_mut().branches.insert(second_id, second);
    store.set_changed_to_necessitate_write();

    let path = store.path().to_owned();
    drop(store);

    let store = VirtualBranchesTomlMetadata::from_path(path)?;
    let shared_heads = store
        .data()
        .branches
        .values()
        .flat_map(|stack| stack.heads.iter())
        .filter(|head| head.name == "shared")
        .count();
    assert_eq!(
        shared_heads, 1,
        "without a workspace projection, duplicate names still use first-wins cleanup",
    );

    Ok(())
}

#[test]
fn garbage_collect_removes_outside_workspace_stack_at_target() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("dlib-standin")?;
    let target = repo.head_id()?.detach();
    let (mut store, _tmp) = empty_vb_store_rw()?;

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
    store
        .data_mut()
        .branches
        .insert(workspace_stack_id, workspace_stack);
    store
        .data_mut()
        .branches
        .insert(outside_stack_id, outside_stack);

    store.garbage_collect(&repo, &project_meta(target))?;

    assert!(store.data().branches.contains_key(&workspace_stack_id));
    assert!(!store.data().branches.contains_key(&outside_stack_id));
    Ok(())
}

#[test]
fn garbage_collect_removes_outside_workspace_stack_with_missing_head() -> anyhow::Result<()> {
    let repo = but_testsupport::read_only_in_memory_scenario("dlib-standin")?;
    let target = repo.head_id()?.detach();
    let missing_head = gix::ObjectId::from_hex(b"30696678319e0fa3a20e54f22d47fc8cf1ceaade")?;
    let (mut store, _tmp) = empty_vb_store_rw()?;
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
    store
        .data_mut()
        .branches
        .insert(outside_stack_id, outside_stack);

    store.garbage_collect(&repo, &project_meta(target))?;

    assert!(!store.data().branches.contains_key(&outside_stack_id));
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
    store
        .data_mut()
        .branches
        .insert(outside_stack_id, outside_stack);

    store.garbage_collect(&repo, &project_meta(target))?;

    assert!(!store.data().branches.contains_key(&outside_stack_id));
    Ok(())
}

#[test]
fn preserves_duplicate_heads_if_they_map_to_the_same_workspace_segment() -> anyhow::Result<()> {
    let (repo, _repo_tmp) = but_testsupport::writable_scenario("ws/multi-lane-with-shared-segment");
    let path = repo.path().join("virtual_branches.toml");
    let mut store = VirtualBranchesTomlMetadata::from_path(&path)?;
    but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: None,
        push_remote: Some("origin".into()),
    }
    .persist(&repo)?;

    let stack_a = LegacyStack::new_with_just_heads(
        vec![
            StackBranch::new_with_zero_head("shared".into(), None, None, false),
            StackBranch::new_with_zero_head("A".into(), None, None, false),
        ],
        0,
        true,
    );
    let stack_b = LegacyStack::new_with_just_heads(
        vec![
            StackBranch::new_with_zero_head("shared".into(), None, None, false),
            StackBranch::new_with_zero_head("B".into(), None, None, false),
        ],
        1,
        true,
    );
    let stack_d = LegacyStack::new_with_just_heads(
        vec![
            StackBranch::new_with_zero_head("shared".into(), None, None, false),
            StackBranch::new_with_zero_head("C".into(), None, None, false),
            StackBranch::new_with_zero_head("D".into(), None, None, false),
        ],
        2,
        true,
    );
    store.data_mut().branches.insert(stack_a.id, stack_a);
    store.data_mut().branches.insert(stack_b.id, stack_b);
    store.data_mut().branches.insert(stack_d.id, stack_d);
    store.set_changed_to_necessitate_write();

    store.write_unreconciled()?;

    let store = VirtualBranchesTomlMetadata::from_path(path)?;
    let shared_heads = store
        .data()
        .branches
        .values()
        .flat_map(|stack| stack.heads.iter())
        .filter(|head| head.name == "shared")
        .count();
    assert_eq!(
        shared_heads, 3,
        "repo-backed cleanup should preserve duplicate names when they resolve to the same projected segment",
    );

    Ok(())
}

#[test]
fn removes_within_stack_duplicate_heads_even_when_mapped_to_a_segment_13345() -> anyhow::Result<()>
{
    let (repo, _repo_tmp) = but_testsupport::writable_scenario("ws/multi-lane-with-shared-segment");
    let path = repo.path().join("virtual_branches.toml");
    let mut store = VirtualBranchesTomlMetadata::from_path(&path)?;
    but_core::ref_metadata::ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: None,
        push_remote: Some("origin".into()),
    }
    .persist(&repo)?;

    // A single stack that lists "shared" twice.
    // Even though "shared" maps to a real projected segment, one stack must never repeat a branch.
    let stack = LegacyStack::new_with_just_heads(
        vec![
            StackBranch::new_with_zero_head("shared".into(), None, None, false),
            StackBranch::new_with_zero_head("shared".into(), None, None, false),
            StackBranch::new_with_zero_head("A".into(), None, None, false),
        ],
        0,
        true,
    );
    let stack_id = stack.id;
    store.data_mut().branches.insert(stack_id, stack);
    store.set_changed_to_necessitate_write();

    store.write_unreconciled()?;

    let store = VirtualBranchesTomlMetadata::from_path(path)?;
    let shared_in_stack = store.data().branches.get(&stack_id).map(|stack| {
        stack
            .heads
            .iter()
            .filter(|head| head.name == "shared")
            .count()
    });
    assert_eq!(
        shared_in_stack,
        Some(1),
        "the stack must not keep the same branch twice, even when it maps to a projected segment",
    );

    Ok(())
}

#[cfg(unix)]
#[test]
fn falls_back_to_in_memory_db_when_persistent_db_open_fails() -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    let tmp = tempdir()?;
    let toml_path = tmp.path().join("virtual_branches.toml");
    std::fs::write(&toml_path, "[branches]\n")?;

    let original_permissions = std::fs::metadata(tmp.path())?.permissions();
    let mut read_only_permissions = original_permissions.clone();
    read_only_permissions.set_mode(0o555);
    std::fs::set_permissions(tmp.path(), read_only_permissions)?;

    let store_result = VirtualBranchesTomlMetadata::from_path(&toml_path);

    // Restore permissions so TempDir cleanup can remove the directory.
    std::fs::set_permissions(tmp.path(), original_permissions)?;

    let _store = store_result?;

    assert!(
        !tmp.path().join("but.sqlite").exists(),
        "failed on-disk DB open should not leave a persistent sqlite file behind"
    );
    assert!(
        !tmp.path().join("but.sqlite-wal").exists(),
        "failed on-disk DB open should not leave sqlite wal sidecars behind"
    );
    assert!(
        !tmp.path().join("but.sqlite-shm").exists(),
        "failed on-disk DB open should not leave sqlite shm sidecars behind"
    );
    Ok(())
}

fn project_meta(target_commit_id: gix::ObjectId) -> but_core::ref_metadata::ProjectMeta {
    but_core::ref_metadata::ProjectMeta {
        target_commit_id: Some(target_commit_id),
        ..Default::default()
    }
}
