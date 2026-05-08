use std::borrow::Cow;

use bstr::ByteSlice;
use but_core::{
    RefMetadata,
    ref_metadata::{StackId, ValueInfo},
};
use but_graph::init::Options;
use but_testsupport::{graph_workspace, id_at, id_by_rev, visualize_commit_graph_all};
use but_workspace::branch::create_reference::{Anchor, Position::*};
use gix::refs::transaction::PreviousValue;

use crate::{
    ref_info::with_workspace_commit::utils::{
        named_read_only_in_memory_scenario, named_writable_scenario,
    },
    utils::{r, rc},
};

mod with_workspace {
    use std::borrow::Cow;

    use but_core::{RefMetadata, ref_metadata::ValueInfo};
    use but_graph::init::Options;
    use but_meta::VirtualBranchesTomlMetadata;
    use but_testsupport::{graph_workspace, id_at, id_by_rev, visualize_commit_graph_all};
    use but_workspace::branch::create_reference::{Anchor, Position::*};

    use crate::{
        branch::create_reference::stack_id_for_name,
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
        let ws = graph.into_workspace()?;

        // And even though setting an extra-target works like it should, i.e a simulated target
        // which we can store in absence of a selected target branch…
        insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43");

        // …we chose to work with an open-ended workspace just to struggle more.
        meta.data_mut()
            .default_target
            .as_mut()
            .expect("always set to have workspace")
            .sha = gix::hash::Kind::Sha1.null();
        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.into_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
        └── ≡:1:main
            └── :1:main
                └── ·3183e43 (🏘️)
        ");

        let new_name = rc("refs/heads/A");
        let err = but_workspace::branch::create_reference(
            new_name,
            None, /* anchor */
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
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
        let ws = graph.into_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43");

        let a_ref = r("refs/heads/A");
        let ws = but_workspace::branch::create_reference(
            a_ref,
            None, /* anchor */
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )
        .expect("it updates the workspace metadata legitimate the new ref at base");
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:3:A on 3183e43 {41}
            └── 📙:3:A
        ");
        let ws_base = ws.lower_bound.expect("target is set");
        assert_eq!(
            repo.find_reference(a_ref)?.id(),
            ws_base,
            "new stack refs are created on the workspace base"
        );

        let b_ref = r("refs/heads/B");
        let ws = but_workspace::branch::create_reference(
            b_ref,
            None, /* anchor */
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:4:B on 3183e43 {42}
        │   └── 📙:4:B
        └── ≡📙:3:A on 3183e43 {41}
            └── 📙:3:A
        ");

        // Idempotency
        let ws = but_workspace::branch::create_reference(
            b_ref,
            None, /* anchor */
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:4:B on 3183e43 {42}
        │   └── 📙:4:B
        └── ≡📙:3:A on 3183e43 {41}
            └── 📙:3:A
        ");

        let above_a = rc("refs/heads/above-A");
        let ws = but_workspace::branch::create_reference(
            above_a,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(a_ref),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:5:B on 3183e43 {42}
        │   └── 📙:5:B
        └── ≡📙:3:above-A on 3183e43 {41}
            ├── 📙:3:above-A
            └── 📙:4:A
        ");

        let below_b = rc("refs/heads/below-B");
        let ws = but_workspace::branch::create_reference(
            below_b,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:5:B on 3183e43 {42}
        │   ├── 📙:5:B
        │   └── 📙:6:below-B
        └── ≡📙:3:above-A on 3183e43 {41}
            ├── 📙:3:above-A
            └── 📙:4:A
        ");

        // Finally, assure the data looks correct. Can't afford bugs in the translation.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.into_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:5:B on 3183e43 {42}
        │   ├── 📙:5:B
        │   └── 📙:6:below-B
        └── ≡📙:3:above-A on 3183e43 {41}
            ├── 📙:3:above-A
            └── 📙:4:A
        ");

        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, origin/main, main, below-B, above-A, B, A) M1");

        Ok(())
    }

    #[test]
    fn journey_single_branch_segment_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-4-commits")?;
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
        * 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
        * 43f9472 (A) A2
        * 6fdab32 A1
        * bce0c5e (origin/main, main) M2
        * 3183e43 M1
        ");

        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.into_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡:3:A on bce0c5e
            └── :3:A
                ├── ·43f9472 (🏘️)
                └── ·6fdab32 (🏘️)
        ");

        let above_bottom_ref = r("refs/heads/above-bottom");
        let bottom_id = id_by_rev(&repo, ":/A1");
        let ws = but_workspace::branch::create_reference(
            above_bottom_ref,
            Anchor::AtCommit {
                commit_id: bottom_id.detach(),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        // It handles this special case, by creating the necessary workspace metadata
        // if for some reason (like manual building) it's not set.
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡:4:A on bce0c5e {4cf}
            ├── :4:A
            │   └── ·43f9472 (🏘️)
            └── 📙:3:above-bottom
                └── ·6fdab32 (🏘️)
        ");

        let bottom_ref = rc("refs/heads/bottom");
        let ws = but_workspace::branch::create_reference(
            bottom_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_bottom_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡:4:A on bce0c5e {4cf}
            ├── :4:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:3:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:5:bottom
        ");

        let above_a_commit_ref = r("refs/heads/above-A-commit");
        let a_id = id_by_rev(&repo, ":/A");
        let ws = but_workspace::branch::create_reference(
            above_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        // Note how 'Above' *a commit* means directly above, not on top of everything.
        // And as there are now two references on one commit, and one has metadata, the other one doesn't,
        // 'A' is moved to the background.
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡📙:3:above-A-commit on bce0c5e {4cf}
            ├── 📙:3:above-A-commit
            │   └── ·43f9472 (🏘️) ►A
            ├── 📙:4:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:5:bottom
        ");

        // We can, however, restore it simply by putting idempotency.
        let a_ref = rc("refs/heads/A");
        let ws = but_workspace::branch::create_reference(
            a_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        // And 'A' is back, with the desired order correctly restored.
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡📙:5:above-A-commit on bce0c5e {4cf}
            ├── 📙:5:above-A-commit
            ├── 📙:6:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:4:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:7:bottom
        ");

        let above_a_ref = rc("refs/heads/above-A");
        let a_ref = rc("refs/heads/A");
        let ws = but_workspace::branch::create_reference(
            above_a_ref,
            Anchor::AtSegment {
                ref_name: a_ref,
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        // *Above a segment means what one would expect though.
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡📙:5:above-A-commit on bce0c5e {4cf}
            ├── 📙:5:above-A-commit
            ├── 📙:6:above-A
            ├── 📙:7:A
            │   └── ·43f9472 (🏘️)
            ├── 📙:4:above-bottom
            │   └── ·6fdab32 (🏘️)
            └── 📙:8:bottom
        ");

        let below_a_commit_ref = rc("refs/heads/below-A-commit");
        let ws = but_workspace::branch::create_reference(
            below_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡📙:5:above-A-commit on bce0c5e {4cf}
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
        let ws = but_workspace::branch::create_reference(
            below_a_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_a_commit_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        └── ≡📙:5:above-A-commit on bce0c5e {4cf}
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
        let ws = but_workspace::branch::create_reference(
            b_ref,
            None,
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        ├── ≡📙:5:B on bce0c5e {42}
        │   └── 📙:5:B
        └── ≡📙:6:above-A-commit on bce0c5e {4cf}
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
        let ws = but_workspace::branch::create_reference(
            above_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        ├── ≡📙:5:above-B on bce0c5e {42}
        │   ├── 📙:5:above-B
        │   └── 📙:6:B
        └── ≡📙:7:above-A-commit on bce0c5e {4cf}
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
        // (which somewhat counter-intuitively works here) because it's a completely new
        // independent branch.
        let below_b_ref = rc("refs/heads/below-B");
        let ws = but_workspace::branch::create_reference(
            below_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        ├── ≡📙:5:above-B on bce0c5e {42}
        │   ├── 📙:5:above-B
        │   ├── 📙:6:B
        │   └── 📙:7:below-B
        └── ≡📙:8:above-A-commit on bce0c5e {4cf}
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
        let ws = graph.into_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
        ├── ≡📙:5:above-B on bce0c5e {42}
        │   ├── 📙:5:above-B
        │   ├── 📙:6:B
        │   └── 📙:7:below-B
        └── ≡📙:8:above-A-commit on bce0c5e {4cf}
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

        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
        * c2878fb (HEAD -> gitbutler/workspace, A) A2
        * 49d4b34 A1
        * 3183e43 (origin/main, main) M1
        ");

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.into_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:3:A on 3183e43 {0}
            └── 📙:3:A
                ├── ·c2878fb (🏘️)
                └── ·49d4b34 (🏘️)
        ");

        let above_bottom_ref = r("refs/heads/above-bottom");
        let bottom_id = id_by_rev(&repo, ":/A1");
        let ws = but_workspace::branch::create_reference(
            above_bottom_ref,
            Anchor::AtCommit {
                commit_id: bottom_id.detach(),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:4:A on 3183e43 {0}
            ├── 📙:4:A
            │   └── ·c2878fb (🏘️)
            └── 📙:3:above-bottom
                └── ·49d4b34 (🏘️)
        ");

        let bottom_ref = rc("refs/heads/bottom");
        let ws = but_workspace::branch::create_reference(
            bottom_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_bottom_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        // We can create branches that would be on the base.
        // There are
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:4:A on 3183e43 {0}
            ├── 📙:4:A
            │   └── ·c2878fb (🏘️)
            ├── 📙:3:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:5:bottom
        ");

        let above_a_commit_ref = r("refs/heads/above-A-commit");
        let a_id = id_by_rev(&repo, ":/A");
        let ws = but_workspace::branch::create_reference(
            above_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        // Note how 'Above' *a commit* means directly above, not on top of everything.
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:5:A on 3183e43 {0}
            ├── 📙:5:A
            ├── 📙:6:above-A-commit
            │   └── ·c2878fb (🏘️)
            ├── 📙:3:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:7:bottom
        ");

        let above_a_ref = rc("refs/heads/above-A");
        let a_ref = rc("refs/heads/A");
        let ws = but_workspace::branch::create_reference(
            above_a_ref,
            Anchor::AtSegment {
                ref_name: a_ref,
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        // *Above a segment means what one would expect though.
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:5:above-A on 3183e43 {0}
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
        let ws = but_workspace::branch::create_reference(
            above_a_ref,
            Anchor::AtSegment {
                ref_name: a_ref,
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:5:above-A on 3183e43 {0}
            ├── 📙:5:above-A
            ├── 📙:6:A
            ├── 📙:7:above-A-commit
            │   └── ·c2878fb (🏘️)
            ├── 📙:3:above-bottom
            │   └── ·49d4b34 (🏘️)
            └── 📙:8:bottom
        ");

        let below_a_commit_ref = rc("refs/heads/below-A-commit");
        let ws = but_workspace::branch::create_reference(
            below_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:5:above-A on 3183e43 {0}
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
        let ws = but_workspace::branch::create_reference(
            below_a_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_a_commit_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:5:above-A on 3183e43 {0}
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
        let ws = but_workspace::branch::create_reference(
            b_ref,
            None,
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:5:B on 3183e43 {42}
        │   └── 📙:5:B
        └── ≡📙:6:above-A on 3183e43 {0}
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
        let ws = but_workspace::branch::create_reference(
            above_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:5:above-B on 3183e43 {42}
        │   ├── 📙:5:above-B
        │   └── 📙:6:B
        └── ≡📙:7:above-A on 3183e43 {0}
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
        // (which somewhat counter-intuitively works here) because it's a completely new
        // independent branch.
        let below_b_ref = rc("refs/heads/below-B");
        let ws = but_workspace::branch::create_reference(
            below_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:5:above-B on 3183e43 {42}
        │   ├── 📙:5:above-B
        │   ├── 📙:6:B
        │   └── 📙:7:below-B
        └── ≡📙:8:above-A on 3183e43 {0}
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
        let ws = graph.into_workspace()?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:5:above-B on 3183e43 {42}
        │   ├── 📙:5:above-B
        │   ├── 📙:6:B
        │   └── 📙:7:below-B
        └── ≡📙:8:above-A on 3183e43 {0}
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

        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
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
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
        * c2878fb (HEAD -> gitbutler/workspace, A) A2
        * 49d4b34 A1
        * 3183e43 (origin/main, main) M1
        ");

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.into_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:3:A on 3183e43 {0}
            └── 📙:3:A
                ├── ·c2878fb (🏘️)
                └── ·49d4b34 (🏘️)
        ");

        let bottom_ref = rc("refs/heads/bottom");
        let bottom_id = id_by_rev(&repo, ":/A1");
        let ws = but_workspace::branch::create_reference(
            bottom_ref,
            Anchor::AtCommit {
                commit_id: bottom_id.detach(),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:3:A on 3183e43 {0}
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
        let ws = graph.into_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:4:B on 3183e43 {1}
        │   └── 📙:4:B
        │       └── ·f57c528 (🏘️)
        └── ≡📙:3:A on 3183e43 {0}
            └── 📙:3:A
                └── ·49d4b34 (🏘️)
        ");

        let bottom_ref_a = rc("refs/heads/a-bottom");
        let bottom_a_id = id_by_rev(&repo, ":/A1");
        let ws = but_workspace::branch::create_reference(
            bottom_ref_a,
            Anchor::AtCommit {
                commit_id: bottom_a_id.detach(),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:4:B on 3183e43 {1}
        │   └── 📙:4:B
        │       └── ·f57c528 (🏘️)
        └── ≡📙:3:A on 3183e43 {0}
            ├── 📙:3:A
            │   └── ·49d4b34 (🏘️)
            └── 📙:5:a-bottom
        ");

        let bottom_ref_b = rc("refs/heads/b-bottom");
        let bottom_b_id = id_by_rev(&repo, ":/B1");
        let ws = but_workspace::branch::create_reference(
            bottom_ref_b,
            Anchor::AtCommit {
                commit_id: bottom_b_id.detach(),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        ├── ≡📙:4:B on 3183e43 {1}
        │   ├── 📙:4:B
        │   │   └── ·f57c528 (🏘️)
        │   └── 📙:6:b-bottom
        └── ≡📙:3:A on 3183e43 {0}
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
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
        * bce0c5e (HEAD -> gitbutler/workspace, main) M2
        * 3183e43 (origin/main) M1
        ");

        let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
        let ws = graph.into_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on bce0c5e");

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
                stack_id_for_name,
                None,
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
        insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
        * bba50eb (extra) E1
        * c2878fb (HEAD -> gitbutler/workspace, A) A2
        * 49d4b34 A1
        * 3183e43 (origin/main, main) M1
        ");

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

        let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
        let ws = graph.into_workspace()?;

        insta::assert_snapshot!(graph_workspace(&ws), @"
        📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
        └── ≡📙:3:A on 3183e43 {0}
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
                stack_id_for_name,
                None,
            )
            .unwrap_err();

            assert_eq!(
                err.to_string(),
                "Branch 'gitbutler/workspace' cannot be created: the target commit (49d4b34f36239228b64ee758be8f58849bac02d5) already belongs to another branch in the workspace. Each commit can only belong to one branch at a time.",
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
                stack_id_for_name,
                None,
            )
            .unwrap_err();

            assert_eq!(
                err.to_string(),
                "Branch 'gitbutler/workspace' cannot be created: the target commit (c2878fb5dda8243a099a0353452d497d906bc6b5) already belongs to another branch in the workspace. Each commit can only belong to one branch at a time.",
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
            stack_id_for_name,
            None,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Reference 'extra' already exists and is outside the workspace",
            "Existing refs outside the workspace should fail explicitly instead of surfacing the generic segment error"
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
            stack_id_for_name,
            None,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "Couldn't find any stack that contained the branch named 'bogus'",
            "It yells loudly if the inputs don't match up - anchors must always be in the workspace."
        );
        Ok(())
    }

    /// Creating a new branch in a workspace that has proper metadata stacks plus
    /// extra "ghost" stacks in the TOML that don't correspond to any actual git refs.
    #[test]
    fn create_branch_with_metadata_stacks() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("ws-ref-ws-commit-two-stacks")?;

        // Register stacks A and B in the metadata (simulating real production state)
        add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);
        // Add a "ghost" stack that was previously unapplied/deleted but still in TOML
        add_stack_with_segments(&mut meta, 3, "deleted-branch", StackState::Inactive, &[]);

        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.into_workspace()?;

        let c_ref = r("refs/heads/C");
        let ws = but_workspace::branch::create_reference(
            c_ref,
            None,
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        ws.find_segment_and_stack_by_refname(c_ref)
            .expect("C should be a standalone segment");

        Ok(())
    }

    /// When workspace metadata has a bloated stack (e.g. ["A", "C", "ghost1", ...])
    /// and the user creates an independent branch named "C", `update_workspace_metadata`
    /// finds "C" already in A's stack and marks A's stack as Merged instead of creating
    /// a new independent stack. C should still appear as its own segment.
    #[test]
    fn create_independent_branch_reusing_name_from_bloated_stack() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("ws-ref-ws-commit-two-stacks")?;

        // Set up metadata that resembles production: stack A has accumulated
        // stale branch entries, including one named "C" from a previous dependent branch.
        add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["C", "ghost1"]);
        add_stack_with_segments(&mut meta, 2, "B", StackState::InWorkspace, &[]);

        let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
        let ws = graph.into_workspace()?;

        // Try creating an independent branch "C" — same name as the stale entry in A's stack.
        let c_ref = r("refs/heads/C");
        let ws = but_workspace::branch::create_reference(
            c_ref,
            None,
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        ws.find_segment_and_stack_by_refname(c_ref)
            .expect("C should be a standalone segment in the workspace");

        Ok(())
    }
}

#[test]
fn errors() -> anyhow::Result<()> {
    let (repo, mut meta) = named_read_only_in_memory_scenario("unborn-empty", "")?;
    let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
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
        stack_id_for_name,
        None,
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "Cannot create reference on unborn branch");

    let (repo, mut meta) =
        named_read_only_in_memory_scenario("with-remotes-no-workspace", "remote")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 89cc2d3 (A) change in A
    * d79bba9 new file in A
    * c166d42 (HEAD -> main) init-integration
    ");

    let graph = but_graph::Graph::from_head(&repo, &*meta, Options::limited())?;
    let ws = graph.into_workspace()?;

    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
            └── ·c166d42
    ");

    let (id, ref_name) = id_at(&repo, "main");
    for anchor in [
        Anchor::at_id(id, Below),
        Anchor::at_segment(ref_name.as_ref(), Below),
    ] {
        // Below first in history
        let err = but_workspace::branch::create_reference(
            new_name,
            anchor,
            &repo,
            &ws,
            &mut *meta,
            stack_id_for_name,
            None,
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

    // Misaligned workspace - commit not included.
    let (id, ref_name) = id_at(&repo, "A");
    for anchor in [Anchor::at_id(id, Below), Anchor::at_id(id, Above)] {
        let err = but_workspace::branch::create_reference(
            new_name,
            anchor,
            &repo,
            &ws,
            &mut *meta,
            stack_id_for_name,
            None,
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
    let (a_id, a_ref) = id_at(&repo, "A");
    for anchor in [
        (Anchor::at_segment(a_ref.as_ref(), Below)),
        (Anchor::at_segment(a_ref.as_ref(), Above)),
    ] {
        let err = but_workspace::branch::create_reference(
            new_name,
            anchor,
            &repo,
            &ws,
            &mut *meta,
            stack_id_for_name,
            None,
        )
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
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:A <> ✓!
    └── ≡:0:A {1}
        ├── :0:A
        │   ├── ·89cc2d3
        │   └── ·d79bba9
        └── :1:main[🌳]
            └── ·c166d42
    ");

    // Create the same ref at a different location
    let a_ref = r("refs/heads/A");
    let (main_id, main_ref) = id_at(&repo, "main");
    for anchor in [
        (Anchor::at_segment(main_ref.as_ref(), Above)),
        (Anchor::at_id(main_id, Above)),
    ] {
        let err = but_workspace::branch::create_reference(
            a_ref,
            anchor,
            &repo,
            &ws,
            &mut *meta,
            stack_id_for_name,
            None,
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
        a_id,
        a_ref.to_owned(),
        &*meta,
        Options {
            extra_target_commit_id: main_id.detach().into(),
            commits_limit_hint: 0.into(),
            ..Options::limited()
        },
    )?;
    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:A <> ✓! on 89cc2d3
    └── ≡:0:A {1}
        └── :0:A
    ");

    let (a_id, _a_ref_owned) = id_at(&repo, "A");
    for (anchor, expected_err) in [
        (
            Anchor::at_segment(a_ref, Below),
            "Cannot create reference on unborn branch",
        ),
        (
            Anchor::at_id(a_id, Below),
            "Commit 89cc2d303514654e9cab2d05b9af08b420a740c1 isn't part of the workspace",
        ),
    ] {
        let err = but_workspace::branch::create_reference(
            new_name,
            anchor.clone(),
            &repo,
            &ws,
            &mut *meta,
            stack_id_for_name,
            None,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            expected_err,
            "{anchor:?}: TODO: make these error messages consistent, and one might argue that this makes it hard to create refs on such bases."
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
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 281da94 (HEAD -> main) 3
    * 12995d7 2
    * 3d57fc1 1
    ");

    let graph = but_graph::Graph::from_head(&repo, &meta, Default::default())?;
    let ws = graph.into_workspace()?;

    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        └── :0:main[🌳]
            ├── ·281da94
            ├── ·12995d7
            └── ·3d57fc1
    ");

    let (main_id, main_ref) = id_at(&repo, "main");
    let new_name = r("refs/heads/below-main");
    let ws = but_workspace::branch::create_reference(
        new_name,
        Anchor::at_segment(main_ref.as_ref(), Below),
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )
    .expect("this works as the branch is unique");

    // We always add metadata to new branches.
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        ├── :0:main[🌳]
        │   └── ·281da94
        └── 📙:1:below-main
            ├── ·12995d7
            └── ·3d57fc1
    ");
    let md = meta.branch(new_name)?;
    assert!(!md.is_default(), "It should have set the date at least");
    assert!(md.ref_info.updated_at.is_none());
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
    let ws = but_workspace::branch::create_reference(
        new_name,
        Anchor::at_id(main_id, Below),
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        ├── :0:main[🌳]
        │   └── ·281da94
        └── 📙:1:below-main
            ├── ·12995d7
            └── ·3d57fc1
    ");

    // the last possible branch without a workspace.
    let ws = but_workspace::branch::create_reference(
        rc("refs/heads/two-below-main"),
        Anchor::at_segment(r("refs/heads/below-main"), Below),
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        ├── :0:main[🌳]
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
        stack_id_for_name,
        None,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Branch 'another-below-main' cannot be created: the target commit (12995d783f3ac841a1774e9433ee8e4c1edac576) already belongs to another branch in the workspace. Each commit can only belong to one branch at a time."
    );

    // branch already exists in the workspace, all good.
    let main_ref = r("refs/heads/main");
    let ws = but_workspace::branch::create_reference(
        main_ref,
        None,
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;

    assert!(
        meta.branch(main_ref)?.is_default(),
        "no data was stored, it wasn't stored before either, for independent branches\
            There should be no benefit doing that."
    );
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡:0:main[🌳] {1}
        ├── :0:main[🌳]
        │   └── ·281da94
        ├── 📙:1:below-main
        │   └── ·12995d7
        └── 📙:2:two-below-main
            └── ·3d57fc1
    ");

    // However, creating a dependent branch creates metadata as well.
    let ws = but_workspace::branch::create_reference(
        main_ref,
        Anchor::AtCommit {
            commit_id: main_id.detach(),
            position: Above,
        },
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;

    assert!(
        !meta.branch(main_ref)?.is_default(),
        "Data is created/updated for dependent branches though,
            which is a way to make segments appear if there were not visible before due to ambiguity."
    );
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:main[🌳] <> ✓!
    └── ≡📙:0:main[🌳] {1}
        ├── 📙:0:main[🌳]
        │   └── ·281da94
        ├── 📙:1:below-main
        │   └── ·12995d7
        └── 📙:2:two-below-main
            └── ·3d57fc1
    ");

    Ok(())
}

#[test]
fn existing_git_ref_inside_workspace_is_adopted() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-4-commits")?;
    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
    let ws = graph.into_workspace()?;

    let test_ref = r("refs/heads/created-with-git");
    let target_id = id_by_rev(&repo, ":/A1").detach();
    repo.reference(
        test_ref,
        target_id,
        PreviousValue::Any,
        "manual branch created with git",
    )?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 43f9472 (A) A2
    * 6fdab32 (created-with-git) A1
    * bce0c5e (origin/main, main) M2
    * 3183e43 M1
    ");

    let ws = but_workspace::branch::create_reference(
        test_ref,
        None,
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡:4:A on bce0c5e {632}
        ├── :4:A
        │   └── ·43f9472 (🏘️)
        └── 📙:3:created-with-git
            └── ·6fdab32 (🏘️)
    ");

    Ok(())
}

#[test]
fn journey_anon_workspace() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-with-3-commits")?;
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 281da94 (HEAD -> main) 3
    * 12995d7 2
    * 3d57fc1 1
    ");

    let id = id_by_rev(&repo, "@~1");
    let graph = but_graph::Graph::from_commit_traversal(id, None, &meta, Default::default())?;
    let ws = graph.into_workspace()?;

    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:DETACHED <> ✓!
    └── ≡:0:anon: {1}
        └── :0:anon:
            ├── ·12995d7
            └── ·3d57fc1
    ");

    let first_ref = rc("refs/heads/first");
    let first_id = id_by_rev(&repo, "@~2");
    let ws = but_workspace::branch::create_reference(
        first_ref,
        Anchor::AtCommit {
            commit_id: first_id.detach(),
            position: Above,
        },
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:DETACHED <> ✓!
    └── ≡:0:anon: {1}
        ├── :0:anon:
        │   └── ·12995d7
        └── 📙:1:first
            └── ·3d57fc1
    ");

    let new = r("refs/heads/new-independent");
    let err = but_workspace::branch::create_reference(
        new,
        None,
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "workspace at <anonymous> is missing a base"
    );
    assert!(repo.try_find_reference(new)?.is_none());

    let second_ref = rc("refs/heads/second");
    let second_id = id_by_rev(&repo, "@~1");
    let ws = but_workspace::branch::create_reference(
        second_ref,
        Anchor::AtCommit {
            commit_id: second_id.detach(),
            position: Above,
        },
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:second <> ✓!
    └── ≡📙:0:second {1}
        ├── 📙:0:second
        │   └── ·12995d7
        └── 📙:1:first
            └── ·3d57fc1
    ");

    let err = but_workspace::branch::create_reference(
        new,
        None,
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )
    .unwrap_err();

    assert_eq!(
        err.to_string(),
        "workspace at refs/heads/second is missing a base",
        "We need more setup for independent branches"
    );
    assert!(repo.try_find_reference(new)?.is_none());

    // Give the graph a base
    let graph = but_graph::Graph::from_commit_traversal(
        id,
        None,
        &meta,
        Options {
            extra_target_commit_id: Some(first_id.detach()),
            ..Default::default()
        },
    )?;
    let ws = graph.into_workspace()?;
    // And the extra-target serves as base also in single-branch mode.
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:0:second <> ✓! on 3d57fc1
    └── ≡📙:0:second on 3d57fc1 {1}
        └── 📙:0:second
            └── ·12995d7
    ");

    Ok(())
}

/// Creating a new independent branch in a workspace that already contains
/// two stacks with commits should succeed.
#[test]
fn create_independent_branch_in_workspace_with_two_stacks() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = named_writable_scenario("ws-ref-ws-commit-two-stacks")?;

    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
    let ws = graph.into_workspace()?;

    let c_ref = r("refs/heads/C");
    let ws = but_workspace::branch::create_reference(
        c_ref,
        None,
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;

    let (stack, segment) = ws
        .find_segment_and_stack_by_refname(c_ref)
        .expect("C should be a standalone segment in the workspace");
    assert!(
        stack.id.is_some(),
        "new independent stack should have an ID"
    );
    assert_eq!(
        segment.ref_name().map(|rn| rn.shorten().to_string()),
        Some("C".to_string()),
    );

    Ok(())
}

/// Reproducer: creating a new independent branch when the workspace already
/// contains an empty stack (one that points at the base commit with no
/// commits of its own).
#[test]
fn create_independent_branch_with_existing_empty_stack() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = named_writable_scenario("ws-with-empty-stack")?;

    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
    let ws = graph.into_workspace()?;

    // B is an empty stack already pointing at the base commit.
    // Creating a new independent branch C should succeed.
    let c_ref = r("refs/heads/C");
    let result = but_workspace::branch::create_reference(
        c_ref,
        None,
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    );

    let ws = result?;
    let (stack, segment) = ws
        .find_segment_and_stack_by_refname(c_ref)
        .expect("C should be a standalone segment in the workspace");
    assert!(
        stack.id.is_some(),
        "new independent stack should have an ID"
    );
    assert_eq!(
        segment.ref_name().map(|rn| rn.shorten().to_string()),
        Some("C".to_string()),
    );

    Ok(())
}

/// Creating a new independent branch when origin/main has
/// advanced past the fork point of existing stacks.
#[test]
fn create_independent_branch_with_advanced_remote() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) =
        named_writable_scenario("ws-ref-ws-commit-two-stacks-advanced-remote")?;

    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
    let ws = graph.into_workspace()?;

    let c_ref = r("refs/heads/C");
    let ws = but_workspace::branch::create_reference(
        c_ref,
        None,
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;

    ws.find_segment_and_stack_by_refname(c_ref)
        .expect("C should be a standalone segment in the workspace");

    Ok(())
}

/// Reproducer: creating a new independent branch when there are extra
/// commits on top of the workspace commit (outside commits).
#[test]
fn create_independent_branch_with_advanced_workspace() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = named_writable_scenario("ws-ref-ws-commit-one-stack-ws-advanced")?;

    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
    let ws = graph.into_workspace()?;

    let c_ref = r("refs/heads/C");
    let result = but_workspace::branch::create_reference(
        c_ref,
        None,
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    );

    match result {
        Ok(ws) => {
            ws.find_segment_and_stack_by_refname(c_ref)
                .expect("C should be a standalone segment in the workspace");
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already belongs to another branch") {
                panic!("BUG REPRODUCED: {msg}");
            } else {
                return Err(e);
            }
        }
    }

    Ok(())
}

/// Creating a second independent branch at the same base commit as an existing one.
/// Both branches have metadata at the base, so disambiguation fails during traversal.
/// The `workspace_upgrades` post-processing must recover by creating separate segments.
#[test]
fn create_second_independent_branch_at_same_base() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = named_writable_scenario("ws-ref-ws-commit-two-stacks")?;

    let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
    let ws = graph.into_workspace()?;

    // First, create independent branch D at the base.
    let d_ref = r("refs/heads/D");
    let ws = but_workspace::branch::create_reference(
        d_ref,
        None,
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;

    ws.find_segment_and_stack_by_refname(d_ref)
        .expect("D should be a standalone segment in the workspace");

    // Now create a SECOND independent branch E at the same base commit.
    let e_ref = r("refs/heads/E");
    let ws = but_workspace::branch::create_reference(
        e_ref,
        None,
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;

    // E should also appear as its own stack.
    let (stack, segment) = ws
        .find_segment_and_stack_by_refname(e_ref)
        .expect("E should be a standalone segment in the workspace");
    assert!(
        stack.id.is_some(),
        "new independent stack should have an ID"
    );
    assert_eq!(
        segment.ref_name().map(|rn| rn.shorten().to_string()),
        Some("E".to_string()),
    );

    Ok(())
}

fn stack_id_for_name(rn: &gix::refs::FullNameRef) -> StackId {
    StackId::from_number_for_testing(rn.shorten().chars().map(|c| c as u128).sum())
}

/// Parameterized test infrastructure for `create_reference` across many workspace configurations.
///
/// The builder creates a workspace from a fixture, configures metadata with various
/// levels of "bloat" (ghost entries, stale stacks), and then attempts to create an
/// independent branch — asserting it succeeds.
mod parameterized {
    use but_graph::init::Options;
    use but_testsupport::graph_workspace;

    use crate::{
        branch::create_reference::stack_id_for_name,
        ref_info::with_workspace_commit::utils::{
            StackState, add_stack_with_segments, named_writable_scenario,
        },
        utils::r,
    };

    /// Which git fixture to use as the base repository structure.
    #[derive(Debug, Clone, Copy)]
    enum Fixture {
        /// Two independent stacks (A, B) each with one commit, forking from M.
        TwoStacks,
        /// One stack (B on top of A), both with commits, forking from M.
        OneStack,
        /// Two stacks (A with commit, B empty at base), forking from M.
        EmptyStack,
        /// Two stacks with origin/main advanced past the fork point.
        AdvancedRemote,
    }

    impl Fixture {
        fn scenario_name(self) -> &'static str {
            match self {
                Fixture::TwoStacks => "ws-ref-ws-commit-two-stacks",
                Fixture::OneStack => "ws-ref-ws-commit-one-stack",
                Fixture::EmptyStack => "ws-with-empty-stack",
                Fixture::AdvancedRemote => "ws-ref-ws-commit-two-stacks-advanced-remote",
            }
        }

        /// Return the branch names that exist as actual git refs in this fixture
        /// (excluding main/workspace).
        fn real_branch_names(self) -> &'static [&'static str] {
            match self {
                Fixture::TwoStacks | Fixture::AdvancedRemote | Fixture::EmptyStack => &["A", "B"],
                Fixture::OneStack => &["A", "B"],
            }
        }
    }

    /// How to configure metadata for each real stack.
    #[derive(Debug)]
    struct StackMetaConfig {
        /// Extra ghost segment names to add to this stack's metadata.
        ghost_segments: Vec<&'static str>,
        /// Whether the stack is active or inactive in workspace metadata.
        in_workspace: bool,
    }

    impl StackMetaConfig {
        fn clean() -> Self {
            StackMetaConfig {
                ghost_segments: vec![],
                in_workspace: true,
            }
        }

        fn with_ghosts(ghosts: &[&'static str]) -> Self {
            StackMetaConfig {
                ghost_segments: ghosts.to_vec(),
                in_workspace: true,
            }
        }

        #[allow(dead_code)]
        fn inactive() -> Self {
            StackMetaConfig {
                ghost_segments: vec![],
                in_workspace: false,
            }
        }
    }

    /// Configuration for extra stacks that don't correspond to any real git refs.
    #[derive(Debug)]
    struct ExtraStack {
        name: &'static str,
        segments: Vec<&'static str>,
        in_workspace: bool,
    }

    /// Full scenario configuration.
    #[derive(Debug)]
    struct Scenario {
        fixture: Fixture,
        /// Metadata config for each real branch, indexed by position in `fixture.real_branch_names()`.
        stack_configs: Vec<StackMetaConfig>,
        /// Additional stacks in metadata that don't correspond to real git refs.
        extra_stacks: Vec<ExtraStack>,
        /// Name of the branch to create.
        new_branch_name: &'static str,
    }

    impl Scenario {
        fn run(&self) -> anyhow::Result<()> {
            let (_tmp, repo, mut meta) = named_writable_scenario(self.fixture.scenario_name())?;
            let real_names = self.fixture.real_branch_names();

            // Configure metadata for real stacks.
            for (i, (name, config)) in real_names.iter().zip(&self.stack_configs).enumerate() {
                let id = (i + 1) as u128;
                let state = if config.in_workspace {
                    StackState::InWorkspace
                } else {
                    StackState::Inactive
                };
                add_stack_with_segments(&mut meta, id, name, state, &config.ghost_segments);
            }

            // Add extra stacks.
            for (i, extra) in self.extra_stacks.iter().enumerate() {
                let id = (100 + i) as u128;
                let state = if extra.in_workspace {
                    StackState::InWorkspace
                } else {
                    StackState::Inactive
                };
                add_stack_with_segments(&mut meta, id, extra.name, state, &extra.segments);
            }

            let graph = but_graph::Graph::from_head(&repo, &meta, Options::limited())?;
            let ws = graph.into_workspace()?;
            let before = graph_workspace(&ws);

            let ref_string = format!("refs/heads/{}", self.new_branch_name);
            let new_ref = r(&ref_string);
            let result = but_workspace::branch::create_reference(
                new_ref,
                None,
                &repo,
                &ws,
                &mut meta,
                stack_id_for_name,
                None,
            );

            match result {
                Ok(ws) => {
                    let (_stack, segment) = ws
                        .find_segment_and_stack_by_refname(new_ref)
                        .unwrap_or_else(|| {
                            panic!(
                                "Branch '{}' should be findable in the workspace after creation.\n\
                             Workspace before:\n{before}\n\
                             Workspace after:\n{}",
                                self.new_branch_name,
                                graph_workspace(&ws),
                            )
                        });
                    assert_eq!(
                        segment.ref_name().map(|rn| rn.shorten().to_string()),
                        Some(self.new_branch_name.to_string()),
                        "Segment ref name should match the created branch"
                    );
                    Ok(())
                }
                Err(e) => {
                    panic!(
                        "create_reference failed for scenario {self:?}:\n  error: {e}\n  \
                         workspace before:\n{before}",
                    );
                }
            }
        }
    }

    // --- Parameterized tests ---

    /// Clean metadata, no ghosts — baseline for each fixture.
    mod clean_metadata {
        use super::*;

        #[test]
        fn two_stacks() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![StackMetaConfig::clean(), StackMetaConfig::clean()],
                extra_stacks: vec![],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn one_stack() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::OneStack,
                stack_configs: vec![StackMetaConfig::clean(), StackMetaConfig::clean()],
                extra_stacks: vec![],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn empty_stack() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::EmptyStack,
                stack_configs: vec![StackMetaConfig::clean(), StackMetaConfig::clean()],
                extra_stacks: vec![],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn advanced_remote() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::AdvancedRemote,
                stack_configs: vec![StackMetaConfig::clean(), StackMetaConfig::clean()],
                extra_stacks: vec![],
                new_branch_name: "C",
            }
            .run()
        }
    }

    /// Stacks with ghost/stale segment entries that don't correspond to real refs.
    mod bloated_metadata {
        use super::*;

        #[test]
        fn one_ghost_in_first_stack() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![
                    StackMetaConfig::with_ghosts(&["ghost1"]),
                    StackMetaConfig::clean(),
                ],
                extra_stacks: vec![],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn many_ghosts_in_first_stack() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![
                    StackMetaConfig::with_ghosts(&[
                        "ghost1", "ghost2", "ghost3", "ghost4", "ghost5",
                    ]),
                    StackMetaConfig::clean(),
                ],
                extra_stacks: vec![],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn ghosts_in_both_stacks() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![
                    StackMetaConfig::with_ghosts(&["ghost1", "ghost2"]),
                    StackMetaConfig::with_ghosts(&["ghost3", "ghost4"]),
                ],
                extra_stacks: vec![],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn new_branch_name_collides_with_ghost() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![
                    StackMetaConfig::with_ghosts(&["C", "ghost1"]),
                    StackMetaConfig::clean(),
                ],
                extra_stacks: vec![],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn new_branch_name_collides_with_ghost_in_second_stack() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![
                    StackMetaConfig::clean(),
                    StackMetaConfig::with_ghosts(&["C"]),
                ],
                extra_stacks: vec![],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn heavily_bloated_one_stack() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::OneStack,
                stack_configs: vec![
                    StackMetaConfig::clean(),
                    StackMetaConfig::with_ghosts(&[
                        "g1", "g2", "g3", "g4", "g5", "g6", "g7", "g8", "g9", "g10", "g11", "g12",
                        "g13", "g14", "g15",
                    ]),
                ],
                extra_stacks: vec![],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn ghost_collides_with_advanced_remote() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::AdvancedRemote,
                stack_configs: vec![
                    StackMetaConfig::with_ghosts(&["C", "ghost1"]),
                    StackMetaConfig::with_ghosts(&["ghost2"]),
                ],
                extra_stacks: vec![],
                new_branch_name: "C",
            }
            .run()
        }
    }

    /// Extra stacks in metadata that have no real git refs at all.
    mod extra_stacks {
        use super::*;

        #[test]
        fn one_inactive_extra_stack() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![StackMetaConfig::clean(), StackMetaConfig::clean()],
                extra_stacks: vec![ExtraStack {
                    name: "deleted-branch",
                    segments: vec![],
                    in_workspace: false,
                }],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn active_extra_stack_with_new_branch_name() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![StackMetaConfig::clean(), StackMetaConfig::clean()],
                extra_stacks: vec![ExtraStack {
                    name: "C",
                    segments: vec![],
                    in_workspace: true,
                }],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn inactive_extra_stack_with_new_branch_name() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![StackMetaConfig::clean(), StackMetaConfig::clean()],
                extra_stacks: vec![ExtraStack {
                    name: "C",
                    segments: vec![],
                    in_workspace: false,
                }],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn many_inactive_extra_stacks() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![StackMetaConfig::clean(), StackMetaConfig::clean()],
                extra_stacks: vec![
                    ExtraStack {
                        name: "old1",
                        segments: vec![],
                        in_workspace: false,
                    },
                    ExtraStack {
                        name: "old2",
                        segments: vec!["old2a", "old2b"],
                        in_workspace: false,
                    },
                    ExtraStack {
                        name: "old3",
                        segments: vec![],
                        in_workspace: false,
                    },
                ],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn extra_stack_with_ghosts_and_collision() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![
                    StackMetaConfig::with_ghosts(&["ghost1"]),
                    StackMetaConfig::clean(),
                ],
                extra_stacks: vec![ExtraStack {
                    name: "stale-stack",
                    segments: vec!["C", "another-ghost"],
                    in_workspace: true,
                }],
                new_branch_name: "C",
            }
            .run()
        }
    }

    /// Combinations: bloated metadata + extra stacks + different fixtures.
    mod combined {
        use super::*;

        #[test]
        fn bloated_plus_inactive_stacks_two_stacks() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![
                    StackMetaConfig::with_ghosts(&["C", "ghost1", "ghost2"]),
                    StackMetaConfig::with_ghosts(&["ghost3"]),
                ],
                extra_stacks: vec![
                    ExtraStack {
                        name: "old-stack",
                        segments: vec!["old-seg"],
                        in_workspace: false,
                    },
                    ExtraStack {
                        name: "another-old",
                        segments: vec![],
                        in_workspace: false,
                    },
                ],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn bloated_plus_inactive_empty_stack() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::EmptyStack,
                stack_configs: vec![
                    StackMetaConfig::with_ghosts(&["C"]),
                    StackMetaConfig::clean(),
                ],
                extra_stacks: vec![ExtraStack {
                    name: "deleted",
                    segments: vec!["seg1", "seg2"],
                    in_workspace: false,
                }],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn bloated_plus_inactive_advanced_remote() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::AdvancedRemote,
                stack_configs: vec![
                    StackMetaConfig::with_ghosts(&["ghost1"]),
                    StackMetaConfig::with_ghosts(&["C", "ghost2"]),
                ],
                extra_stacks: vec![ExtraStack {
                    name: "archived",
                    segments: vec![],
                    in_workspace: false,
                }],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn collision_in_both_real_and_extra_stack() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::TwoStacks,
                stack_configs: vec![
                    StackMetaConfig::with_ghosts(&["C"]),
                    StackMetaConfig::clean(),
                ],
                extra_stacks: vec![ExtraStack {
                    name: "extra",
                    segments: vec!["C"],
                    in_workspace: true,
                }],
                new_branch_name: "C",
            }
            .run()
        }

        #[test]
        fn one_stack_heavily_bloated_with_extras() -> anyhow::Result<()> {
            Scenario {
                fixture: Fixture::OneStack,
                stack_configs: vec![
                    StackMetaConfig::with_ghosts(&["C", "g1", "g2", "g3"]),
                    StackMetaConfig::with_ghosts(&["g4", "g5", "g6", "g7", "g8", "g9", "g10"]),
                ],
                extra_stacks: vec![
                    ExtraStack {
                        name: "inactive1",
                        segments: vec!["i1a", "i1b", "i1c"],
                        in_workspace: false,
                    },
                    ExtraStack {
                        name: "inactive2",
                        segments: vec![],
                        in_workspace: false,
                    },
                ],
                new_branch_name: "C",
            }
            .run()
        }
    }
}
