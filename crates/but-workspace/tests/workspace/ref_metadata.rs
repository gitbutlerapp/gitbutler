use but_core::ref_metadata::ValueInfo;
use but_core::RefMetadata;
use std::collections::HashMap;

mod virtual_branches_toml {
    use crate::ref_metadata::{roundtrip_journey, sanitize_uuids_and_timestamps};
    use but_core::ref_metadata::ValueInfo;
    use but_core::RefMetadata;
    use but_testsupport::gix_testtools::tempfile::{tempdir, TempDir};
    use but_workspace::VirtualBranchesTomlMetadata;
    use std::mem::ManuallyDrop;
    use std::ops::Deref;
    use std::path::PathBuf;

    fn vb_fixture(name: &str) -> PathBuf {
        format!("tests/fixtures/{name}.toml").into()
    }

    /// A store that won't write itself back.
    // TODO: use it or remove it.
    #[allow(dead_code)]
    fn vb_store_ro(name: &str) -> anyhow::Result<ManuallyDrop<VirtualBranchesTomlMetadata>> {
        Ok(ManuallyDrop::new(VirtualBranchesTomlMetadata::from_path(
            vb_fixture(name),
        )?))
    }

    fn vb_store_rw(name: &str) -> anyhow::Result<(VirtualBranchesTomlMetadata, TempDir)> {
        let tmp = TempDir::new()?;
        let writable_toml_path = tmp.path().join("vb.toml");
        std::fs::copy(vb_fixture(name), &writable_toml_path)?;

        let store = VirtualBranchesTomlMetadata::from_path(&writable_toml_path)?;
        Ok((store, tmp))
    }

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
        insta::assert_debug_snapshot!(ws.deref(), @r#"
        Workspace {
            ref_info: RefInfo {
                created_at: Some(
                    Time {
                        seconds: 1675176957,
                        offset: 0,
                        sign: Plus,
                    },
                ),
                updated_at: None,
            },
            stacks: [
                WorkspaceStack {
                    ref_names: [
                        FullName(
                            "refs/heads/A",
                        ),
                    ],
                },
                WorkspaceStack {
                    ref_names: [
                        FullName(
                            "refs/heads/B-top",
                        ),
                        FullName(
                            "refs/heads/B",
                        ),
                    ],
                },
                WorkspaceStack {
                    ref_names: [
                        FullName(
                            "refs/heads/C-top",
                        ),
                        FullName(
                            "refs/heads/C-middle",
                        ),
                        FullName(
                            "refs/heads/C",
                        ),
                    ],
                },
                WorkspaceStack {
                    ref_names: [
                        FullName(
                            "refs/heads/D-top",
                        ),
                        FullName(
                            "refs/heads/D",
                        ),
                    ],
                },
                WorkspaceStack {
                    ref_names: [
                        FullName(
                            "refs/heads/E",
                        ),
                    ],
                },
            ],
            target_ref: Some(
                FullName(
                    "refs/remotes/origin/master",
                ),
            ),
        }
        "#);

        let branches = ws
            .stacks
            .iter()
            .flat_map(|stack| &stack.ref_names)
            .map(|ref_name| {
                store
                    .branch(ref_name.as_ref())
                    .expect("branch is present for each refs mentioned in workspace")
                    .clone()
            })
            .collect::<Vec<_>>();
        insta::assert_debug_snapshot!(branches, @r"
        [
            Branch {
                ref_info: RefInfo {
                    created_at: None,
                    updated_at: Some(
                        Time {
                            seconds: 1740394757,
                            offset: 3600,
                            sign: Plus,
                        },
                    ),
                },
                description: None,
                review: Review {
                    pull_request: Some(
                        12,
                    ),
                    review_id: None,
                },
                archived: false,
            },
            Branch {
                ref_info: RefInfo {
                    created_at: None,
                    updated_at: Some(
                        Time {
                            seconds: 1740394727,
                            offset: 3600,
                            sign: Plus,
                        },
                    ),
                },
                description: None,
                review: Review {
                    pull_request: None,
                    review_id: None,
                },
                archived: false,
            },
            Branch {
                ref_info: RefInfo {
                    created_at: None,
                    updated_at: Some(
                        Time {
                            seconds: 1740394727,
                            offset: 3600,
                            sign: Plus,
                        },
                    ),
                },
                description: None,
                review: Review {
                    pull_request: None,
                    review_id: None,
                },
                archived: false,
            },
            Branch {
                ref_info: RefInfo {
                    created_at: None,
                    updated_at: Some(
                        Time {
                            seconds: 1740394670,
                            offset: 3600,
                            sign: Plus,
                        },
                    ),
                },
                description: None,
                review: Review {
                    pull_request: None,
                    review_id: None,
                },
                archived: false,
            },
            Branch {
                ref_info: RefInfo {
                    created_at: None,
                    updated_at: Some(
                        Time {
                            seconds: 1740394670,
                            offset: 3600,
                            sign: Plus,
                        },
                    ),
                },
                description: None,
                review: Review {
                    pull_request: None,
                    review_id: None,
                },
                archived: false,
            },
            Branch {
                ref_info: RefInfo {
                    created_at: None,
                    updated_at: Some(
                        Time {
                            seconds: 1740394670,
                            offset: 3600,
                            sign: Plus,
                        },
                    ),
                },
                description: None,
                review: Review {
                    pull_request: None,
                    review_id: None,
                },
                archived: false,
            },
            Branch {
                ref_info: RefInfo {
                    created_at: None,
                    updated_at: Some(
                        Time {
                            seconds: 1740394788,
                            offset: 3600,
                            sign: Plus,
                        },
                    ),
                },
                description: None,
                review: Review {
                    pull_request: None,
                    review_id: None,
                },
                archived: false,
            },
            Branch {
                ref_info: RefInfo {
                    created_at: None,
                    updated_at: Some(
                        Time {
                            seconds: 1740394788,
                            offset: 3600,
                            sign: Plus,
                        },
                    ),
                },
                description: None,
                review: Review {
                    pull_request: None,
                    review_id: None,
                },
                archived: false,
            },
            Branch {
                ref_info: RefInfo {
                    created_at: None,
                    updated_at: Some(
                        Time {
                            seconds: 1740394801,
                            offset: 3600,
                            sign: Plus,
                        },
                    ),
                },
                description: None,
                review: Review {
                    pull_request: None,
                    review_id: None,
                },
                archived: false,
            },
        ]
        ");

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
    fn create_workspace_and_stacks_with_branches() -> anyhow::Result<()> {
        let tmp = tempdir()?;
        let toml_path = tmp.path().join("vb.toml");
        let mut store = VirtualBranchesTomlMetadata::from_path(&toml_path)?;
        let branch_name: gix::refs::FullName = "refs/heads/feat".try_into()?;
        let mut branch = store.branch(branch_name.as_ref())?;
        assert!(branch.is_default(), "nothing was there yet");
        assert!(!toml_path.exists(), "file wasn't written yet");

        branch.description = Some("mine".into());
        branch.review = but_core::ref_metadata::Review {
            pull_request: Some(42),
            review_id: Some("review-id".into()),
        };
        branch.archived = true;
        store.set_branch(branch_name.as_ref(), &branch)?;

        let workspace_name: gix::refs::FullName = "refs/heads/gitbutler/workspace".try_into()?;
        let ws = store.workspace(workspace_name.as_ref())?;
        assert!(
            ws.is_default(),
            "archived branches aren't auto-added to the workspace"
        );

        branch.archived = false;
        store.set_branch(branch_name.as_ref(), &branch)?;
        let mut ws = store.workspace(workspace_name.as_ref())?;
        assert!(
            !ws.is_default(),
            "non-archived branches enter the workspace"
        );
        insta::assert_debug_snapshot!(ws.deref(), @r#"
        Workspace {
            ref_info: RefInfo {
                created_at: Some(
                    Time {
                        seconds: 1675176957,
                        offset: 0,
                        sign: Plus,
                    },
                ),
                updated_at: None,
            },
            stacks: [
                WorkspaceStack {
                    ref_names: [
                        FullName(
                            "refs/heads/feat",
                        ),
                    ],
                },
            ],
            target_ref: None,
        }
        "#);

        let stacked_branch_name: gix::refs::FullName = "refs/heads/feat-on-top".try_into()?;
        ws.stacks[0]
            .ref_names
            .insert(0, stacked_branch_name.clone());
        store
            .set_workspace(workspace_name.as_ref(), &ws)
            .expect("This is the way to add branches");

        let ws = store.workspace(workspace_name.as_ref())?;
        insta::assert_debug_snapshot!(ws.deref(), @r#"
        Workspace {
            ref_info: RefInfo {
                created_at: Some(
                    Time {
                        seconds: 1675176957,
                        offset: 0,
                        sign: Plus,
                    },
                ),
                updated_at: None,
            },
            stacks: [
                WorkspaceStack {
                    ref_names: [
                        FullName(
                            "refs/heads/feat-on-top",
                        ),
                        FullName(
                            "refs/heads/feat",
                        ),
                    ],
                },
            ],
            target_ref: None,
        }
        "#);

        drop(store);

        assert!(toml_path.exists(), "file was written due to change");
        insta::assert_snapshot!(sanitize_uuids_and_timestamps(std::fs::read_to_string(&toml_path)?), @r#"
        [branch_targets]

        [branches.1]
        id = "1"
        name = ""
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

        let mut store = VirtualBranchesTomlMetadata::from_path(&toml_path)?;
        let new_ws = store.workspace(workspace_name.as_ref())?;
        assert_eq!(
            new_ws.deref(),
            ws.deref(),
            "It's still what it was before - it was persisted"
        );
        insta::assert_debug_snapshot!(ws.deref(), @r#"
        Workspace {
            ref_info: RefInfo {
                created_at: Some(
                    Time {
                        seconds: 1675176957,
                        offset: 0,
                        sign: Plus,
                    },
                ),
                updated_at: None,
            },
            stacks: [
                WorkspaceStack {
                    ref_names: [
                        FullName(
                            "refs/heads/feat-on-top",
                        ),
                        FullName(
                            "refs/heads/feat",
                        ),
                    ],
                },
            ],
            target_ref: None,
        }
        "#);
        assert!(
            store.remove(stacked_branch_name.as_ref())?,
            "there was something to remove"
        );
        assert!(
            store.remove(branch_name.as_ref())?,
            "there was something to remove, still"
        );
        assert!(
            !store.remove(branch_name.as_ref())?,
            "nothing left to remove"
        );

        let ws = store.workspace(workspace_name.as_ref())?;
        assert!(
            ws.is_default(),
            "it's empty, so no difference to a default one"
        );
        insta::assert_debug_snapshot!(ws.deref(), @r"
        Workspace {
            ref_info: RefInfo {
                created_at: Some(
                    Time {
                        seconds: 1675176957,
                        offset: 0,
                        sign: Plus,
                    },
                ),
                updated_at: None,
            },
            stacks: [],
            target_ref: None,
        }
        ");
        Ok(())
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
            if let Err(err) = metadata.set_workspace(ref_name.as_ref(), &ws) {
                if err.to_string().contains("unsupported") {
                    continue;
                }
            }
            assert_eq!(
                &*metadata.workspace(ref_name.as_ref())?,
                ws_from_iter,
                "nothing should change, it's a no-op"
            );
        } else if let Some(br_from_iter) = md.downcast_ref::<but_core::ref_metadata::Branch>() {
            let br = metadata.branch(ref_name.as_ref())?;
            assert!(!br.is_default(), "default data won't be iterated");
            metadata
                .set_branch(ref_name.as_ref(), &br)
                .expect("updates have no reason to fail, even if no-op");
            assert_eq!(
                &*metadata.branch(ref_name.as_ref())?,
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

fn sanitize_uuids_and_timestamps(input: String) -> String {
    let uuid_regex = regex::Regex::new(
        r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
    )
    .unwrap();
    let timestamp_regex = regex::Regex::new(r#""\d{13}""#).unwrap();

    let mut uuid_map: HashMap<String, usize> = HashMap::new();
    let mut uuid_counter = 1;

    let mut timestamp_map: HashMap<String, usize> = HashMap::new();
    let mut timestamp_counter = 12_345;

    let result = uuid_regex.replace_all(&input, |caps: &regex::Captures| {
        let uuid = caps.get(0).unwrap().as_str().to_string();
        let entry = uuid_map.entry(uuid).or_insert_with(|| {
            let num = uuid_counter;
            uuid_counter += 1;
            num
        });
        entry.to_string()
    });
    let result = timestamp_regex.replace_all(&result, |caps: &regex::Captures| {
        let timestamp = caps.get(0).unwrap().as_str().to_string();
        let entry = timestamp_map.entry(timestamp).or_insert_with(|| {
            let num = timestamp_counter;
            timestamp_counter += 1;
            num
        });
        entry.to_string()
    });

    result.to_string()
}
