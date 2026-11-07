mod trailer {
    // use gitbutler_core::ops::entry::Trailer;
    use std::str::FromStr;

    use gitbutler_oplog::entry::Trailer;

    #[test]
    fn display() {
        let trailer = Trailer::Other {
            key: "foo".to_string(),
            value: "bar".to_string(),
        };
        assert_eq!(format!("{trailer}"), "foo: bar");
    }

    #[test]
    fn from_str() {
        let s = "foo: bar";
        let trailer = Trailer::from_str(s).unwrap();
        assert_eq!(
            trailer,
            Trailer::Other {
                key: "foo".to_owned(),
                value: "bar".to_owned()
            }
        );
    }

    #[test]
    fn from_str_invalid() {
        let s = "foobar";
        let result = Trailer::from_str(s);
        assert!(result.is_err());
    }

    #[test]
    fn agent_round_trips() {
        for agent in [Some("claude-code".to_owned()), None] {
            let trailer = Trailer::Agent(agent);
            assert_eq!(
                Trailer::from_str(&trailer.to_string()).unwrap(),
                trailer,
                "a person reads back as no agent, not as one called `none`"
            );
        }
        assert_eq!(Trailer::Agent(None).to_string(), "agent: none");
    }
}

mod version {
    use std::str::FromStr;

    use gitbutler_oplog::entry::{Trailer, Version};

    #[test]
    fn from_trailer() {
        let s = "Version: 3";
        let Trailer::Version(version) = Trailer::from_str(s).unwrap() else {
            panic!();
        };
        assert_eq!(version, Version::default());
    }

    #[test]
    fn non_default() {
        let s = "Version: 1";
        let Trailer::Version(version) = Trailer::from_str(s).unwrap() else {
            panic!();
        };
        assert_eq!(version, Version(1));
    }

    #[test]
    fn invalid() {
        let s = "Version: -1";
        assert!(Trailer::from_str(s).is_err());
    }
}

mod operation_kind {
    use std::str::FromStr;

    use gitbutler_oplog::entry::{OperationKind, SnapshotDetails, Trailer, Version};

    #[test]
    fn from_trailer() {
        let s = "Operation: CreateCommit";
        let Trailer::Operation(operation) = Trailer::from_str(s).unwrap() else {
            panic!();
        };
        assert_eq!(operation, OperationKind::CreateCommit);
    }

    #[test]
    fn unknown() {
        let commit_message = "Create a new snapshot\n\nBody text 1\nBody text2\n\nBody text 3\n\nVersion: 3\nOperation: Asdf\nFoo: Bar\n";
        let details = SnapshotDetails::from_str(commit_message).unwrap();
        assert_eq!(details.version, Version::default());
        assert_eq!(details.operation, OperationKind::Unknown);
        assert_eq!(details.title, "Create a new snapshot");
        assert_eq!(
            details.body,
            Some("Body text 1\nBody text2\n\nBody text 3".to_string())
        );
        assert_eq!(
            details.trailers,
            vec![Trailer::Other {
                key: "Foo".to_string(),
                value: "Bar".to_string(),
            }]
        );
    }
}

mod snapshot_details {
    use std::str::FromStr;

    use gitbutler_oplog::entry::{OperationKind, Snapshot, SnapshotDetails, Trailer, Version};

    #[test]
    fn new() {
        let commit_sha = gix::ObjectId::null(gix::hash::Kind::Sha1);
        let commit_message =
            "Create a new snapshot\n\nBody text 1\nBody text2\n\nBody text 3\n\nVersion: 3\nOperation: CreateCommit\nFoo: Bar\n".to_string();
        let timezone_offset_does_not_matter = 1234;
        let created_at = gix::date::Time::new(1234567890, timezone_offset_does_not_matter * 60);
        let details = SnapshotDetails::from_str(&commit_message).unwrap();
        let snapshot = Snapshot {
            commit_id: commit_sha,
            created_at,
            details: Some(details),
        };
        assert_eq!(snapshot.commit_id, commit_sha);
        assert_eq!(snapshot.created_at, created_at);
        let details = snapshot.details.unwrap();
        assert_eq!(details.version, Version::default());
        assert_eq!(details.operation, OperationKind::CreateCommit);
        assert_eq!(details.title, "Create a new snapshot");
        assert_eq!(
            details.body,
            Some("Body text 1\nBody text2\n\nBody text 3".to_string())
        );
        assert_eq!(
            details.trailers,
            vec![Trailer::Other {
                key: "Foo".to_string(),
                value: "Bar".to_string(),
            }]
        );
        assert_eq!(details.to_string(), commit_message);
    }

    #[test]
    fn new_with_newline_in_trailer() {
        let snapshot_details = new_details(Trailer::Other {
            key: "Message".to_string(),
            value: "Header\n\nBody".to_string(),
        });
        let serialized = snapshot_details.to_string();
        let deserialized = SnapshotDetails::from_str(&serialized).unwrap();
        assert_eq!(
            deserialized, snapshot_details,
            "this works because newlines are quoted"
        )
    }

    #[test]
    fn new_with_space_in_trailer_key() {
        for value in ["trailing-space ", " leading-space"] {
            let trailer = Trailer::Other {
                key: value.to_string(),
                value: "anything".to_string(),
            };
            let mut snapshot_details = new_details(trailer);
            if let Trailer::Other { key, .. } = &mut snapshot_details.trailers[0] {
                *key = key.trim().to_string();
            } else {
                panic!()
            }

            let serialized = snapshot_details.to_string();
            let deserialized = SnapshotDetails::from_str(&serialized).unwrap();
            assert_eq!(deserialized, snapshot_details, "values are trimmed")
        }
    }

    fn new_details(trailer: Trailer) -> SnapshotDetails {
        SnapshotDetails {
            version: Version::default(),
            operation: OperationKind::CreateCommit,
            title: "Create a new snapshot".to_string(),
            body: None,
            trailers: vec![trailer],
        }
    }
}

mod prepare_snapshot {
    use but_meta::virtual_branches_legacy_types::VirtualBranches;
    use but_testsupport::Sandbox;
    use gitbutler_oplog::OplogExt;

    #[test]
    fn snapshot_preserves_workspace_metadata_for_a_missing_branch() -> anyhow::Result<()> {
        let env =
            Sandbox::init_scenario_with_target_and_default_settings("metadata-free-workspace");
        let [stack_id] = *env.setup_metadata(&["A"]) else {
            unreachable!("one branch creates one stack")
        };
        env.invoke_git("checkout main");

        let ctx = but_ctx::Context::from_repo_for_testing(env.open_repo())?;
        let guard = ctx.shared_worktree_access();
        let snapshot_tree_id = ctx.prepare_snapshot(guard.read_permission())?;
        let repo = ctx.repo.get()?;
        let snapshot_tree = repo.find_tree(snapshot_tree_id)?;
        let metadata_blob = snapshot_tree
            .lookup_entry_by_path("virtual_branches.toml")?
            .expect("snapshot contains legacy metadata")
            .object()?
            .into_blob();
        let metadata: VirtualBranches = toml::from_str(std::str::from_utf8(&metadata_blob.data)?)?;

        assert!(
            metadata
                .branches
                .get(&stack_id)
                .expect("legacy stack is retained")
                .in_workspace,
            "snapshot metadata retains saved membership even when Git has no matching branch"
        );
        Ok(())
    }

    #[test]
    fn snapshot_restores_missing_and_surviving_refs_in_the_same_stack() -> anyhow::Result<()> {
        for missing in ["tip", "lower"] {
            let env =
                Sandbox::init_scenario_with_target_and_default_settings("metadata-free-workspace");
            let lower = env.invoke_git("commit-tree main^{tree} -p main -m lower");
            let tip = env.invoke_git(&format!("commit-tree main^{{tree}} -p {lower} -m tip"));
            env.invoke_git(&format!("branch lower {lower}"));
            env.invoke_git(&format!("branch tip {tip}"));
            env.setup_metadata(&["tip"]);
            let workspace_ref = but_core::WORKSPACE_REF_NAME.try_into()?;
            let workspace_before = {
                let mut db = env.meta();
                let mut workspace = db.meta().workspace(workspace_ref)?;
                workspace.stacks[0]
                    .branches
                    .push(but_core::ref_metadata::WorkspaceStackBranch {
                        ref_name: "refs/heads/lower".try_into()?,
                        archived: false,
                    });
                db.meta_mut()?.set_workspace(&workspace)?;
                if missing == "lower" {
                    let mut snapshot = db.virtual_branches().get_snapshot()?.unwrap();
                    for head in &mut snapshot.heads {
                        head.name = format!("refs/heads/{}", head.name);
                    }
                    db.virtual_branches_mut()?.replace_snapshot(&snapshot)?;
                }
                (*workspace).clone()
            };
            env.invoke_git(&format!("branch -D {missing}"));
            env.invoke_git(&format!("tag {missing} main"));
            let surviving = if missing == "tip" { "lower" } else { "tip" };
            let surviving_commit = if missing == "tip" { &lower } else { &tip };

            let mut ctx = but_ctx::Context::from_repo_for_testing(env.open_repo())?;
            let mut guard = ctx.exclusive_worktree_access();
            let snapshot_tree_id = ctx.prepare_snapshot(guard.read_permission())?;
            env.invoke_git(&format!("branch {missing} main"));
            env.invoke_git(&format!("branch -f {surviving} main"));
            let loose_commit = env
                .open_repo()
                .git_dir()
                .join("objects")
                .join(&surviving_commit[..2])
                .join(&surviving_commit[2..]);
            assert!(loose_commit.is_file(), "fixture commit is stored loose");
            std::fs::remove_file(loose_commit)?;

            gitbutler_oplog::restore_checkpoint(
                &mut ctx,
                snapshot_tree_id,
                guard.write_permission(),
            )?;
            let repo = ctx.repo.get()?;
            assert!(
                repo.try_find_reference(&format!("refs/heads/{missing}"))?
                    .is_none(),
                "a ref absent at snapshot time stays absent after restore"
            );
            assert_eq!(
                repo.find_reference(&format!("refs/heads/{surviving}"))?
                    .peel_to_id()?
                    .to_string(),
                *surviving_commit,
                "restore recovers the surviving branch and its archived commit"
            );
            assert_eq!(
                *ctx.db.get_cache()?.meta().workspace(workspace_ref)?,
                workspace_before,
                "missing Git refs do not alter saved workspace membership"
            );
        }
        Ok(())
    }
}

mod integration;
