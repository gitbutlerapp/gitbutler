use bstr::ByteSlice;
use but_core::{
    RefMetadata,
    ref_metadata::{StackId, ValueInfo},
};
use but_graph::init::Options;
use but_meta::BranchOrderMetadata;
use but_testsupport::{graph_workspace, id_at, id_by_rev, visualize_commit_graph_all};
use but_workspace::branch::create_reference::{Anchor, Position::*};
use gix::refs::transaction::PreviousValue;
use std::borrow::Cow;

use crate::{
    ref_info::with_workspace_commit::utils::{
        named_read_only_in_memory_scenario, named_writable_scenario, project_meta,
    },
    utils::{r, rc},
};

fn branch_order_meta(repo: &gix::Repository) -> anyhow::Result<BranchOrderMetadata> {
    BranchOrderMetadata::from_paths(repo.path().join("virtual-branches.toml"), repo.path())
}

mod with_workspace {
    use snapbox::IntoData;
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
            named_writable_scenario, named_writable_scenario_with_description, project_meta,
        },
        utils::{r, rc},
    };

    #[test]
    fn journey_no_ws_commit_no_target() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, desc) =
            named_writable_scenario_with_description("single-branch-no-ws-commit-no-target")?;
        snapbox::assert_data_eq!(
            desc,
            snapbox::str![[r#"
Single commit, no main remote/target, no ws commit, but ws-reference

"#]]
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 3183e43 (HEAD -> gitbutler/workspace, main) M1

"#]]
        );

        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            Options {
                extra_target_commit_id: id_by_rev(&repo, "main").detach().into(),
                ..Options::limited()
            },
        )?;
        let ws = graph.into_workspace()?;

        // And even though setting an extra-target works like it should, i.e a simulated target
        // which we can store in absence of a selected target branch…
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43

"#]]
        );

        // …we chose to work with an open-ended workspace just to struggle more.
        let mut project_meta = project_meta(&repo)?;
        project_meta.target_commit_id = None;
        let graph = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
└── ≡:1:main
    └── :1:main
        └── ·3183e43 (🏘️)

"#]]
        );

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
        snapbox::assert_data_eq!(
            desc,
            snapbox::str![[r#"
Single commit, target, no ws commit, but ws-reference

"#]]
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 3183e43 (HEAD -> gitbutler/workspace, origin/main, main) M1

"#]]
        );

        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:3:A on 3183e43 {41}
    └── 📙:3:A

"#]]
        );
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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:3:A on 3183e43 {41}
│   └── 📙:3:A
└── ≡📙:4:B on 3183e43 {42}
    └── 📙:4:B

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:3:A on 3183e43 {41}
│   └── 📙:3:A
└── ≡📙:4:B on 3183e43 {42}
    └── 📙:4:B

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:3:above-A on 3183e43 {41}
│   ├── 📙:3:above-A
│   └── 📙:4:A
└── ≡📙:5:B on 3183e43 {42}
    └── 📙:5:B

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:3:above-A on 3183e43 {41}
│   ├── 📙:3:above-A
│   └── 📙:4:A
└── ≡📙:5:B on 3183e43 {42}
    ├── 📙:5:B
    └── 📙:6:below-B

"#]]
        );

        // Finally, assure the data looks correct. Can't afford bugs in the translation.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:3:above-A on 3183e43 {41}
│   ├── 📙:3:above-A
│   └── 📙:4:A
└── ≡📙:5:B on 3183e43 {42}
    ├── 📙:5:B
    └── 📙:6:below-B

"#]]
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 3183e43 (HEAD -> gitbutler/workspace, origin/main, main, below-B, above-A, B, A) M1

"#]]
        );

        Ok(())
    }

    #[test]
    fn journey_single_branch_segment_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-4-commits")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 43f9472 (A) A2
* 6fdab32 A1
* bce0c5e (origin/main, main) M2
* 3183e43 M1

"#]]
        );

        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡:3:A on bce0c5e
    └── :3:A
        ├── ·43f9472 (🏘️)
        └── ·6fdab32 (🏘️)

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡:4:A on bce0c5e {4cf}
    ├── :4:A
    │   └── ·43f9472 (🏘️)
    └── 📙:3:above-bottom
        └── ·6fdab32 (🏘️)

"#]]
        );

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

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡:4:A on bce0c5e {4cf}
    ├── :4:A
    │   └── ·43f9472 (🏘️)
    ├── 📙:3:above-bottom
    │   └── ·6fdab32 (🏘️)
    └── 📙:5:bottom

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:3:above-A-commit on bce0c5e {4cf}
    ├── 📙:3:above-A-commit
    │   └── ·43f9472 (🏘️) ►A
    ├── 📙:4:above-bottom
    │   └── ·6fdab32 (🏘️)
    └── 📙:5:bottom

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:5:above-A-commit on bce0c5e {4cf}
    ├── 📙:5:above-A-commit
    ├── 📙:6:A
    │   └── ·43f9472 (🏘️)
    ├── 📙:4:above-bottom
    │   └── ·6fdab32 (🏘️)
    └── 📙:7:bottom

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:5:above-A-commit on bce0c5e {4cf}
    ├── 📙:5:above-A-commit
    ├── 📙:6:above-A
    ├── 📙:7:A
    │   └── ·43f9472 (🏘️)
    ├── 📙:4:above-bottom
    │   └── ·6fdab32 (🏘️)
    └── 📙:8:bottom

"#]]
        );

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

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
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

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
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

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
├── ≡📙:6:above-A-commit on bce0c5e {4cf}
│   ├── 📙:6:above-A-commit
│   ├── 📙:7:above-A
│   ├── 📙:8:A
│   │   └── ·43f9472 (🏘️)
│   ├── 📙:9:below-A
│   ├── 📙:10:below-A-commit
│   ├── 📙:11:above-bottom
│   │   └── ·6fdab32 (🏘️)
│   └── 📙:12:bottom
└── ≡📙:5:B on bce0c5e {42}
    └── 📙:5:B

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
├── ≡📙:7:above-A-commit on bce0c5e {4cf}
│   ├── 📙:7:above-A-commit
│   ├── 📙:8:above-A
│   ├── 📙:9:A
│   │   └── ·43f9472 (🏘️)
│   ├── 📙:10:below-A
│   ├── 📙:11:below-A-commit
│   ├── 📙:12:above-bottom
│   │   └── ·6fdab32 (🏘️)
│   └── 📙:13:bottom
└── ≡📙:5:above-B on bce0c5e {42}
    ├── 📙:5:above-B
    └── 📙:6:B

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
├── ≡📙:8:above-A-commit on bce0c5e {4cf}
│   ├── 📙:8:above-A-commit
│   ├── 📙:9:above-A
│   ├── 📙:10:A
│   │   └── ·43f9472 (🏘️)
│   ├── 📙:11:below-A
│   ├── 📙:12:below-A-commit
│   ├── 📙:13:above-bottom
│   │   └── ·6fdab32 (🏘️)
│   └── 📙:14:bottom
└── ≡📙:5:above-B on bce0c5e {42}
    ├── 📙:5:above-B
    ├── 📙:6:B
    └── 📙:7:below-B

"#]]
        );

        // Finally, assure the data looks correct. Can't afford bugs in the translation.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
├── ≡📙:8:above-A-commit on bce0c5e {4cf}
│   ├── 📙:8:above-A-commit
│   ├── 📙:9:above-A
│   ├── 📙:10:A
│   │   └── ·43f9472 (🏘️)
│   ├── 📙:11:below-A
│   ├── 📙:12:below-A-commit
│   ├── 📙:13:above-bottom
│   │   └── ·6fdab32 (🏘️)
│   └── 📙:14:bottom
└── ≡📙:5:above-B on bce0c5e {42}
    ├── 📙:5:above-B
    ├── 📙:6:B
    └── 📙:7:below-B

"#]]
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 43f9472 (above-A-commit, above-A, A) A2
* 6fdab32 (below-A-commit, below-A, above-bottom) A1
* bce0c5e (origin/main, main, bottom, below-B, above-B, B) M2
* 3183e43 M1

"#]]
        );
        Ok(())
    }

    #[test]
    fn journey_single_branch_no_ws_commit_segment_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) =
            named_writable_scenario("single-branch-3-commits-no-ws-commit")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* c2878fb (HEAD -> gitbutler/workspace, A) A2
* 49d4b34 A1
* 3183e43 (origin/main, main) M1

"#]]
        );

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:3:A on 3183e43 {0}
    └── 📙:3:A
        ├── ·c2878fb (🏘️)
        └── ·49d4b34 (🏘️)

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:4:A on 3183e43 {0}
    ├── 📙:4:A
    │   └── ·c2878fb (🏘️)
    └── 📙:3:above-bottom
        └── ·49d4b34 (🏘️)

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:4:A on 3183e43 {0}
    ├── 📙:4:A
    │   └── ·c2878fb (🏘️)
    ├── 📙:3:above-bottom
    │   └── ·49d4b34 (🏘️)
    └── 📙:5:bottom

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:5:A on 3183e43 {0}
    ├── 📙:5:A
    ├── 📙:6:above-A-commit
    │   └── ·c2878fb (🏘️)
    ├── 📙:3:above-bottom
    │   └── ·49d4b34 (🏘️)
    └── 📙:7:bottom

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:5:above-A on 3183e43 {0}
    ├── 📙:5:above-A
    ├── 📙:6:A
    ├── 📙:7:above-A-commit
    │   └── ·c2878fb (🏘️)
    ├── 📙:3:above-bottom
    │   └── ·49d4b34 (🏘️)
    └── 📙:8:bottom

"#]]
        );

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

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:5:above-A on 3183e43 {0}
    ├── 📙:5:above-A
    ├── 📙:6:A
    ├── 📙:7:above-A-commit
    │   └── ·c2878fb (🏘️)
    ├── 📙:3:above-bottom
    │   └── ·49d4b34 (🏘️)
    └── 📙:8:bottom

"#]]
        );

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

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
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

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
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

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:6:above-A on 3183e43 {0}
│   ├── 📙:6:above-A
│   ├── 📙:7:A
│   ├── 📙:8:above-A-commit
│   │   └── ·c2878fb (🏘️)
│   ├── 📙:9:below-A
│   ├── 📙:10:below-A-commit
│   ├── 📙:11:above-bottom
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:12:bottom
└── ≡📙:5:B on 3183e43 {42}
    └── 📙:5:B

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:7:above-A on 3183e43 {0}
│   ├── 📙:7:above-A
│   ├── 📙:8:A
│   ├── 📙:9:above-A-commit
│   │   └── ·c2878fb (🏘️)
│   ├── 📙:10:below-A
│   ├── 📙:11:below-A-commit
│   ├── 📙:12:above-bottom
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:13:bottom
└── ≡📙:5:above-B on 3183e43 {42}
    ├── 📙:5:above-B
    └── 📙:6:B

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:8:above-A on 3183e43 {0}
│   ├── 📙:8:above-A
│   ├── 📙:9:A
│   ├── 📙:10:above-A-commit
│   │   └── ·c2878fb (🏘️)
│   ├── 📙:11:below-A
│   ├── 📙:12:below-A-commit
│   ├── 📙:13:above-bottom
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:14:bottom
└── ≡📙:5:above-B on 3183e43 {42}
    ├── 📙:5:above-B
    ├── 📙:6:B
    └── 📙:7:below-B

"#]]
        );

        // Finally, assure the data looks correct. Can't afford bugs in the translation.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:8:above-A on 3183e43 {0}
│   ├── 📙:8:above-A
│   ├── 📙:9:A
│   ├── 📙:10:above-A-commit
│   │   └── ·c2878fb (🏘️)
│   ├── 📙:11:below-A
│   ├── 📙:12:below-A-commit
│   ├── 📙:13:above-bottom
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:14:bottom
└── ≡📙:5:above-B on 3183e43 {42}
    ├── 📙:5:above-B
    ├── 📙:6:B
    └── 📙:7:below-B

"#]]
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* c2878fb (HEAD -> gitbutler/workspace, above-A-commit, above-A, A) A2
* 49d4b34 (below-A-commit, below-A, above-bottom) A1
* 3183e43 (origin/main, main, bottom, below-B, above-B, B) M1

"#]]
        );
        Ok(())
    }

    #[test]
    fn journey_single_branch_no_ws_commit_commit_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) =
            named_writable_scenario("single-branch-3-commits-no-ws-commit")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* c2878fb (HEAD -> gitbutler/workspace, A) A2
* 49d4b34 A1
* 3183e43 (origin/main, main) M1

"#]]
        );

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:3:A on 3183e43 {0}
    └── 📙:3:A
        ├── ·c2878fb (🏘️)
        └── ·49d4b34 (🏘️)

"#]]
        );

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

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:3:A on 3183e43 {0}
    ├── 📙:3:A
    │   ├── ·c2878fb (🏘️)
    │   └── ·49d4b34 (🏘️)
    └── 📙:4:bottom

"#]]
        );
        Ok(())
    }

    #[test]
    fn journey_multi_branch_commit_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("multi-branch-with-ws-commit")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
*   eaf2834 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 49d4b34 (A) A1
* | f57c528 (B) B1
|/  
* 3183e43 (origin/main, main) M1

"#]]
            .raw()
        );

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:3:A on 3183e43 {0}
│   └── 📙:3:A
│       └── ·49d4b34 (🏘️)
└── ≡📙:4:B on 3183e43 {1}
    └── 📙:4:B
        └── ·f57c528 (🏘️)

"#]]
        );

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
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:3:A on 3183e43 {0}
│   ├── 📙:3:A
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:5:a-bottom
└── ≡📙:4:B on 3183e43 {1}
    └── 📙:4:B
        └── ·f57c528 (🏘️)

"#]]
        );

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

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:3:A on 3183e43 {0}
│   ├── 📙:3:A
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:6:a-bottom
└── ≡📙:4:B on 3183e43 {1}
    ├── 📙:4:B
    │   └── ·f57c528 (🏘️)
    └── 📙:5:b-bottom

"#]]
        );
        Ok(())
    }

    #[test]
    fn journey_at_reference() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-4-commits")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 43f9472 (A) A2
* 6fdab32 A1
* bce0c5e (origin/main, main) M2
* 3183e43 M1

"#]]
        );

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:3:A on bce0c5e {0}
    └── 📙:3:A
        ├── ·43f9472 (🏘️)
        └── ·6fdab32 (🏘️)

"#]]
        );

        // Split 'A' so it owns only its top commit, with 'foo' owning the one below.
        let foo_ref = r("refs/heads/foo");
        let a1_id = id_by_rev(&repo, ":/A1");
        let ws = but_workspace::branch::create_reference(
            foo_ref,
            Anchor::AtCommit {
                commit_id: a1_id.detach(),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:3:A on bce0c5e {0}
    ├── 📙:3:A
    │   └── ·43f9472 (🏘️)
    └── 📙:4:foo
        └── ·6fdab32 (🏘️)

"#]]
        );

        // Below a *reference* means the new ref points at the same commit as 'A',
        // ordered right below it: 'A' becomes empty, 'new' takes over its commit.
        let new_ref = r("refs/heads/new");
        let a_ref = r("refs/heads/A");
        let ws = but_workspace::branch::create_reference(
            new_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:5:A on bce0c5e {0}
    ├── 📙:5:A
    ├── 📙:6:new
    │   └── ·43f9472 (🏘️)
    └── 📙:4:foo
        └── ·6fdab32 (🏘️)

"#]]
        );

        // Above is just like `AtSegment` above: an empty segment right on top of 'A'.
        let above_a_ref = r("refs/heads/above-A");
        let ws = but_workspace::branch::create_reference(
            above_a_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:5:above-A on bce0c5e {0}
    ├── 📙:5:above-A
    ├── 📙:6:A
    ├── 📙:7:new
    │   └── ·43f9472 (🏘️)
    └── 📙:4:foo
        └── ·6fdab32 (🏘️)

"#]]
        );

        // Anchoring below the now-empty 'A' sees through to the commit its ref points at.
        let below_empty_a_ref = r("refs/heads/below-empty-A");
        let ws = but_workspace::branch::create_reference(
            below_empty_a_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:5:above-A on bce0c5e {0}
    ├── 📙:5:above-A
    ├── 📙:6:A
    ├── 📙:7:below-empty-A
    ├── 📙:8:new
    │   └── ·43f9472 (🏘️)
    └── 📙:4:foo
        └── ·6fdab32 (🏘️)

"#]]
        );

        // Idempotency: recreating an existing reference at the same spot changes nothing.
        let ws = but_workspace::branch::create_reference(
            below_empty_a_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:5:above-A on bce0c5e {0}
    ├── 📙:5:above-A
    ├── 📙:6:A
    ├── 📙:7:below-empty-A
    ├── 📙:8:new
    │   └── ·43f9472 (🏘️)
    └── 📙:4:foo
        └── ·6fdab32 (🏘️)

"#]]
        );

        // Assure the persisted data reproduces the same workspace.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡📙:5:above-A on bce0c5e {0}
    ├── 📙:5:above-A
    ├── 📙:6:A
    ├── 📙:7:below-empty-A
    ├── 📙:8:new
    │   └── ·43f9472 (🏘️)
    └── 📙:4:foo
        └── ·6fdab32 (🏘️)

"#]]
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 43f9472 (new, below-empty-A, above-A, A) A2
* 6fdab32 (foo) A1
* bce0c5e (origin/main, main) M2
* 3183e43 M1

"#]]
        );
        Ok(())
    }

    #[test]
    fn at_reference_on_ws_base() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-no-ws-commit")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 3183e43 (HEAD -> gitbutler/workspace, origin/main, main) M1

"#]]
        );

        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;

        let a_ref = r("refs/heads/A");
        let ws = but_workspace::branch::create_reference(
            a_ref,
            None, /* anchor */
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:3:A on 3183e43 {41}
    └── 📙:3:A

"#]]
        );

        // Both positions work even though the anchor sits right on the workspace base.
        let below_a_ref = r("refs/heads/below-A");
        let ws = but_workspace::branch::create_reference(
            below_a_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:3:A on 3183e43 {41}
    ├── 📙:3:A
    └── 📙:4:below-A

"#]]
        );

        let above_a_ref = r("refs/heads/above-A");
        let ws = but_workspace::branch::create_reference(
            above_a_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Above,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:3:above-A on 3183e43 {41}
    ├── 📙:3:above-A
    ├── 📙:4:A
    └── 📙:5:below-A

"#]]
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 3183e43 (HEAD -> gitbutler/workspace, origin/main, main, below-A, above-A, A) M1

"#]]
        );
        Ok(())
    }

    #[test]
    fn at_reference_below_first_commit_in_history() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) =
            named_writable_scenario("single-branch-no-ws-commit-no-target")?;
        // Make the workspace open-ended so 'main' with the first commit in history is part of it.
        add_stack_with_segments(&mut meta, 0, "main", StackState::InWorkspace, &[]);

        let mut project_meta = project_meta(&repo)?;
        project_meta.target_commit_id = None;
        let graph = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:1:main {0}
    └── 📙:1:main
        └── ·3183e43 (🏘️)

"#]]
        );

        let new_ref = r("refs/heads/new");
        let main_ref = r("refs/heads/main");
        // There is no parent commit to point to below the first commit in history.
        let err = but_workspace::branch::create_reference(
            new_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(main_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )
        .unwrap_err();
        snapbox::assert_data_eq!(
            err.to_string(),
            snapbox::str![
                "Commit 3183e43ff482a2c4c8ff531d595453b64f58d90b is the first in history and no branch can point below it"
            ]
        );

        // A reference anchor doesn't need one - it shares the commit and only orders below.
        let ws = but_workspace::branch::create_reference(
            new_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(main_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:2:main {0}
    ├── 📙:2:main
    └── 📙:3:new
        └── ·3183e43 (🏘️)

"#]]
        );
        Ok(())
    }

    #[test]
    fn at_reference_multi_stack() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("multi-branch-with-ws-commit")?;
        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:3:A on 3183e43 {0}
│   └── 📙:3:A
│       └── ·49d4b34 (🏘️)
└── ≡📙:4:B on 3183e43 {1}
    └── 📙:4:B
        └── ·f57c528 (🏘️)

"#]]
        );

        // The new reference lands in the stack of its anchor.
        let new_ref = r("refs/heads/new");
        let b_ref = r("refs/heads/B");
        let ws = but_workspace::branch::create_reference(
            new_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(b_ref),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
├── ≡📙:3:A on 3183e43 {0}
│   └── 📙:3:A
│       └── ·49d4b34 (🏘️)
└── ≡📙:5:B on 3183e43 {1}
    ├── 📙:5:B
    └── 📙:6:new
        └── ·f57c528 (🏘️)

"#]]
        );
        Ok(())
    }

    #[test]
    fn at_reference_errors() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-4-commits")?;
        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;

        // The anchor must be a segment within the workspace.
        let new_ref = r("refs/heads/new");
        let err = but_workspace::branch::create_reference(
            new_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(r("refs/heads/bogus")),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )
        .unwrap_err();
        snapbox::assert_data_eq!(
            err.to_string(),
            snapbox::str!["Couldn't find any stack that contained the branch named 'bogus'"]
        );
        assert!(
            repo.try_find_reference(new_ref)?.is_none(),
            "the reference isn't physically available"
        );

        // The anchor must also be consolidated into workspace metadata.
        let err = but_workspace::branch::create_reference(
            new_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(r("refs/heads/A")),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )
        .unwrap_err();
        snapbox::assert_data_eq!(
            err.to_string(),
            snapbox::str!["Couldn't find anchor 'A' in workspace metadata - it's not consolidated"]
        );
        assert!(
            repo.try_find_reference(new_ref)?.is_none(),
            "the reference isn't physically available"
        );

        // A reference cannot be positioned relative to itself (managed workspace path).
        let err = but_workspace::branch::create_reference(
            r("refs/heads/A"),
            Anchor::AtReference {
                ref_name: Cow::Borrowed(r("refs/heads/A")),
                position: Below,
            },
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )
        .unwrap_err();
        snapbox::assert_data_eq!(
            err.to_string(),
            snapbox::str!["Cannot position 'A' relative to itself"]
        );
        Ok(())
    }

    #[test]
    fn error1() -> anyhow::Result<()> {
        let (repo, mut meta) = named_read_only_in_memory_scenario(
            "with-remotes-and-workspace",
            "single-branch-no-ws-commit",
        )?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* bce0c5e (HEAD -> gitbutler/workspace, main) M2
* 3183e43 (origin/main) M1

"#]]
        );

        let graph =
            but_graph::Graph::from_head(&repo, &*meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on bce0c5e

"#]]
        );

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
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* bba50eb (extra) E1
* c2878fb (HEAD -> gitbutler/workspace, A) A2
* 49d4b34 A1
* 3183e43 (origin/main, main) M1

"#]]
        );

        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

        let graph =
            but_graph::Graph::from_head(&repo, &*meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
└── ≡📙:3:A on 3183e43 {0}
    └── 📙:3:A
        ├── ·c2878fb (🏘️)
        └── ·49d4b34 (🏘️)

"#]]
        );

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

    /// Regression: `but branch new <name>` and `create_virtual_branch` both go through
    /// `create_reference(.., None, ..)`. When the target (`origin/main`) is advanced past the
    /// workspace, its tip sits OUTSIDE the workspace. The no-anchor path used to anchor the new
    /// ref at that tip and bail with "the target commit ... already belongs to another branch".
    /// It must instead anchor at `merge_base(target_tip, ws-commit)` (here M1), inside the
    /// workspace, so the branch emerges cleanly as its own stack.
    #[test]
    fn no_anchor_branch_with_target_tip_outside_workspace() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta) = named_writable_scenario("stack-below-advanced-target")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 1021d74 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 49d4b34 (A) A1
| * bce0c5e (origin/main, main) M2
|/  
* 3183e43 M1

"#]]
        );

        // `A` is applied (in the workspace), based at M1.
        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);

        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
└── ≡📙:3:A on 3183e43 {0}
    └── 📙:3:A
        └── ·49d4b34 (🏘️)

"#]]
        );

        // Precondition (see `⇣1` above): the target tip (M2) is one commit ahead of A's base,
        // so it sits OUTSIDE the workspace — the situation the no-anchor path mishandled.
        let target_id = ws
            .resolved_target_commit_id()
            .expect("the scenario sets a default target");
        assert!(
            ws.find_owner_indexes_by_commit_id(target_id).is_none(),
            "the target tip must be outside the workspace for this repro"
        );

        // Creating the no-anchor branch now succeeds (it used to bail).
        let new_name = rc("refs/heads/new-branch");
        let new_ref = new_name.as_ref();
        let updated_ws = but_workspace::branch::create_reference(
            new_ref,
            None,
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )?;

        // `new-branch` emerges as its own standalone stack/segment, based at M1.
        snapbox::assert_data_eq!(
            graph_workspace(&updated_ws).to_string(),
            snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main⇣1 on 3183e43
├── ≡📙:3:A on 3183e43 {0}
│   └── 📙:3:A
│       └── ·49d4b34 (🏘️)
└── ≡📙:5:new-branch on 3183e43 {3e5}
    └── 📙:5:new-branch

"#]]
        );

        // `new-branch` was written at M1 (== merge_base(target, ws-commit)), the commit just
        // below the advanced target — inside the workspace, not at the out-of-workspace tip.
        let new_tip = repo.find_reference(new_ref)?.peel_to_id()?.detach();
        assert_eq!(
            new_tip,
            repo.rev_parse_single("main~1")?.detach(),
            "the new branch must be anchored at M1, inside the workspace"
        );
        Ok(())
    }
}

#[test]
fn errors() -> anyhow::Result<()> {
    let (repo, mut meta) = named_read_only_in_memory_scenario("unborn-empty", "")?;
    let graph = but_graph::Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Options::limited(),
    )?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:main[🌳] <> ✓!
└── ≡:0:main[🌳] {1}
    └── :0:main[🌳]

"#]]
    );

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
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 89cc2d3 (A) change in A
* d79bba9 new file in A
* c166d42 (HEAD -> main) init-integration

"#]]
    );

    let graph =
        but_graph::Graph::from_head(&repo, &*meta, project_meta(&repo)?, Options::limited())?;
    let ws = graph.into_workspace()?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:main[🌳] <> ✓! on c166d42
└── ≡:0:main[🌳] {1}
    └── :0:main[🌳]

"#]]
    );

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
        let err = err.to_string();
        assert!(
            matches!(
                err.as_str(),
                "Cannot create reference on unborn branch"
                    | "Commit c166d42d4ef2e5e742d33554d03805cfb0b24d11 isn't part of the workspace"
            ),
            "workspace base cannot be used as a below-anchor: {err}"
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

    let graph = but_graph::Graph::from_commit_traversal(
        a_id,
        a_ref,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        Options::limited(),
    )?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:A <> ✓!
└── ≡:0:A {1}
    ├── :0:A
    │   ├── ·89cc2d3
    │   └── ·d79bba9
    └── :1:main[🌳]
        └── ·c166d42

"#]]
    );

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
        but_core::ref_metadata::ProjectMeta::default(),
        Options {
            extra_target_commit_id: main_id.detach().into(),
            commits_limit_hint: 0.into(),
            ..Options::limited()
        },
    )?;
    let ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:A <> ✓! on 89cc2d3
└── ≡:0:A {1}
    └── :0:A

"#]]
    );

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
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 281da94 (HEAD -> main) 3
* 12995d7 2
* 3d57fc1 1

"#]]
    );

    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Options::default(),
    )?;
    let ws = graph.into_workspace()?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:main[🌳] <> ✓!
└── ≡:0:main[🌳] {1}
    └── :0:main[🌳]
        ├── ·281da94
        ├── ·12995d7
        └── ·3d57fc1

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:main[🌳] <> ✓!
└── ≡:0:main[🌳] {1}
    ├── :0:main[🌳]
    │   └── ·281da94
    └── 📙:1:below-main
        ├── ·12995d7
        └── ·3d57fc1

"#]]
    );
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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:main[🌳] <> ✓!
└── ≡:0:main[🌳] {1}
    ├── :0:main[🌳]
    │   └── ·281da94
    └── 📙:1:below-main
        ├── ·12995d7
        └── ·3d57fc1

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:main[🌳] <> ✓!
└── ≡:0:main[🌳] {1}
    ├── :0:main[🌳]
    │   └── ·281da94
    ├── 📙:1:below-main
    │   └── ·12995d7
    └── 📙:2:two-below-main
        └── ·3d57fc1

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:main[🌳] <> ✓!
└── ≡:0:main[🌳] {1}
    ├── :0:main[🌳]
    │   └── ·281da94
    ├── 📙:1:below-main
    │   └── ·12995d7
    └── 📙:2:two-below-main
        └── ·3d57fc1

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:main[🌳] <> ✓!
└── ≡📙:0:main[🌳] {1}
    ├── 📙:0:main[🌳]
    │   └── ·281da94
    ├── 📙:1:below-main
    │   └── ·12995d7
    └── 📙:2:two-below-main
        └── ·3d57fc1

"#]]
    );

    Ok(())
}

#[test]
fn existing_git_ref_inside_workspace_is_adopted() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-4-commits")?;
    let graph =
        but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?;
    let ws = graph.into_workspace()?;

    let test_ref = r("refs/heads/created-with-git");
    let target_id = id_by_rev(&repo, ":/A1").detach();
    repo.reference(
        test_ref,
        target_id,
        PreviousValue::Any,
        "manual branch created with git",
    )?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 43f9472 (A) A2
* 6fdab32 (created-with-git) A1
* bce0c5e (origin/main, main) M2
* 3183e43 M1

"#]]
    );

    let ws = but_workspace::branch::create_reference(
        test_ref,
        None,
        &repo,
        &ws,
        &mut meta,
        stack_id_for_name,
        None,
    )?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
└── ≡:4:A on bce0c5e {632}
    ├── :4:A
    │   └── ·43f9472 (🏘️)
    └── 📙:3:created-with-git
        └── ·6fdab32 (🏘️)

"#]]
    );

    Ok(())
}

#[test]
fn journey_anon_workspace() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-with-3-commits")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 281da94 (HEAD -> main) 3
* 12995d7 2
* 3d57fc1 1

"#]]
    );

    let id = id_by_rev(&repo, "@~1");
    let graph = but_graph::Graph::from_commit_traversal(
        id,
        None,
        &meta,
        but_core::ref_metadata::ProjectMeta::default(),
        but_graph::init::Options::default(),
    )?;
    let ws = graph.into_workspace()?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:DETACHED <> ✓!
└── ≡:0:anon: {1}
    └── :0:anon:
        ├── ·12995d7
        └── ·3d57fc1

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:DETACHED <> ✓!
└── ≡:0:anon: {1}
    ├── :0:anon:
    │   └── ·12995d7
    └── 📙:1:first
        └── ·3d57fc1

"#]]
    );

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
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:second <> ✓!
└── ≡📙:0:second {1}
    ├── 📙:0:second
    │   └── ·12995d7
    └── 📙:1:first
        └── ·3d57fc1

"#]]
    );

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
        but_core::ref_metadata::ProjectMeta::default(),
        Options {
            extra_target_commit_id: Some(first_id.detach()),
            ..Default::default()
        },
    )?;
    let ws = graph.into_workspace()?;
    // And the extra-target serves as base also in single-branch mode.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:0:second <> ✓! on 3d57fc1
└── ≡📙:0:second on 3d57fc1 {1}
    └── 📙:0:second
        └── ·12995d7

"#]]
    );

    Ok(())
}

fn stack_id_for_name(rn: &gix::refs::FullNameRef) -> StackId {
    StackId::from_number_for_testing(rn.shorten().chars().map(|c| c as u128).sum())
}

/// [`Anchor::AtReference`] in an ad-hoc (single-branch) workspace, where the tip-to-base order of
/// same-commit branches lives in the `branch_order` table rather than workspace metadata.
mod ad_hoc_at_reference {
    use super::*;
    use but_workspace::branch::create_reference::Position;

    /// A single-branch workspace checked out on `main` (3 commits) with a *writable* branch-order
    /// backend, so `AtReference` placements can persist their order.
    fn ad_hoc_workspace() -> anyhow::Result<(
        tempfile::TempDir,
        gix::Repository,
        BranchOrderMetadata,
        but_core::ref_metadata::ProjectMeta,
        but_graph::Workspace,
    )> {
        let (tmp, repo, _legacy_meta) = named_writable_scenario("single-branch-with-3-commits")?;
        let project_meta = project_meta(&repo)?;
        let meta = branch_order_meta(&repo)?;
        let ws =
            but_graph::Graph::from_head(&repo, &meta, project_meta.clone(), Options::limited())?
                .into_workspace()?;
        Ok((tmp, repo, meta, project_meta, ws))
    }

    /// Create `new_ref` positioned relative to `anchor_ref` and return the resulting workspace.
    fn create(
        repo: &gix::Repository,
        ws: &but_graph::Workspace,
        meta: &mut BranchOrderMetadata,
        new_ref: &gix::refs::FullNameRef,
        anchor_ref: &gix::refs::FullNameRef,
        position: Position,
    ) -> anyhow::Result<but_graph::Workspace> {
        Ok(but_workspace::branch::create_reference(
            new_ref,
            Anchor::at_reference(anchor_ref, position),
            repo,
            ws,
            meta,
            stack_id_for_name,
            None,
        )?
        .into_owned())
    }

    /// Assert the durable tip-to-base order recorded for the chain containing `anchor`.
    fn assert_order(
        meta: &BranchOrderMetadata,
        anchor: &gix::refs::FullNameRef,
        expected: &[&str],
    ) {
        let expected: Vec<gix::refs::FullName> =
            expected.iter().copied().map(|s| r(s).to_owned()).collect();
        assert_eq!(
            meta.branch_stack_order(anchor).expect("order is readable"),
            Some(expected),
        );
    }

    #[test]
    fn orders_local_branches_only() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, _project_meta, ws) = ad_hoc_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:0:main[🌳] <> ✓! on 281da94
└── ≡:0:main[🌳] {1}
    └── :0:main[🌳]

"#]]
        );

        let main_ref = r("refs/heads/main");
        let main_id = repo.find_reference(main_ref)?.id();
        let below_ref = r("refs/heads/new-below");
        create(&repo, &ws, &mut meta, below_ref, main_ref, Below)?;
        assert_order(
            &meta,
            main_ref,
            &["refs/heads/main", "refs/heads/new-below"],
        );
        assert_eq!(repo.find_reference(below_ref)?.id(), main_id);

        // A remote anchor carries no ad-hoc ordering, so it is rejected.
        let remote_ref = r("refs/remotes/origin/main");
        repo.reference(
            remote_ref,
            main_id,
            PreviousValue::Any,
            "test remote anchor",
        )?;
        let err = create(
            &repo,
            &ws,
            &mut meta,
            r("refs/heads/new-remote-anchor"),
            remote_ref,
            Above,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Cannot position 'new-remote-anchor' relative to non-local reference 'origin/main' without a managed workspace"
        );
        Ok(())
    }

    #[test]
    fn requires_branch_order_metadata() -> anyhow::Result<()> {
        // A TOML-only backend can't persist order, so ad-hoc `AtReference` is refused up front.
        let (_tmp, repo, mut meta) = named_writable_scenario("single-branch-with-3-commits")?;
        let ws =
            but_graph::Graph::from_head(&repo, &meta, project_meta(&repo)?, Options::limited())?
                .into_workspace()?;

        let new_ref = r("refs/heads/new");
        let err = but_workspace::branch::create_reference(
            new_ref,
            Anchor::at_reference(r("refs/heads/main"), Above),
            &repo,
            &ws,
            &mut meta,
            stack_id_for_name,
            None,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Cannot position 'new' relative to local reference 'main' without branch order metadata"
        );
        assert!(
            repo.try_find_reference(new_ref)?.is_none(),
            "unsupported metadata must fail before creating the ref"
        );
        Ok(())
    }

    #[test]
    fn below_checked_out_branch_is_projected() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta, ws) = ad_hoc_workspace()?;

        let bottom_ref = r("refs/heads/empty-bottom");
        let main_ref = r("refs/heads/main");
        create(&repo, &ws, &mut meta, bottom_ref, main_ref, Below)?;

        let ws = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?
            .into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:1:main[🌳] <> ✓! on 281da94
└── ≡:1:main[🌳] {1}
    ├── :1:main[🌳]
    └── 📙:0:empty-bottom
        ├── ·281da94
        ├── ·12995d7
        └── ·3d57fc1

"#]]
        );
        assert!(repo.try_find_reference(bottom_ref)?.is_some());
        assert_order(
            &meta,
            main_ref,
            &["refs/heads/main", "refs/heads/empty-bottom"],
        );
        Ok(())
    }

    #[test]
    fn below_empty_branch_between_empty_branches() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta, ws) = ad_hoc_workspace()?;

        let middle_ref = r("refs/heads/empty-middle");
        let inserted_ref = r("refs/heads/inserted-below-middle");
        let bottom_ref = r("refs/heads/empty-bottom");
        let main_ref = r("refs/heads/main");

        let mut ws = ws;
        for (ref_name, anchor_ref) in [
            (bottom_ref, main_ref),
            (middle_ref, main_ref),
            (inserted_ref, middle_ref),
        ] {
            ws = create(&repo, &ws, &mut meta, ref_name, anchor_ref, Below)?;
        }

        assert_order(
            &meta,
            main_ref,
            &[
                "refs/heads/main",
                "refs/heads/empty-middle",
                "refs/heads/inserted-below-middle",
                "refs/heads/empty-bottom",
            ],
        );

        let ws = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?
            .into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:1:main[🌳] <> ✓! on 281da94
└── ≡:1:main[🌳] {1}
    ├── :1:main[🌳]
    ├── 📙:2:empty-middle
    ├── 📙:3:inserted-below-middle
    └── 📙:0:empty-bottom
        ├── ·281da94
        ├── ·12995d7
        └── ·3d57fc1

"#]]
        );
        Ok(())
    }

    #[test]
    fn above_checked_out_branch_is_projected_as_new_tip() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, _project_meta, ws) = ad_hoc_workspace()?;

        let top_ref = r("refs/heads/empty-top");
        let main_ref = r("refs/heads/main");
        let ws = create(&repo, &ws, &mut meta, top_ref, main_ref, Above)?;

        // Creating above the checked-out branch makes the new empty branch the projected tip.
        // `create_reference` does not move `HEAD`; the caller checks the new tip out (see its docs).
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:1:empty-top[🌳] <> ✓! on 281da94
└── ≡📙:1:empty-top[🌳] {1}
    ├── 📙:1:empty-top[🌳]
    └── :0:main
        ├── ·281da94
        ├── ·12995d7
        └── ·3d57fc1

"#]]
        );
        assert_eq!(
            ws.ref_name(),
            Some(top_ref),
            "the new tip is projected as the workspace entrypoint (as if checked out)"
        );
        assert!(repo.try_find_reference(top_ref)?.is_some());
        assert_order(
            &meta,
            main_ref,
            &["refs/heads/empty-top", "refs/heads/main"],
        );
        Ok(())
    }

    #[test]
    fn above_a_branch_over_the_entrypoint_is_rejected_without_a_checkout() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta, ws) = ad_hoc_workspace()?;
        let top_ref = r("refs/heads/empty-top");
        let main_ref = r("refs/heads/main");

        // Create the tip above `main`, but do NOT check it out - the real `HEAD` stays on `main`.
        create(&repo, &ws, &mut meta, top_ref, main_ref, Above)?;

        // Re-project from the real `HEAD` (`main`); `empty-top` now sits *above* the entrypoint and
        // is not part of the projection. Anchoring a further branch above `empty-top` would also
        // land above the entrypoint, so it can't be projected and is rejected. In practice the API
        // checks the tip out first, which is what makes stacking above it work.
        let ws = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?
            .into_workspace()?;
        assert_eq!(ws.ref_name(), Some(main_ref));
        let err = create(
            &repo,
            &ws,
            &mut meta,
            r("refs/heads/higher"),
            top_ref,
            Above,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("cannot be created"),
            "anchoring above a branch that sits above the entrypoint should be rejected: {err}"
        );
        Ok(())
    }

    #[test]
    fn order_survives_a_metadata_reload() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta, ws) = ad_hoc_workspace()?;
        let main_ref = r("refs/heads/main");
        let middle_ref = r("refs/heads/empty-middle");
        let bottom_ref = r("refs/heads/empty-bottom");

        let ws = create(&repo, &ws, &mut meta, bottom_ref, main_ref, Below)?;
        create(&repo, &ws, &mut meta, middle_ref, main_ref, Below)?;
        let expected = [
            "refs/heads/main",
            "refs/heads/empty-middle",
            "refs/heads/empty-bottom",
        ];
        assert_order(&meta, main_ref, &expected);

        // Reopen the branch-order backend from disk: the durable order must survive a fresh handle.
        drop(meta);
        let meta = branch_order_meta(&repo)?;
        assert_order(&meta, main_ref, &expected);

        // ...and the workspace re-projects identically from the reloaded metadata.
        let ws = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?
            .into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:1:main[🌳] <> ✓! on 281da94
└── ≡:1:main[🌳] {1}
    ├── :1:main[🌳]
    ├── 📙:2:empty-middle
    └── 📙:0:empty-bottom
        ├── ·281da94
        ├── ·12995d7
        └── ·3d57fc1

"#]]
        );
        Ok(())
    }

    #[test]
    fn rejects_a_missing_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, _project_meta, ws) = ad_hoc_workspace()?;
        let new_ref = r("refs/heads/new");
        let missing_anchor = r("refs/heads/does-not-exist");

        let err = create(&repo, &ws, &mut meta, new_ref, missing_anchor, Above).unwrap_err();
        assert!(
            err.to_string()
                .contains("the anchor reference does not exist"),
            "a non-existent anchor must be rejected with a clear precondition error: {err}"
        );
        assert!(
            repo.try_find_reference(new_ref)?.is_none(),
            "no ref should be created for a missing anchor"
        );
        assert_eq!(meta.branch_stack_order(missing_anchor)?, None);
        Ok(())
    }

    #[test]
    fn rejects_positioning_a_reference_relative_to_itself() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, _project_meta, ws) = ad_hoc_workspace()?;
        let main_ref = r("refs/heads/main");

        // Positioning a ref relative to itself must be a clean validation error, not a panic.
        let err = create(&repo, &ws, &mut meta, main_ref, main_ref, Above).unwrap_err();
        assert!(
            err.to_string().contains("relative to itself"),
            "self-referential placement must be rejected: {err}"
        );
        assert_eq!(meta.branch_stack_order(main_ref)?, None);
        Ok(())
    }

    #[test]
    fn reusing_an_existing_ref_for_a_different_commit_fails_without_mutation() -> anyhow::Result<()>
    {
        let (_tmp, repo, mut meta, _project_meta, ws) = ad_hoc_workspace()?;
        let main_ref = r("refs/heads/main");

        // `existing` already points at an older commit than `main`'s tip.
        let existing_ref = r("refs/heads/existing");
        let older = id_by_rev(&repo, "main~1").detach();
        repo.reference(existing_ref, older, PreviousValue::Any, "pre-existing ref")?;

        assert!(
            create(&repo, &ws, &mut meta, existing_ref, main_ref, Above).is_err(),
            "reusing an existing ref that points at a different commit must be rejected"
        );
        // The failure must be atomic: the ref is untouched and no order was persisted.
        assert_eq!(repo.find_reference(existing_ref)?.id().detach(), older);
        assert_eq!(meta.branch_stack_order(main_ref)?, None);
        Ok(())
    }

    #[test]
    fn rejects_a_name_colliding_with_an_existing_branch() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, _project_meta, ws) = ad_hoc_workspace()?;
        let main_ref = r("refs/heads/main");

        // `refs/heads/main` exists as a file, so `refs/heads/main/child` cannot be created.
        let colliding = r("refs/heads/main/child");
        let err = create(&repo, &ws, &mut meta, colliding, main_ref, Above).unwrap_err();
        assert!(
            err.to_string().contains("collides with existing branch"),
            "a name colliding with an existing branch should be reported clearly: {err}"
        );
        assert_eq!(meta.branch_stack_order(main_ref)?, None);
        Ok(())
    }

    #[test]
    fn interleaved_insertions_keep_a_consistent_order() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta, ws) = ad_hoc_workspace()?;
        let main_ref = r("refs/heads/main");
        let upper = r("refs/heads/upper");
        let middle = r("refs/heads/middle");
        let lower = r("refs/heads/lower");
        let bottom = r("refs/heads/bottom");
        let crown = r("refs/heads/crown");

        // Build a taller stack downward from the checked-out branch, inserting below different
        // anchors to exercise the insertion-index arithmetic beyond the two-branch cases.
        let mut ws = ws;
        for (new_ref, anchor_ref) in [
            (bottom, main_ref), // [main, bottom]
            (middle, main_ref), // [main, middle, bottom]
            (upper, main_ref),  // [main, upper, middle, bottom]
            (lower, middle),    // [main, upper, middle, lower, bottom]
        ] {
            ws = create(&repo, &ws, &mut meta, new_ref, anchor_ref, Below)?;
        }
        assert_order(
            &meta,
            main_ref,
            &[
                "refs/heads/main",
                "refs/heads/upper",
                "refs/heads/middle",
                "refs/heads/lower",
                "refs/heads/bottom",
            ],
        );

        // Mix in an `Above` insertion: a new tip over the (still checked-out) `main`.
        let ws = but_graph::Graph::from_head(&repo, &meta, project_meta, Options::limited())?
            .into_workspace()?;
        create(&repo, &ws, &mut meta, crown, main_ref, Above)?;
        assert_order(
            &meta,
            main_ref,
            &[
                "refs/heads/crown",
                "refs/heads/main",
                "refs/heads/upper",
                "refs/heads/middle",
                "refs/heads/lower",
                "refs/heads/bottom",
            ],
        );
        Ok(())
    }
}
