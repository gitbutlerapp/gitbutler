mod create_reference_as_segment {
    use crate::ref_info::with_workspace_commit::utils::{
        named_read_only_in_memory_scenario, named_writable_scenario,
    };
    use crate::utils::r;
    use ReferencePosition::*;
    use but_core::RefMetadata;
    use but_core::ref_metadata::ValueInfo;
    use but_graph::init::Options;
    use but_testsupport::{graph_workspace, id_at, visualize_commit_graph_all};
    use but_workspace::branch::{ReferenceAnchor, ReferencePosition};

    mod with_workspace {
        use crate::ref_info::with_workspace_commit::utils::{
            StackState, add_stack_with_segments, named_read_only_in_memory_scenario,
        };
        use crate::utils::r;
        use but_core::RefMetadata;
        use but_core::ref_metadata::ValueInfo;
        use but_graph::init::Options;
        use but_testsupport::{graph_workspace, id_at, id_by_rev, visualize_commit_graph_all};
        use but_workspace::branch::ReferenceAnchor;
        use but_workspace::branch::ReferencePosition::*;

        #[test]
        fn error1() -> anyhow::Result<()> {
            let (repo, mut meta) = named_read_only_in_memory_scenario(
                "with-remotes-and-workspace",
                "single-branch-no-ws-commit",
            )?;
            insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
            * bce0c5e (HEAD -> gitbutler/workspace, main) M2
            * 3183e43 (origin/main) M1
            ");

            let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
            let ws = graph.to_workspace()?;

            insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main⇣1 on bce0c5e");

            let (ws_id, ws_ref_name) = id_at(&repo, "gitbutler/workspace");
            let main_remote_id = id_by_rev(&repo, "@~1");
            for anchor in [
                (ReferenceAnchor::at_id(main_remote_id, Above)),
                (ReferenceAnchor::at_segment(r("refs/remotes/origin/main").to_owned(), Above)),
            ] {
                let err = but_workspace::branch::create_reference_as_segment(
                    ws_ref_name.as_ref(),
                    anchor.clone(),
                    &repo,
                    &ws,
                    &mut *meta,
                )
                .unwrap_err();

                let expected_err = if matches!(anchor, ReferenceAnchor::AtCommit { .. }) {
                    "Commit 3183e43ff482a2c4c8ff531d595453b64f58d90b isn't part of the workspace"
                } else {
                    "Couldn't find any stack that contained the branch named 'refs/remotes/origin/main'"
                };
                assert_eq!(
                    err.to_string(),
                    expected_err,
                    "cannot overwrite workspace ref, but it fails as there is nothing in the workspace"
                );
                assert_eq!(
                    repo.find_reference(ws_ref_name.as_ref())?.id(),
                    ws_id,
                    "the reference wasn't changed to the desired location"
                );
                assert!(
                    meta.branch(ws_ref_name.as_ref())?.is_default(),
                    "no data was stored"
                );
            }
            Ok(())
        }

        #[test]
        fn error2() -> anyhow::Result<()> {
            let (repo, mut meta) = named_read_only_in_memory_scenario(
                "with-remotes-and-workspace",
                "single-branch-two-commits-no-ws-commit",
            )?;
            insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
            * c2878fb (HEAD -> gitbutler/workspace, A) A2
            * 49d4b34 A1
            * 3183e43 (origin/main, main) M1
            ");

            add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

            let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
            let ws = graph.to_workspace()?;

            insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
            └── ≡📙:3:A on 3183e43
                └── 📙:3:A
                    ├── ·c2878fb (🏘️)
                    └── ·49d4b34 (🏘️)
            ");

            let (ws_id, ws_ref_name) = id_at(&repo, "gitbutler/workspace");
            // Try to set gitbutler/workspace to a position in the workspace, but one below its current position
            let (a_id, a_ref_name) = id_at(&repo, "A");
            for anchor in [
                (ReferenceAnchor::at_id(a_id, Below)),
                (ReferenceAnchor::at_segment(a_ref_name.to_owned(), Below)),
            ] {
                let err = but_workspace::branch::create_reference_as_segment(
                    ws_ref_name.as_ref(),
                    anchor.clone(),
                    &repo,
                    &ws,
                    &mut *meta,
                )
                .unwrap_err();

                assert_eq!(
                    err.to_string(),
                    "Reference refs/heads/gitbutler/workspace cannot be created as segment at 49d4b34f36239228b64ee758be8f58849bac02d5",
                    "It realizses that the workspace reference isn't ever a segment"
                );
                assert_eq!(
                    repo.find_reference(ws_ref_name.as_ref())?.id(),
                    ws_id,
                    "the reference wasn't changed to the desired location"
                );
                assert!(
                    meta.branch(ws_ref_name.as_ref())?.is_default(),
                    "no data was stored"
                );
            }

            // Try to set gitbutler/workspace to the same position, which technically is in the workspace
            // and is where it's currently pointing to so it seems like nothing changes.
            for anchor in [
                (ReferenceAnchor::at_id(a_id, Above)),
                (ReferenceAnchor::at_segment(a_ref_name.to_owned(), Above)),
            ] {
                let err = but_workspace::branch::create_reference_as_segment(
                    ws_ref_name.as_ref(),
                    anchor.clone(),
                    &repo,
                    &ws,
                    &mut *meta,
                )
                .unwrap_err();

                assert_eq!(
                    err.to_string(),
                    "Reference refs/heads/gitbutler/workspace cannot be created as segment at c2878fb5dda8243a099a0353452d497d906bc6b5",
                    "it detects this issue by simulating the workspace before applying changes"
                );
                assert_eq!(
                    repo.find_reference(ws_ref_name.as_ref())?.id(),
                    ws_id,
                    "the reference wasn't changed to the desired location"
                );
                assert!(
                    meta.branch(ws_ref_name.as_ref())?.is_default(),
                    "no data was stored"
                );
            }
            Ok(())
        }

        // TODO: try to use an existing segment as anchor, try to overwrite gitbutler/workspace
    }

    #[test]
    fn errors() -> anyhow::Result<()> {
        let (repo, mut meta) =
            named_read_only_in_memory_scenario("with-remotes-no-workspace", "remote")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * 89cc2d3 (A) change in A
        * d79bba9 new file in A
        * c166d42 (HEAD -> main) init-integration
        ");

        let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
        let ws = graph.to_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @r"
        ⌂:0:main <> ✓!
        └── ≡:0:main
            └── :0:main
                └── ·c166d42
        ");

        let (id, ref_name) = id_at(&repo, "main");
        let new_name = r("refs/heads/does-not-matter");
        for anchor in [
            ReferenceAnchor::at_id(id, Below),
            ReferenceAnchor::at_segment(ref_name.clone(), Below),
        ] {
            // Below first in history
            let err = but_workspace::branch::create_reference_as_segment(
                new_name, anchor, &repo, &ws, &mut *meta,
            )
            .unwrap_err();
            assert_eq!(
                err.to_string(),
                "Commit c166d42d4ef2e5e742d33554d03805cfb0b24d11 is the first in history and no branch can point below it",
                "it's not possible to show anything before the beginning of time"
            );
            assert!(
                repo.try_find_reference(new_name)?.is_none(),
                "the reference isn't physically available"
            );
            assert!(
                meta.branch(ref_name.as_ref())?.is_default(),
                "no data was stored"
            );
        }

        // Ambiguity (multiple refs in one spot).
        for anchor in [
            ReferenceAnchor::at_id(id, Above),
            ReferenceAnchor::at_segment(ref_name.clone(), Above),
        ] {
            assert!(repo.try_find_reference(new_name)?.is_none());
            let err = but_workspace::branch::create_reference_as_segment(
                new_name, anchor, &repo, &ws, &mut *meta,
            )
            .unwrap_err();
            assert_eq!(
                err.to_string(),
                "Reference refs/heads/does-not-matter cannot be created as segment at c166d42d4ef2e5e742d33554d03805cfb0b24d11",
                "Can't show this reference on top in ad-hoc workspaces (entrypoint rule, etc)"
            );
            assert!(
                repo.try_find_reference(new_name)?.is_none(),
                "the reference isn't physically available"
            );
            assert!(
                meta.branch(ref_name.as_ref())?.is_default(),
                "no data was stored"
            );
        }

        // Misaligned workspace - commit not included.
        let (id, ref_name) = id_at(&repo, "A");
        for anchor in [
            ReferenceAnchor::at_id(id, Below),
            ReferenceAnchor::at_id(id, Above),
        ] {
            let err = but_workspace::branch::create_reference_as_segment(
                new_name, anchor, &repo, &ws, &mut *meta,
            )
            .unwrap_err();
            assert_eq!(
                err.to_string(),
                "Commit 89cc2d303514654e9cab2d05b9af08b420a740c1 isn't part of the workspace",
                "commits are checked for presence in workspace for good measure, and it fails here as the anchor itself isn't\
                in the workspace"
            );
            assert!(
                repo.try_find_reference(new_name)?.is_none(),
                "the reference isn't physically available"
            );
            assert!(
                meta.branch(ref_name.as_ref())?.is_default(),
                "no data was stored"
            );
        }

        // Misaligned workspace - segment not included.
        let (id, ref_name) = id_at(&repo, "A");
        for anchor in [
            (ReferenceAnchor::at_segment(ref_name.clone(), Below)),
            (ReferenceAnchor::at_segment(ref_name.clone(), Above)),
        ] {
            let err = but_workspace::branch::create_reference_as_segment(
                new_name, anchor, &repo, &ws, &mut *meta,
            )
            .unwrap_err();
            assert_eq!(
                err.to_string(),
                "Could not find a segment named 'refs/heads/A' in workspace",
                "segments need to be in the workspace, too"
            );
            assert!(
                repo.try_find_reference(new_name)?.is_none(),
                "the reference isn't physically available"
            );
            assert!(
                meta.branch(ref_name.as_ref())?.is_default(),
                "no data was stored"
            );
        }

        let graph = but_graph::Graph::from_commit_traversal(
            id,
            ref_name.clone(),
            &*meta,
            Options::limited(),
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        ⌂:0:A <> ✓!
        └── ≡:0:A
            ├── :0:A
            │   ├── ·89cc2d3
            │   └── ·d79bba9
            └── :1:main
                └── ·c166d42
        ");

        // Create the same ref at a different location
        let a_ref = r("refs/heads/A");
        let (main_id, main_ref) = id_at(&repo, "main");
        for anchor in [
            (ReferenceAnchor::at_segment(main_ref.to_owned(), Above)),
            (ReferenceAnchor::at_id(main_id, Above)),
        ] {
            let err = but_workspace::branch::create_reference_as_segment(
                a_ref, anchor, &repo, &ws, &mut *meta,
            )
            .unwrap_err();
            assert_eq!(
                err.to_string(),
                "The reference \"refs/heads/A\" should have content c166d42d4ef2e5e742d33554d03805cfb0b24d11, actual content was 89cc2d303514654e9cab2d05b9af08b420a740c1",
                "it won't reset existing refs as the constraint is setup correctly.\
                It does try though."
            );
            assert!(meta.branch(a_ref)?.is_default(), "no data was stored");
            assert_ne!(
                repo.find_reference(a_ref)?.id(),
                main_id,
                "it shouldn't actually have change the ref"
            );
        }

        let graph = but_graph::Graph::from_commit_traversal(
            id,
            ref_name,
            &*meta,
            Options {
                extra_target_commit_id: main_id.detach().into(),
                commits_limit_hint: 0.into(),
                ..Options::limited()
            },
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        ⌂:0:A <> ✓!
        └── ≡:0:A
            └── :0:A
                └── ✂️·89cc2d3
        ");

        let (a_id, _a_ref_owned) = id_at(&repo, "A");
        for anchor in [
            (ReferenceAnchor::at_segment(a_ref.to_owned(), Below)),
            (ReferenceAnchor::at_id(a_id, Below)),
        ] {
            let err = but_workspace::branch::create_reference_as_segment(
                new_name, anchor, &repo, &ws, &mut *meta,
            )
            .unwrap_err();
            assert_eq!(
                err.to_string(),
                "Commit d79bba960b112dbd25d45921c47eeda22288022b isn't part of the workspace",
                "it also checks the final commit for workspace membership (as it could be a parent that is below the base);\
                This is an extra check for better error message"
            );
            assert!(meta.branch(a_ref)?.is_default(), "no data was stored");
            assert_ne!(
                repo.find_reference(a_ref)?.id(),
                main_id,
                "it shouldn't actually have change the ref"
            );
        }
        Ok(())
    }

    #[test]
    fn journey() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-with-3-commits")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * 281da94 (HEAD -> main) 3
        * 12995d7 2
        * 3d57fc1 1
        ");

        let graph = but_graph::Graph::from_head(&repo, &meta, meta.graph_options())?;
        let ws = graph.to_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @r"
        ⌂:0:main <> ✓!
        └── ≡:0:main
            └── :0:main
                ├── ·281da94
                ├── ·12995d7
                └── ·3d57fc1
        ");

        let (id, main) = id_at(&repo, "main");
        let new_name = r("refs/heads/below-main");
        let new_graph = but_workspace::branch::create_reference_as_segment(
            new_name,
            ReferenceAnchor::at_segment(main.to_owned(), Below),
            &repo,
            &ws,
            &mut meta,
        )
        .expect("this works as the branch is unique");

        // We always add metadata to new branches.
        insta::assert_snapshot!(graph_workspace(&new_graph.to_workspace()?), @r"
        ⌂:0:main <> ✓!
        └── ≡:0:main
            ├── :0:main
            │   └── ·281da94
            └── 📙:1:below-main
                ├── ·12995d7
                └── ·3d57fc1
        ");
        let md = meta.branch(new_name)?;
        assert!(!md.is_default(), "It should have set the date at least");
        assert!(md.ref_info.updated_at.is_some());
        assert!(
            md.ref_info.created_at.is_none(),
            "It marks the creation date as well.\
            HOWEVER: this backend can't currently store such a field - needs sqlite backend"
        );
        assert!(
            repo.find_reference(new_name).is_ok(),
            "It should just have been created"
        );

        // Creating the same reference again is idempotent.
        let new_graph = but_workspace::branch::create_reference_as_segment(
            new_name,
            ReferenceAnchor::at_id(id, Below),
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = new_graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        ⌂:0:main <> ✓!
        └── ≡:0:main
            ├── :0:main
            │   └── ·281da94
            └── 📙:1:below-main
                ├── ·12995d7
                └── ·3d57fc1
        ");

        // the last possible branch without a workspace.
        let new_graph = but_workspace::branch::create_reference_as_segment(
            r("refs/heads/two-below-main"),
            ReferenceAnchor::at_segment(r("refs/heads/below-main").to_owned(), Below),
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = new_graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        ⌂:0:main <> ✓!
        └── ≡:0:main
            ├── :0:main
            │   └── ·281da94
            ├── 📙:1:below-main
            │   └── ·12995d7
            └── 📙:2:two-below-main
                └── ·3d57fc1
        ");

        // Now no new segment can be created anymore, each commit aan only have one.
        // the last possible branch without a workspace.
        let err = but_workspace::branch::create_reference_as_segment(
            r("refs/heads/another-below-main"),
            ReferenceAnchor::at_segment(main, Below),
            &repo,
            &ws,
            &mut meta,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Reference refs/heads/another-below-main cannot be created as segment at 12995d783f3ac841a1774e9433ee8e4c1edac576"
        );

        Ok(())
    }
}
