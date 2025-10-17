use std::borrow::Cow;

use but_core::{RefMetadata, ref_metadata::ValueInfo};
use but_graph::init::Options;
use but_testsupport::{graph_workspace, id_at, id_by_rev, visualize_commit_graph_all};
use but_workspace::branch::create_reference::{Anchor, Position::*};

use crate::{
    ref_info::with_workspace_commit::utils::{
        named_read_only_in_memory_scenario, named_writable_scenario,
    },
    utils::{r, rc},
};

mod with_workspace {
    use std::borrow::Cow;

    use but_core::{RefMetadata, ref_metadata::ValueInfo};
    use but_graph::{VirtualBranchesTomlMetadata, init::Options};
    use but_testsupport::{graph_workspace, id_at, id_by_rev, visualize_commit_graph_all};
    use but_workspace::branch::create_reference::{Anchor, Position::*};

    use crate::{
        ref_info::with_workspace_commit::utils::{
            StackState, add_stack_with_segments, named_read_only_in_memory_scenario,
            named_writable_scenario, named_writable_scenario_with_description,
        },
        utils::{r, rc},
    };

    #[test]
    fn journey_no_ws_commit_no_target() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, desc) =
            named_writable_scenario_with_description("single-branch-no-ws-commit-no-target")?;
        insta::assert_snapshot!(desc, @"Single commit, no main remote/target, no ws commit, but ws-reference");

        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, main) M1");

        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            Options {
                extra_target_commit_id: id_by_rev(&repo, "main").detach().into(),
                ..Options::limited()
            },
        )?;
        let ws = graph.to_workspace()?;

        // And even though setting an extra-target works like it should, i.e a simulated target
        // which we can store in absence of a selected target branch…
        insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓! on 3183e43");

        // …we chose to work with an open-ended workspace just to struggle more.
        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️⚠️:0:gitbutler/workspace <> ✓!
            └── ≡:1:main
                └── :1:main
                    └── ·3183e43 (🏘️)
            ");

        let new_name = rc("refs/heads/A");
        let err = but_workspace::branch::create_reference(
            new_name, None, /* anchor */
            &repo, &ws, &mut meta,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "workspace at refs/heads/gitbutler/workspace is missing a base",
            "independent branches can't currently be created in this kind of workspace - need a base"
        );

        Ok(())
    }

    #[test]
    fn journey_no_ws_commit() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, desc) =
            named_writable_scenario_with_description("single-branch-no-ws-commit")?;
        insta::assert_snapshot!(desc, @"Single commit, target, no ws commit, but ws-reference");

        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, origin/main, main) M1");

        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.to_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43");

        let a_ref = r("refs/heads/A");
        let graph = but_workspace::branch::create_reference(
            a_ref, None, /* anchor */
            &repo, &ws, &mut meta,
        )
        .expect("it updates the workspace metadata legitimate the new ref at base");
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
            └── ≡📙:3:A on 3183e43
                └── 📙:3:A
            ");
        let ws_base = ws.lower_bound.expect("target is set");
        assert_eq!(
            repo.find_reference(a_ref)?.id(),
            ws_base,
            "new stack refs are created on the workspace base"
        );

        let b_ref = r("refs/heads/B");
        let graph = but_workspace::branch::create_reference(
            b_ref, None, /* anchor */
            &repo, &ws, &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
            ├── ≡📙:4:B on 3183e43
            │   └── 📙:4:B
            └── ≡📙:3:A on 3183e43
                └── 📙:3:A
            ");

        // Idempotency
        let graph = but_workspace::branch::create_reference(
            b_ref, None, /* anchor */
            &repo, &ws, &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
            ├── ≡📙:4:B on 3183e43
            │   └── 📙:4:B
            └── ≡📙:3:A on 3183e43
                └── 📙:3:A
            ");

        let above_a = rc("refs/heads/above-A");
        let graph = but_workspace::branch::create_reference(
            above_a,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(a_ref),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
            ├── ≡📙:5:B on 3183e43
            │   └── 📙:5:B
            └── ≡📙:3:above-A on 3183e43
                ├── 📙:3:above-A
                └── 📙:4:A
            ");

        let below_b = rc("refs/heads/below-B");
        let graph = but_workspace::branch::create_reference(
            below_b,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
            ├── ≡📙:5:B on 3183e43
            │   ├── 📙:5:B
            │   └── 📙:6:below-B
            └── ≡📙:3:above-A on 3183e43
                ├── 📙:3:above-A
                └── 📙:4:A
            ");

        // Finally, assure the data looks correct. Can't afford bugs in the translation.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
            ├── ≡📙:5:B on 3183e43
            │   ├── 📙:5:B
            │   └── 📙:6:below-B
            └── ≡📙:3:above-A on 3183e43
                ├── 📙:3:above-A
                └── 📙:4:A
            ");

        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, origin/main, main, below-B, above-A, B, A) M1");

        Ok(())
    }

    #[test]
    fn journey_single_branch_segment_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-4-commits")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
            * 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
            * 43f9472 (A) A2
            * 6fdab32 A1
            * bce0c5e (origin/main, main) M2
            * 3183e43 M1
            ");

        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.to_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
            └── ≡:3:A on bce0c5e
                └── :3:A
                    ├── ·43f9472 (🏘️)
                    └── ·6fdab32 (🏘️)
            ");

        let above_bottom_ref = r("refs/heads/above-bottom");
        let bottom_id = id_by_rev(&repo, ":/A1");
        let graph = but_workspace::branch::create_reference(
            above_bottom_ref,
            Anchor::AtCommit {
                commit_id: bottom_id.detach(),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        // It handles this special case, by creating the necessary workspace metadata
        // if for some reason (like manual building) it's not set.
        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
            └── ≡:4:A on bce0c5e
                ├── :4:A
                │   └── ·43f9472 (🏘️)
                └── 📙:3:above-bottom
                    └── ·6fdab32 (🏘️)
            ");

        let bottom_ref = rc("refs/heads/bottom");
        let graph = but_workspace::branch::create_reference(
            bottom_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_bottom_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡:4:A on bce0c5e
            ├── :4:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:3:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:5:bottom
        ");

        let above_a_commit_ref = r("refs/heads/above-A-commit");
        let a_id = id_by_rev(&repo, ":/A");
        let graph = but_workspace::branch::create_reference(
            above_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        // Note how 'Above' *a commit* means directly above, not on top of everything.
        let ws = graph.to_workspace()?;
        // And as there are now two references on one commit, and one has metadata, the other one doesn't,
        // 'A' is moved to the background.
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡📙:3:above-A-commit on bce0c5e
            ├── 📙:3:above-A-commit
            │   └── ·43f9472 (🏘️) ►A
            ├── 📙:4:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:5:bottom
        ");

        // We can, however, restore it simply by putting idempotency.
        let a_ref = rc("refs/heads/A");
        let graph = but_workspace::branch::create_reference(
            a_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        let ws = graph.to_workspace()?;
        // And 'A' is back, with the desired order correctly restored.
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡📙:5:above-A-commit on bce0c5e
            ├── 📙:5:above-A-commit
            ├── 📙:6:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:4:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:7:bottom
        ");

        let above_a_ref = rc("refs/heads/above-A");
        let a_ref = rc("refs/heads/A");
        let graph = but_workspace::branch::create_reference(
            above_a_ref,
            Anchor::AtSegment {
                ref_name: a_ref,
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        // *Above a segment means what one would expect though.
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡📙:5:above-A-commit on bce0c5e
            ├── 📙:5:above-A-commit
            ├── 📙:6:above-A
            ├── 📙:7:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:4:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:8:bottom
        ");

        let below_a_commit_ref = rc("refs/heads/below-A-commit");
        let graph = but_workspace::branch::create_reference(
            below_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡📙:5:above-A-commit on bce0c5e
            ├── 📙:5:above-A-commit
            ├── 📙:6:above-A
            ├── 📙:7:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:8:below-A-commit
            ├── 📙:9:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:10:bottom
        ");

        let below_a_ref = rc("refs/heads/below-A");
        let graph = but_workspace::branch::create_reference(
            below_a_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_a_commit_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡📙:5:above-A-commit on bce0c5e
            ├── 📙:5:above-A-commit
            ├── 📙:6:above-A
            ├── 📙:7:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:8:below-A
            ├── 📙:9:below-A-commit
            ├── 📙:10:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:11:bottom
        ");

        // create a new stack for good measure.
        let b_ref = r("refs/heads/B");
        let graph = but_workspace::branch::create_reference(b_ref, None, &repo, &ws, &mut meta)?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
        ├── ≡📙:5:B on bce0c5e
        │   └── 📙:5:B
        └── ≡📙:6:above-A-commit on bce0c5e
            ├── 📙:6:above-A-commit
            ├── 📙:7:above-A
            ├── 📙:8:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:9:below-A
            ├── 📙:10:below-A-commit
            ├── 📙:11:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:12:bottom
        ");

        // create a new dependent branch by segment above (commit can't be done).
        let above_b_ref = rc("refs/heads/above-B");
        let graph = but_workspace::branch::create_reference(
            above_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
        ├── ≡📙:5:above-B on bce0c5e
        │   ├── 📙:5:above-B
        │   └── 📙:6:B
        └── ≡📙:7:above-A-commit on bce0c5e
            ├── 📙:7:above-A-commit
            ├── 📙:8:above-A
            ├── 📙:9:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:10:below-A
            ├── 📙:11:below-A-commit
            ├── 📙:12:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:13:bottom
        ");

        // create a new dependent branch by segment below
        // (which somewhat counter-intuitively works here) because it's a completly new
        // independent branch.
        let below_b_ref = rc("refs/heads/below-B");
        let graph = but_workspace::branch::create_reference(
            below_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
        ├── ≡📙:5:above-B on bce0c5e
        │   ├── 📙:5:above-B
        │   ├── 📙:6:B
        │   └── 📙:7:below-B
        └── ≡📙:8:above-A-commit on bce0c5e
            ├── 📙:8:above-A-commit
            ├── 📙:9:above-A
            ├── 📙:10:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:11:below-A
            ├── 📙:12:below-A-commit
            ├── 📙:13:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:14:bottom
        ");

        // Finally, assure the data looks correct. Can't afford bugs in the translation.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on bce0c5e
        ├── ≡📙:5:above-B on bce0c5e
        │   ├── 📙:5:above-B
        │   ├── 📙:6:B
        │   └── 📙:7:below-B
        └── ≡📙:8:above-A-commit on bce0c5e
            ├── 📙:8:above-A-commit
            ├── 📙:9:above-A
            ├── 📙:10:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:11:below-A
            ├── 📙:12:below-A-commit
            ├── 📙:13:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:14:bottom
        ");

        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        * 43f9472 (above-A-commit, above-A, A) A2
        * 6fdab32 (below-A-commit, below-A, above-bottom) A1
        * bce0c5e (origin/main, main, bottom, below-B, above-B, B) M2
        * 3183e43 M1
        ");
        Ok(())
    }

    #[test]
    fn journey_single_branch_no_ws_commit_segment_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) =
            named_writable_scenario("single-branch-3-commits-no-ws-commit")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
            * c2878fb (HEAD -> gitbutler/workspace, A) A2
            * 49d4b34 A1
            * 3183e43 (origin/main, main) M1
            ");

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.to_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
            └── ≡📙:3:A on 3183e43
                └── 📙:3:A
                    ├── ·c2878fb (🏘️)
                    └── ·49d4b34 (🏘️)
            ");

        let above_bottom_ref = r("refs/heads/above-bottom");
        let bottom_id = id_by_rev(&repo, ":/A1");
        let graph = but_workspace::branch::create_reference(
            above_bottom_ref,
            Anchor::AtCommit {
                commit_id: bottom_id.detach(),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
            └── ≡📙:4:A on 3183e43
                ├── 📙:4:A
                │   └── ·c2878fb (🏘️)
                └── 📙:3:above-bottom
                    └── ·49d4b34 (🏘️)
            ");

        let bottom_ref = rc("refs/heads/bottom");
        let graph = but_workspace::branch::create_reference(
            bottom_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_bottom_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        let ws = graph.to_workspace()?;
        // We can create branches that would be on the base.
        // There are
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:4:A on 3183e43
            ├── 📙:4:A
            │   └── ·c2878fb (🏘️)
            ├── 📙:3:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:5:bottom
        ");

        let above_a_commit_ref = r("refs/heads/above-A-commit");
        let a_id = id_by_rev(&repo, ":/A");
        let graph = but_workspace::branch::create_reference(
            above_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        // Note how 'Above' *a commit* means directly above, not on top of everything.
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:5:A on 3183e43
            ├── 📙:5:A
            ├── 📙:6:above-A-commit
            │   └── ·c2878fb (🏘️)
            ├── 📙:3:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:7:bottom
        ");

        let above_a_ref = rc("refs/heads/above-A");
        let a_ref = rc("refs/heads/A");
        let graph = but_workspace::branch::create_reference(
            above_a_ref,
            Anchor::AtSegment {
                ref_name: a_ref,
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        // *Above a segment means what one would expect though.
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:5:above-A on 3183e43
            ├── 📙:5:above-A
            ├── 📙:6:A
            ├── 📙:7:above-A-commit
            │   └── ·c2878fb (🏘️)
            ├── 📙:3:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:8:bottom
        ");

        // Idempotency!
        let above_a_ref = rc("refs/heads/above-A");
        let a_ref = rc("refs/heads/A");
        let graph = but_workspace::branch::create_reference(
            above_a_ref,
            Anchor::AtSegment {
                ref_name: a_ref,
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:5:above-A on 3183e43
            ├── 📙:5:above-A
            ├── 📙:6:A
            ├── 📙:7:above-A-commit
            │   └── ·c2878fb (🏘️)
            ├── 📙:3:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:8:bottom
        ");

        let below_a_commit_ref = rc("refs/heads/below-A-commit");
        let graph = but_workspace::branch::create_reference(
            below_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:5:above-A on 3183e43
            ├── 📙:5:above-A
            ├── 📙:6:A
            ├── 📙:7:above-A-commit
            │   └── ·c2878fb (🏘️)
            ├── 📙:8:below-A-commit
            ├── 📙:9:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:10:bottom
        ");

        let below_a_ref = rc("refs/heads/below-A");
        let graph = but_workspace::branch::create_reference(
            below_a_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_a_commit_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:5:above-A on 3183e43
            ├── 📙:5:above-A
            ├── 📙:6:A
            ├── 📙:7:above-A-commit
            │   └── ·c2878fb (🏘️)
            ├── 📙:8:below-A
            ├── 📙:9:below-A-commit
            ├── 📙:10:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:11:bottom
        ");

        // create a new stack for good measure.
        let b_ref = r("refs/heads/B");
        let graph = but_workspace::branch::create_reference(b_ref, None, &repo, &ws, &mut meta)?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:5:B on 3183e43
        │   └── 📙:5:B
        └── ≡📙:6:above-A on 3183e43
            ├── 📙:6:above-A
            ├── 📙:7:A
            ├── 📙:8:above-A-commit
            │   └── ·c2878fb (🏘️)
            ├── 📙:9:below-A
            ├── 📙:10:below-A-commit
            ├── 📙:11:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:12:bottom
        ");

        // create a new dependent branch by segment above (commit can't be done).
        let above_b_ref = rc("refs/heads/above-B");
        let graph = but_workspace::branch::create_reference(
            above_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:5:above-B on 3183e43
        │   ├── 📙:5:above-B
        │   └── 📙:6:B
        └── ≡📙:7:above-A on 3183e43
            ├── 📙:7:above-A
            ├── 📙:8:A
            ├── 📙:9:above-A-commit
            │   └── ·c2878fb (🏘️)
            ├── 📙:10:below-A
            ├── 📙:11:below-A-commit
            ├── 📙:12:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:13:bottom
        ");

        // create a new dependent branch by segment below
        // (which somewhat counter-intuitively works here) because it's a completly new
        // independent branch.
        let below_b_ref = rc("refs/heads/below-B");
        let graph = but_workspace::branch::create_reference(
            below_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:5:above-B on 3183e43
        │   ├── 📙:5:above-B
        │   ├── 📙:6:B
        │   └── 📙:7:below-B
        └── ≡📙:8:above-A on 3183e43
            ├── 📙:8:above-A
            ├── 📙:9:A
            ├── 📙:10:above-A-commit
            │   └── ·c2878fb (🏘️)
            ├── 📙:11:below-A
            ├── 📙:12:below-A-commit
            ├── 📙:13:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:14:bottom
        ");

        // Finally, assure the data looks correct. Can't afford bugs in the translation.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:5:above-B on 3183e43
        │   ├── 📙:5:above-B
        │   ├── 📙:6:B
        │   └── 📙:7:below-B
        └── ≡📙:8:above-A on 3183e43
            ├── 📙:8:above-A
            ├── 📙:9:A
            ├── 📙:10:above-A-commit
            │   └── ·c2878fb (🏘️)
            ├── 📙:11:below-A
            ├── 📙:12:below-A-commit
            ├── 📙:13:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:14:bottom
        ");

        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * c2878fb (HEAD -> gitbutler/workspace, above-A-commit, above-A, A) A2
        * 49d4b34 (below-A-commit, below-A, above-bottom) A1
        * 3183e43 (origin/main, main, bottom, below-B, above-B, B) M1
        ");
        Ok(())
    }

    #[test]
    fn journey_single_branch_no_ws_commit_commit_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) =
            named_writable_scenario("single-branch-3-commits-no-ws-commit")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
            * c2878fb (HEAD -> gitbutler/workspace, A) A2
            * 49d4b34 A1
            * 3183e43 (origin/main, main) M1
            ");

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.to_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @r"
            📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
            └── ≡📙:3:A on 3183e43
                └── 📙:3:A
                    ├── ·c2878fb (🏘️)
                    └── ·49d4b34 (🏘️)
            ");

        let bottom_ref = rc("refs/heads/bottom");
        let bottom_id = id_by_rev(&repo, ":/A1");
        let graph = but_workspace::branch::create_reference(
            bottom_ref,
            Anchor::AtCommit {
                commit_id: bottom_id.detach(),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️⚠️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:3:A on 3183e43
            ├── 📙:3:A
            │   ├── ·c2878fb (🏘️)
            │   └── ·49d4b34 (🏘️)
            └── 📙:4:bottom
        ");
        Ok(())
    }

    #[test]
    fn journey_multi_branch_commit_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("multi-branch-with-ws-commit")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        *   eaf2834 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        |\  
        | * 49d4b34 (A) A1
        * | f57c528 (B) B1
        |/  
        * 3183e43 (origin/main, main) M1
        ");

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.to_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:4:B on 3183e43
        │   └── 📙:4:B
        │       └── ·f57c528 (🏘️)
        └── ≡📙:3:A on 3183e43
            └── 📙:3:A
                └── ·49d4b34 (🏘️)
        ");

        let bottom_ref_a = rc("refs/heads/a-bottom");
        let bottom_a_id = id_by_rev(&repo, ":/A1");
        let graph = but_workspace::branch::create_reference(
            bottom_ref_a,
            Anchor::AtCommit {
                commit_id: bottom_a_id.detach(),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;
        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:4:B on 3183e43
        │   └── 📙:4:B
        │       └── ·f57c528 (🏘️)
        └── ≡📙:3:A on 3183e43
            ├── 📙:3:A
            │   └── ·49d4b34 (🏘️)
            └── 📙:5:a-bottom
        ");

        let bottom_ref_b = rc("refs/heads/b-bottom");
        let bottom_b_id = id_by_rev(&repo, ":/B1");
        let graph = but_workspace::branch::create_reference(
            bottom_ref_b,
            Anchor::AtCommit {
                commit_id: bottom_b_id.detach(),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
        )?;

        let ws = graph.to_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @r"
        📕🏘️:0:gitbutler/workspace <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:4:B on 3183e43
        │   ├── 📙:4:B
        │   │   └── ·f57c528 (🏘️)
        │   └── 📙:6:b-bottom
        └── ≡📙:3:A on 3183e43
            ├── 📙:3:A
            │   └── ·49d4b34 (🏘️)
            └── 📙:5:a-bottom
        ");
        Ok(())
    }

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
            (Anchor::at_id(main_remote_id, Above)),
            (Anchor::at_segment(r("refs/remotes/origin/main"), Above)),
        ] {
            let err = but_workspace::branch::create_reference(
                ws_ref_name.as_ref(),
                anchor.clone(),
                &repo,
                &ws,
                &mut *meta,
            )
            .unwrap_err();

            let expected_err = if matches!(anchor, Anchor::AtCommit { .. }) {
                "Commit 3183e43ff482a2c4c8ff531d595453b64f58d90b isn't part of the workspace"
            } else {
                "Couldn't find any stack that contained the branch named 'origin/main'"
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
            * bba50eb (extra) E1
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
            (Anchor::at_id(a_id, Below)),
            (Anchor::at_segment(a_ref_name.as_ref(), Below)),
        ] {
            let err = but_workspace::branch::create_reference(
                ws_ref_name.as_ref(),
                anchor.clone(),
                &repo,
                &ws,
                &mut *meta,
            )
            .unwrap_err();

            assert_eq!(
                err.to_string(),
                "Reference 'gitbutler/workspace' cannot be created as segment at 49d4b34f36239228b64ee758be8f58849bac02d5",
                "It realizes that the workspace reference isn't ever a segment"
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
            (Anchor::at_id(a_id, Above)),
            (Anchor::at_segment(a_ref_name.as_ref(), Above)),
        ] {
            let err = but_workspace::branch::create_reference(
                ws_ref_name.as_ref(),
                anchor.clone(),
                &repo,
                &ws,
                &mut *meta,
            )
            .unwrap_err();

            assert_eq!(
                err.to_string(),
                "Reference 'gitbutler/workspace' cannot be created as segment at c2878fb5dda8243a099a0353452d497d906bc6b5",
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

        // Creating independent branches inside the workspace that already exist outside of it.
        let (outside_id, outside_ref) = id_at(&repo, "extra");
        let err = but_workspace::branch::create_reference(
            outside_ref.as_ref(),
            None,
            &repo,
            &ws,
            &mut *meta,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Reference 'extra' cannot be created as segment at 3183e43ff482a2c4c8ff531d595453b64f58d90b",
            "The simulation catches the issue first, note how it wants to create it at the base"
        );
        assert!(
            meta.branch(outside_ref.as_ref())?.is_default(),
            "no data was stored"
        );
        assert_eq!(
            repo.find_reference(outside_ref.as_ref())?.id(),
            outside_id,
            "it shouldn't actually have changed the ref"
        );

        let new_name = rc("refs/heads/new");
        let err = but_workspace::branch::create_reference(
            new_name,
            Anchor::AtSegment {
                ref_name: rc("refs/heads/bogus"),
                position: Below,
            },
            &repo,
            &ws,
            &mut *meta,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "Couldn't find any stack that contained the branch named 'bogus'",
            "It yells loudly if the inputs don't match up - anchors must always be in the workspace."
        );
        Ok(())
    }
}

#[test]
fn errors() -> anyhow::Result<()> {
    let (repo, mut meta) = named_read_only_in_memory_scenario("unborn-empty", "")?;
    let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
    ⌂:0:main <> ✓!
    └── ≡:0:main
        └── :0:main
    ");

    // Below first in history
    let new_name = r("refs/heads/does-not-matter");
    let err = but_workspace::branch::create_reference(
        new_name,
        Anchor::AtSegment {
            ref_name: Cow::Borrowed(r("refs/heads/main")),
            position: Above,
        },
        &repo,
        &ws,
        &mut *meta,
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "Cannot create reference on unborn branch");

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
    for anchor in [
        Anchor::at_id(id, Below),
        Anchor::at_segment(ref_name.as_ref(), Below),
    ] {
        // Below first in history
        let err = but_workspace::branch::create_reference(new_name, anchor, &repo, &ws, &mut *meta)
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

    // Misaligned workspace - commit not included.
    let (id, ref_name) = id_at(&repo, "A");
    for anchor in [Anchor::at_id(id, Below), Anchor::at_id(id, Above)] {
        let err = but_workspace::branch::create_reference(new_name, anchor, &repo, &ws, &mut *meta)
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
    let (a_id, a_ref) = id_at(&repo, "A");
    for anchor in [
        (Anchor::at_segment(a_ref.as_ref(), Below)),
        (Anchor::at_segment(a_ref.as_ref(), Above)),
    ] {
        let err = but_workspace::branch::create_reference(new_name, anchor, &repo, &ws, &mut *meta)
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Could not find a segment named 'A' in workspace",
            "segments need to be in the workspace, too"
        );
        assert!(
            repo.try_find_reference(new_name)?.is_none(),
            "the reference isn't physically available"
        );
        assert!(
            meta.branch(a_ref.as_ref())?.is_default(),
            "no data was stored"
        );
    }

    let graph = but_graph::Graph::from_commit_traversal(a_id, a_ref, &*meta, Options::limited())?;
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
        (Anchor::at_segment(main_ref.as_ref(), Above)),
        (Anchor::at_id(main_id, Above)),
    ] {
        let err = but_workspace::branch::create_reference(a_ref, anchor, &repo, &ws, &mut *meta)
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
        a_id,
        a_ref.to_owned(),
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
        (Anchor::at_segment(a_ref, Below)),
        (Anchor::at_id(a_id, Below)),
    ] {
        let err = but_workspace::branch::create_reference(new_name, anchor, &repo, &ws, &mut *meta)
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
            "it shouldn't actually have changed the ref"
        );
    }
    Ok(())
}

#[test]
fn journey_with_commits() -> anyhow::Result<()> {
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

    let (main_id, main_ref) = id_at(&repo, "main");
    let new_name = r("refs/heads/below-main");
    let graph = but_workspace::branch::create_reference(
        new_name,
        Anchor::at_segment(main_ref.as_ref(), Below),
        &repo,
        &ws,
        &mut meta,
    )
    .expect("this works as the branch is unique");

    // We always add metadata to new branches.
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
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
    let graph = but_workspace::branch::create_reference(
        new_name,
        Anchor::at_id(main_id, Below),
        &repo,
        &ws,
        &mut meta,
    )?;
    let ws = graph.to_workspace()?;
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
    let graph = but_workspace::branch::create_reference(
        rc("refs/heads/two-below-main"),
        Anchor::at_segment(r("refs/heads/below-main"), Below),
        &repo,
        &ws,
        &mut meta,
    )?;
    let ws = graph.to_workspace()?;
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

    // Now no new segment can be created anymore, each commit can only have one.
    // the last possible branch without a workspace.
    let err = but_workspace::branch::create_reference(
        rc("refs/heads/another-below-main"),
        Anchor::at_segment(main_ref.as_ref(), Below),
        &repo,
        &ws,
        &mut meta,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Reference 'another-below-main' cannot be created as segment at 12995d783f3ac841a1774e9433ee8e4c1edac576"
    );

    // branch already exists in the workspace, all good.
    let main_ref = r("refs/heads/main");
    let graph = but_workspace::branch::create_reference(main_ref, None, &repo, &ws, &mut meta)?;

    assert!(
        meta.branch(main_ref)?.is_default(),
        "no data was stored, it wasn't stored before either, for independent branches\
            There should be no benefit doing that."
    );
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
        ⌂:0:main <> ✓!
        └── ≡:0:main
            ├── :0:main
            │   └── ·281da94
            ├── 📙:1:below-main
            │   └── ·12995d7
            └── 📙:2:two-below-main
                └── ·3d57fc1
        ");

    // However, creating a dependent branch creates metadata as well.
    let graph = but_workspace::branch::create_reference(
        main_ref,
        Anchor::AtCommit {
            commit_id: main_id.detach(),
            position: Above,
        },
        &repo,
        &ws,
        &mut meta,
    )?;

    assert!(
            !meta.branch(main_ref)?.is_default(),
            "Data is created/updated for dependent branches though,
            which is a way to make segments appear if there were not visible before due to ambiguity."
        );
    insta::assert_snapshot!(graph_workspace(&graph.to_workspace()?), @r"
        ⌂:0:main <> ✓!
        └── ≡📙:0:main
            ├── 📙:0:main
            │   └── ·281da94
            ├── 📙:1:below-main
            │   └── ·12995d7
            └── 📙:2:two-below-main
                └── ·3d57fc1
        ");

    Ok(())
}

#[test]
fn journey_anon_workspace() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-with-3-commits")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @r"
        * 281da94 (HEAD -> main) 3
        * 12995d7 2
        * 3d57fc1 1
        ");

    let id = id_by_rev(&repo, "@~1");
    let graph = but_graph::Graph::from_commit_traversal(id, None, &meta, meta.graph_options())?;
    let ws = graph.to_workspace()?;

    insta::assert_snapshot!(graph_workspace(&ws), @r"
        ⌂:0:DETACHED <> ✓!
        └── ≡:0:anon:
            └── :0:anon:
                ├── ·12995d7 (✓)
                └── ·3d57fc1 (✓)
        ");

    let first_ref = rc("refs/heads/first");
    let first_id = id_by_rev(&repo, "@~2");
    let graph = but_workspace::branch::create_reference(
        first_ref,
        Anchor::AtCommit {
            commit_id: first_id.detach(),
            position: Above,
        },
        &repo,
        &ws,
        &mut meta,
    )?;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
        ⌂:0:DETACHED <> ✓!
        └── ≡:0:anon:
            ├── :0:anon:
            │   └── ·12995d7 (✓)
            └── 📙:2:first
                └── ·3d57fc1 (✓)
        ");

    let new = r("refs/heads/new-independent");
    let err =
        but_workspace::branch::create_reference(new, None, &repo, &ws, &mut meta).unwrap_err();

    assert_eq!(
        err.to_string(),
        "workspace at <anonymous> is missing a base"
    );
    assert!(repo.try_find_reference(new)?.is_none());

    let second_ref = rc("refs/heads/second");
    let second_id = id_by_rev(&repo, "@~1");
    let graph = but_workspace::branch::create_reference(
        second_ref,
        Anchor::AtCommit {
            commit_id: second_id.detach(),
            position: Above,
        },
        &repo,
        &ws,
        &mut meta,
    )?;
    let ws = graph.to_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @r"
        ⌂:0:second <> ✓!
        └── ≡📙:0:second
            ├── 📙:0:second
            │   └── ·12995d7 (✓)
            └── 📙:2:first
                └── ·3d57fc1 (✓)
        ");

    let err =
        but_workspace::branch::create_reference(new, None, &repo, &ws, &mut meta).unwrap_err();

    assert_eq!(
        err.to_string(),
        "workspace at refs/heads/second is missing a base",
        "We need more setup for independent branches"
    );
    assert!(repo.try_find_reference(new)?.is_none());

    // Give the graph a base - but that doesn't work for ad-hoc workspaces yet.
    let graph = but_graph::Graph::from_commit_traversal(
        id,
        None,
        &meta,
        Options {
            extra_target_commit_id: Some(first_id.detach()),
            ..meta.graph_options()
        },
    )?;
    let ws = graph.to_workspace()?;
    // Let's keep the test as reminder, and try to create a branch once there is a base.
    insta::assert_snapshot!(graph_workspace(&ws), @r"
        ⌂:0:second <> ✓!
        └── ≡📙:0:second
            ├── 📙:0:second
            │   └── ·12995d7
            └── 📙:1:first
                └── ·3d57fc1 (✓)
        ");

    Ok(())
}
