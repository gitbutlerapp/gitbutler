use but_core::{RefMetadata, ref_metadata::ValueInfo};
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
        ref_info::with_workspace_commit::utils::{
            StackState, add_stack_with_segments, named_read_only_in_memory_scenario,
            named_writable_scenario, named_writable_scenario_with_description, project_meta,
        },
        utils::{r, rc},
    };

    #[test]
    fn journey_no_ws_commit_no_target() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, desc, mut db) =
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
            but_core::ref_metadata::ProjectMeta {
                target_ref: None,
                ..project_meta(&repo)?
            },
            &mut db,
            Options::limited(),
        )?;
        let ws = graph.into_workspace()?;

        // And even though a stored target commit works like it should, i.e a simulated target
        // in absence of a selected target branch…
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓! on 3183e43

"#]]
        );

        // The workspace identity must come from workspace metadata, not target metadata.
        crate::ref_info::with_workspace_commit::utils::add_workspace(&mut meta);
        // …we chose to work with an open-ended workspace just to struggle more.
        let mut project_meta = but_core::ref_metadata::ProjectMeta {
            target_ref: None,
            ..project_meta(&repo)?
        };
        project_meta.target_commit_id = None;
        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta, &mut db, Options::limited())?;
        let mut ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡:main
    └── :main
        └── ·3183e43 (🏘️)

"#]]
        );

        let new_name = rc("refs/heads/A");
        let err = crate::utils::create_reference(
            /* anchor */
            &repo, &mut ws, &mut meta, &mut db, new_name, None, None,
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
        let (_tmp, repo, mut meta, desc, mut db) =
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

        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43

"#]]
        );

        let a_ref = r("refs/heads/A");
        let mut ws = crate::utils::create_reference(
            /* anchor */
            &repo, &mut ws, &mut meta, &mut db, a_ref, None, None,
        )
        .expect("it updates the workspace metadata legitimate the new ref at base");
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:A on 3183e43 {[..]}
    └── 📙:A

"#]]
        );
        let ws_base = ws.lower_bound.expect("target is set");
        assert_eq!(
            repo.find_reference(a_ref)?.id(),
            ws_base,
            "new stack refs are created on the workspace base"
        );

        let b_ref = r("refs/heads/B");
        let mut ws = crate::utils::create_reference(
            /* anchor */
            &repo, &mut ws, &mut meta, &mut db, b_ref, None, None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:A on 3183e43 {[..]}
│   └── 📙:A
└── ≡📙:B on 3183e43 {[..]}
    └── 📙:B

"#]]
        );

        let err =
            crate::utils::create_reference(&repo, &mut ws, &mut meta, &mut db, b_ref, None, None)
                .unwrap_err();
        assert_eq!(err.to_string(), "Reference 'B' already exists");

        let above_a = rc("refs/heads/above-A");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            above_a,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(a_ref),
                position: Above,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:above-A on 3183e43 {[..]}
│   ├── 📙:above-A
│   └── 📙:A
└── ≡📙:B on 3183e43 {[..]}
    └── 📙:B

"#]]
        );

        let below_b = rc("refs/heads/below-B");
        let ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            below_b,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Below,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:above-A on 3183e43 {[..]}
│   ├── 📙:above-A
│   └── 📙:A
└── ≡📙:B on 3183e43 {[..]}
    ├── 📙:B
    └── 📙:below-B

"#]]
        );

        // Finally, assure the data looks correct. Can't afford bugs in the translation.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:above-A on 3183e43 {[..]}
│   ├── 📙:above-A
│   └── 📙:A
└── ≡📙:B on 3183e43 {[..]}
    ├── 📙:B
    └── 📙:below-B

"#]]
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 2ed6b76 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/
* 3183e43 (origin/main, main, below-B, above-A, B, A) M1

"#]]
        );

        Ok(())
    }

    #[test]
    fn journey_single_branch_segment_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-branch-4-commits")?;
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

        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡:A on bce0c5e
    └── :A
        ├── ·43f9472 (🏘️)
        └── ·6fdab32 (🏘️)

"#]]
        );

        let above_bottom_ref = r("refs/heads/above-bottom");
        let bottom_id = id_by_rev(&repo, ":/A1");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            above_bottom_ref,
            Anchor::AtCommit {
                commit_id: bottom_id.detach(),
                position: Above,
            },
            None,
        )?;
        // It handles this special case, by creating the necessary workspace metadata
        // if for some reason (like manual building) it's not set.
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:A on bce0c5e {[..]}
    ├── 📙:A
    │   └── ·43f9472 (🏘️)
    └── 📙:above-bottom
        └── ·6fdab32 (🏘️)

"#]]
        );

        let bottom_ref = rc("refs/heads/bottom");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            bottom_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_bottom_ref),
                position: Below,
            },
            None,
        )?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:A on bce0c5e {[..]}
    ├── 📙:A
    │   └── ·43f9472 (🏘️)
    ├── 📙:above-bottom
    │   └── ·6fdab32 (🏘️)
    └── 📙:bottom

"#]]
        );

        let above_a_commit_ref = r("refs/heads/above-A-commit");
        let a_id = id_by_rev(&repo, ":/A");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            above_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Above,
            },
            None,
        )?;

        // Note how 'Above' *a commit* means directly above, not on top of everything.
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:A on bce0c5e {[..]}
    ├── 📙:A
    ├── 📙:above-A-commit
    │   └── ·43f9472 (🏘️)
    ├── 📙:above-bottom
    │   └── ·6fdab32 (🏘️)
    └── 📙:bottom

"#]]
        );

        let above_a_ref = rc("refs/heads/above-A");
        let a_ref = rc("refs/heads/A");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            above_a_ref,
            Anchor::AtSegment {
                ref_name: a_ref,
                position: Above,
            },
            None,
        )?;

        // *Above a segment means what one would expect though.
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:above-A on bce0c5e {[..]}
    ├── 📙:above-A
    ├── 📙:A
    ├── 📙:above-A-commit
    │   └── ·43f9472 (🏘️)
    ├── 📙:above-bottom
    │   └── ·6fdab32 (🏘️)
    └── 📙:bottom

"#]]
        );

        let below_a_commit_ref = rc("refs/heads/below-A-commit");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            below_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Below,
            },
            None,
        )?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:above-A on bce0c5e {[..]}
    ├── 📙:above-A
    ├── 📙:A
    ├── 📙:above-A-commit
    │   └── ·43f9472 (🏘️)
    ├── 📙:below-A-commit
    ├── 📙:above-bottom
    │   └── ·6fdab32 (🏘️)
    └── 📙:bottom

"#]]
        );

        let below_a_ref = rc("refs/heads/below-A");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            below_a_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_a_commit_ref),
                position: Below,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:above-A on bce0c5e {[..]}
    ├── 📙:above-A
    ├── 📙:A
    ├── 📙:above-A-commit
    │   └── ·43f9472 (🏘️)
    ├── 📙:below-A
    ├── 📙:below-A-commit
    ├── 📙:above-bottom
    │   └── ·6fdab32 (🏘️)
    └── 📙:bottom

"#]]
        );

        // create a new stack for good measure.
        let b_ref = r("refs/heads/B");
        let mut ws =
            crate::utils::create_reference(&repo, &mut ws, &mut meta, &mut db, b_ref, None, None)?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
├── ≡📙:above-A on bce0c5e {[..]}
│   ├── 📙:above-A
│   ├── 📙:A
│   ├── 📙:above-A-commit
│   │   └── ·43f9472 (🏘️)
│   ├── 📙:below-A
│   ├── 📙:below-A-commit
│   ├── 📙:above-bottom
│   │   └── ·6fdab32 (🏘️)
│   └── 📙:bottom
└── ≡📙:B on bce0c5e {[..]}
    └── 📙:B

"#]]
        );

        // create a new dependent branch by segment above (commit can't be done).
        let above_b_ref = rc("refs/heads/above-B");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            above_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Above,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
├── ≡📙:above-A on bce0c5e {[..]}
│   ├── 📙:above-A
│   ├── 📙:A
│   ├── 📙:above-A-commit
│   │   └── ·43f9472 (🏘️)
│   ├── 📙:below-A
│   ├── 📙:below-A-commit
│   ├── 📙:above-bottom
│   │   └── ·6fdab32 (🏘️)
│   └── 📙:bottom
└── ≡📙:above-B on bce0c5e {[..]}
    ├── 📙:above-B
    └── 📙:B

"#]]
        );

        // create a new dependent branch by segment below
        // (which somewhat counter-intuitively works here) because it's a completely new
        // independent branch.
        let below_b_ref = rc("refs/heads/below-B");
        let ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            below_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Below,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
├── ≡📙:above-A on bce0c5e {[..]}
│   ├── 📙:above-A
│   ├── 📙:A
│   ├── 📙:above-A-commit
│   │   └── ·43f9472 (🏘️)
│   ├── 📙:below-A
│   ├── 📙:below-A-commit
│   ├── 📙:above-bottom
│   │   └── ·6fdab32 (🏘️)
│   └── 📙:bottom
└── ≡📙:above-B on bce0c5e {[..]}
    ├── 📙:above-B
    ├── 📙:B
    └── 📙:below-B

"#]]
        );

        // Finally, assure the data looks correct. Can't afford bugs in the translation.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
├── ≡📙:above-A on bce0c5e {[..]}
│   ├── 📙:above-A
│   ├── 📙:A
│   ├── 📙:above-A-commit
│   │   └── ·43f9472 (🏘️)
│   ├── 📙:below-A
│   ├── 📙:below-A-commit
│   ├── 📙:above-bottom
│   │   └── ·6fdab32 (🏘️)
│   └── 📙:bottom
└── ≡📙:above-B on bce0c5e {[..]}
    ├── 📙:above-B
    ├── 📙:B
    └── 📙:below-B

"#]]
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
*   72bd978 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* | 43f9472 (above-A-commit, above-A, A) A2
* | 6fdab32 (below-A-commit, below-A, above-bottom) A1
|/  
* bce0c5e (origin/main, main, bottom, below-B, above-B, B) M2
* 3183e43 M1

"#]]
        );
        Ok(())
    }

    #[test]
    fn journey_single_branch_no_ws_commit_segment_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) =
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

        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:A on 3183e43 {0}
    └── 📙:A
        ├── ·c2878fb (🏘️)
        └── ·49d4b34 (🏘️)

"#]]
        );

        let above_bottom_ref = r("refs/heads/above-bottom");
        let bottom_id = id_by_rev(&repo, ":/A1");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            above_bottom_ref,
            Anchor::AtCommit {
                commit_id: bottom_id.detach(),
                position: Above,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:A on 3183e43 {0}
    ├── 📙:A
    │   └── ·c2878fb (🏘️)
    └── 📙:above-bottom
        └── ·49d4b34 (🏘️)

"#]]
        );

        let bottom_ref = rc("refs/heads/bottom");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            bottom_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_bottom_ref),
                position: Below,
            },
            None,
        )?;

        // We can create branches that would be on the base.
        // There are
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:A on 3183e43 {0}
    ├── 📙:A
    │   └── ·c2878fb (🏘️)
    ├── 📙:above-bottom
    │   └── ·49d4b34 (🏘️)
    └── 📙:bottom

"#]]
        );

        let above_a_commit_ref = r("refs/heads/above-A-commit");
        let a_id = id_by_rev(&repo, ":/A");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            above_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Above,
            },
            None,
        )?;

        // Note how 'Above' *a commit* means directly above, not on top of everything.
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:A on 3183e43 {0}
    ├── 📙:A
    ├── 📙:above-A-commit
    │   └── ·c2878fb (🏘️)
    ├── 📙:above-bottom
    │   └── ·49d4b34 (🏘️)
    └── 📙:bottom

"#]]
        );

        let above_a_ref = rc("refs/heads/above-A");
        let a_ref = rc("refs/heads/A");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            above_a_ref,
            Anchor::AtSegment {
                ref_name: a_ref,
                position: Above,
            },
            None,
        )?;

        // *Above a segment means what one would expect though.
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:above-A on 3183e43 {0}
    ├── 📙:above-A
    ├── 📙:A
    ├── 📙:above-A-commit
    │   └── ·c2878fb (🏘️)
    ├── 📙:above-bottom
    │   └── ·49d4b34 (🏘️)
    └── 📙:bottom

"#]]
        );

        let err = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            r("refs/heads/above-A"),
            Anchor::AtSegment {
                ref_name: rc("refs/heads/A"),
                position: Above,
            },
            None,
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Reference 'above-A' already exists");

        let below_a_commit_ref = rc("refs/heads/below-A-commit");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            below_a_commit_ref,
            Anchor::AtCommit {
                commit_id: a_id.detach(),
                position: Below,
            },
            None,
        )?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:above-A on 3183e43 {0}
    ├── 📙:above-A
    ├── 📙:A
    ├── 📙:above-A-commit
    │   └── ·c2878fb (🏘️)
    ├── 📙:below-A-commit
    ├── 📙:above-bottom
    │   └── ·49d4b34 (🏘️)
    └── 📙:bottom

"#]]
        );

        let below_a_ref = rc("refs/heads/below-A");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            below_a_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(above_a_commit_ref),
                position: Below,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:above-A on 3183e43 {0}
    ├── 📙:above-A
    ├── 📙:A
    ├── 📙:above-A-commit
    │   └── ·c2878fb (🏘️)
    ├── 📙:below-A
    ├── 📙:below-A-commit
    ├── 📙:above-bottom
    │   └── ·49d4b34 (🏘️)
    └── 📙:bottom

"#]]
        );

        // create a new stack for good measure.
        let b_ref = r("refs/heads/B");
        let mut ws =
            crate::utils::create_reference(&repo, &mut ws, &mut meta, &mut db, b_ref, None, None)?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:above-A on 3183e43 {0}
│   ├── 📙:above-A
│   ├── 📙:A
│   ├── 📙:above-A-commit
│   │   └── ·c2878fb (🏘️)
│   ├── 📙:below-A
│   ├── 📙:below-A-commit
│   ├── 📙:above-bottom
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:bottom
└── ≡📙:B on 3183e43 {[..]}
    └── 📙:B

"#]]
        );

        // create a new dependent branch by segment above (commit can't be done).
        let above_b_ref = rc("refs/heads/above-B");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            above_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Above,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:above-A on 3183e43 {0}
│   ├── 📙:above-A
│   ├── 📙:A
│   ├── 📙:above-A-commit
│   │   └── ·c2878fb (🏘️)
│   ├── 📙:below-A
│   ├── 📙:below-A-commit
│   ├── 📙:above-bottom
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:bottom
└── ≡📙:above-B on 3183e43 {[..]}
    ├── 📙:above-B
    └── 📙:B

"#]]
        );

        // create a new dependent branch by segment below
        // (which somewhat counter-intuitively works here) because it's a completely new
        // independent branch.
        let below_b_ref = rc("refs/heads/below-B");
        let ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            below_b_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(b_ref),
                position: Below,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:above-A on 3183e43 {0}
│   ├── 📙:above-A
│   ├── 📙:A
│   ├── 📙:above-A-commit
│   │   └── ·c2878fb (🏘️)
│   ├── 📙:below-A
│   ├── 📙:below-A-commit
│   ├── 📙:above-bottom
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:bottom
└── ≡📙:above-B on 3183e43 {[..]}
    ├── 📙:above-B
    ├── 📙:B
    └── 📙:below-B

"#]]
        );

        // Finally, assure the data looks correct. Can't afford bugs in the translation.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:above-A on 3183e43 {0}
│   ├── 📙:above-A
│   ├── 📙:A
│   ├── 📙:above-A-commit
│   │   └── ·c2878fb (🏘️)
│   ├── 📙:below-A
│   ├── 📙:below-A-commit
│   ├── 📙:above-bottom
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:bottom
└── ≡📙:above-B on 3183e43 {[..]}
    ├── 📙:above-B
    ├── 📙:B
    └── 📙:below-B

"#]]
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
*   36048bc (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|/  
* | c2878fb (above-A-commit, above-A, A) A2
* | 49d4b34 (below-A-commit, below-A, above-bottom) A1
|/  
* 3183e43 (origin/main, main, bottom, below-B, above-B, B) M1

"#]]
        );
        Ok(())
    }

    #[test]
    fn journey_single_branch_no_ws_commit_commit_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) =
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

        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:A on 3183e43 {0}
    └── 📙:A
        ├── ·c2878fb (🏘️)
        └── ·49d4b34 (🏘️)

"#]]
        );

        let bottom_ref = rc("refs/heads/bottom");
        let bottom_id = id_by_rev(&repo, ":/A1");
        let ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            bottom_ref,
            Anchor::AtCommit {
                commit_id: bottom_id.detach(),
                position: Below,
            },
            None,
        )?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:A on 3183e43 {0}
    ├── 📙:A
    │   ├── ·c2878fb (🏘️)
    │   └── ·49d4b34 (🏘️)
    └── 📙:bottom

"#]]
        );
        Ok(())
    }

    #[test]
    fn journey_multi_branch_commit_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) =
            named_writable_scenario("multi-branch-with-ws-commit")?;
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

        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:A on 3183e43 {0}
│   └── 📙:A
│       └── ·49d4b34 (🏘️)
└── ≡📙:B on 3183e43 {1}
    └── 📙:B
        └── ·f57c528 (🏘️)

"#]]
        );

        let bottom_ref_a = rc("refs/heads/a-bottom");
        let bottom_a_id = id_by_rev(&repo, ":/A1");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            bottom_ref_a,
            Anchor::AtCommit {
                commit_id: bottom_a_id.detach(),
                position: Below,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:A on 3183e43 {0}
│   ├── 📙:A
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:a-bottom
└── ≡📙:B on 3183e43 {1}
    └── 📙:B
        └── ·f57c528 (🏘️)

"#]]
        );

        let bottom_ref_b = rc("refs/heads/b-bottom");
        let bottom_b_id = id_by_rev(&repo, ":/B1");
        let ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            bottom_ref_b,
            Anchor::AtCommit {
                commit_id: bottom_b_id.detach(),
                position: Below,
            },
            None,
        )?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:A on 3183e43 {0}
│   ├── 📙:A
│   │   └── ·49d4b34 (🏘️)
│   └── 📙:a-bottom
└── ≡📙:B on 3183e43 {1}
    ├── 📙:B
    │   └── ·f57c528 (🏘️)
    └── 📙:b-bottom

"#]]
        );
        Ok(())
    }

    #[test]
    fn journey_at_reference() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-branch-4-commits")?;
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

        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:A on bce0c5e {0}
    └── 📙:A
        ├── ·43f9472 (🏘️)
        └── ·6fdab32 (🏘️)

"#]]
        );

        // Split 'A' so it owns only its top commit, with 'foo' owning the one below.
        let foo_ref = r("refs/heads/foo");
        let a1_id = id_by_rev(&repo, ":/A1");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            foo_ref,
            Anchor::AtCommit {
                commit_id: a1_id.detach(),
                position: Above,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:A on bce0c5e {0}
    ├── 📙:A
    │   └── ·43f9472 (🏘️)
    └── 📙:foo
        └── ·6fdab32 (🏘️)

"#]]
        );

        // Below a *reference* means the new ref points at the same commit as 'A',
        // ordered right below it: 'A' becomes empty, 'new' takes over its commit.
        let new_ref = r("refs/heads/new");
        let a_ref = r("refs/heads/A");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            new_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Below,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:A on bce0c5e {0}
    ├── 📙:A
    ├── 📙:new
    │   └── ·43f9472 (🏘️)
    └── 📙:foo
        └── ·6fdab32 (🏘️)

"#]]
        );

        // Above is just like `AtSegment` above: an empty segment right on top of 'A'.
        let above_a_ref = r("refs/heads/above-A");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            above_a_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Above,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:above-A on bce0c5e {0}
    ├── 📙:above-A
    ├── 📙:A
    ├── 📙:new
    │   └── ·43f9472 (🏘️)
    └── 📙:foo
        └── ·6fdab32 (🏘️)

"#]]
        );

        // Anchoring below the now-empty 'A' sees through to the commit its ref points at.
        let below_empty_a_ref = r("refs/heads/below-empty-A");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            below_empty_a_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Below,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:above-A on bce0c5e {0}
    ├── 📙:above-A
    ├── 📙:A
    ├── 📙:below-empty-A
    ├── 📙:new
    │   └── ·43f9472 (🏘️)
    └── 📙:foo
        └── ·6fdab32 (🏘️)

"#]]
        );

        let err = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            below_empty_a_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Below,
            },
            None,
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Reference 'below-empty-A' already exists");

        // Assure the persisted data reproduces the same workspace.
        let path = meta.path().to_owned();
        drop(meta);
        let meta = VirtualBranchesTomlMetadata::from_path(path)?;
        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e
└── ≡📙:above-A on bce0c5e {0}
    ├── 📙:above-A
    ├── 📙:A
    ├── 📙:below-empty-A
    ├── 📙:new
    │   └── ·43f9472 (🏘️)
    └── 📙:foo
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
    fn at_commit_on_ws_base() -> anyhow::Result<()> {
        for position in [Above, Below] {
            let (_tmp, repo, mut meta, mut db) =
                named_writable_scenario("single-branch-no-ws-commit")?;
            let mut ws = but_graph::Graph::from_head(
                &repo,
                &meta,
                project_meta(&repo)?,
                &mut db,
                Options::limited(),
            )?
            .into_workspace()?;
            let base = ws.lower_bound.expect("fixture has a workspace base");
            let new_ref = r("refs/heads/at-base");
            crate::utils::create_reference(
                &repo,
                &mut ws,
                &mut meta,
                &mut db,
                new_ref,
                Anchor::AtCommit {
                    commit_id: base,
                    position,
                },
                None,
            )?;
            assert_eq!(
                repo.find_reference(new_ref)?.peel_to_id()?.detach(),
                base,
                "both positions at the workspace boundary must point to the base"
            );
        }
        Ok(())
    }

    #[test]
    fn at_reference_on_ws_base() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-branch-no-ws-commit")?;
        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* 3183e43 (HEAD -> gitbutler/workspace, origin/main, main) M1

"#]]
        );

        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;

        let a_ref = r("refs/heads/A");
        let mut ws = crate::utils::create_reference(
            /* anchor */
            &repo, &mut ws, &mut meta, &mut db, a_ref, None, None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:A on 3183e43 {[..]}
    └── 📙:A

"#]]
        );

        // Both positions work even though the anchor sits right on the workspace base.
        let below_a_ref = r("refs/heads/below-A");
        let mut ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            below_a_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Below,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:A on 3183e43 {[..]}
    ├── 📙:A
    └── 📙:below-A

"#]]
        );

        let above_a_ref = r("refs/heads/above-A");
        let ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            above_a_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(a_ref),
                position: Above,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:above-A on 3183e43 {[..]}
    ├── 📙:above-A
    ├── 📙:A
    └── 📙:below-A

"#]]
        );

        snapbox::assert_data_eq!(
            visualize_commit_graph_all(&repo)?,
            snapbox::str![[r#"
* a6258f6 (HEAD -> gitbutler/workspace) GitButler Workspace Commit
* 3183e43 (origin/main, main, below-A, above-A, A) M1

"#]]
        );
        Ok(())
    }

    #[test]
    fn at_reference_below_first_commit_in_history() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) =
            named_writable_scenario("single-branch-no-ws-commit-no-target")?;
        // Make the workspace open-ended so 'main' with the first commit in history is part of it.
        add_stack_with_segments(&mut meta, 0, "main", StackState::InWorkspace, &[]);

        let mut project_meta = but_core::ref_metadata::ProjectMeta {
            target_ref: None,
            ..project_meta(&repo)?
        };
        project_meta.target_commit_id = None;
        let graph =
            but_graph::Graph::from_head(&repo, &meta, project_meta, &mut db, Options::limited())?;
        let mut ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:main {0}
    └── 📙:main
        └── ·3183e43 (🏘️)

"#]]
        );

        let new_ref = r("refs/heads/new");
        let main_ref = r("refs/heads/main");
        // There is no parent commit to point to below the first commit in history.
        let err = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            new_ref,
            Anchor::AtSegment {
                ref_name: Cow::Borrowed(main_ref),
                position: Below,
            },
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
        let ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            new_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(main_ref),
                position: Below,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓!
└── ≡📙:main {0}
    ├── 📙:main
    └── 📙:new
        └── ·3183e43 (🏘️)

"#]]
        );
        Ok(())
    }

    #[test]
    fn at_reference_multi_stack() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) =
            named_writable_scenario("multi-branch-with-ws-commit")?;
        add_stack_with_segments(&mut meta, 0, "A", StackState::InWorkspace, &[]);
        add_stack_with_segments(&mut meta, 1, "B", StackState::InWorkspace, &[]);

        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:A on 3183e43 {0}
│   └── 📙:A
│       └── ·49d4b34 (🏘️)
└── ≡📙:B on 3183e43 {1}
    └── 📙:B
        └── ·f57c528 (🏘️)

"#]]
        );

        // The new reference lands in the stack of its anchor.
        let new_ref = r("refs/heads/new");
        let b_ref = r("refs/heads/B");
        let ws = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            new_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(b_ref),
                position: Below,
            },
            None,
        )?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
├── ≡📙:A on 3183e43 {0}
│   └── 📙:A
│       └── ·49d4b34 (🏘️)
└── ≡📙:B on 3183e43 {1}
    ├── 📙:B
    └── 📙:new
        └── ·f57c528 (🏘️)

"#]]
        );
        Ok(())
    }

    #[test]
    fn at_reference_errors() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-branch-4-commits")?;
        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;

        // The anchor must be a segment within the workspace.
        let new_ref = r("refs/heads/new");
        let err = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            new_ref,
            Anchor::AtReference {
                ref_name: Cow::Borrowed(r("refs/heads/bogus")),
                position: Below,
            },
            None,
        )
        .unwrap_err();
        snapbox::assert_data_eq!(
            err.to_string(),
            snapbox::str!["The anchor reference 'bogus' isn't part of the workspace"]
        );
        assert!(
            repo.try_find_reference(new_ref)?.is_none(),
            "the reference isn't physically available"
        );

        // A reference cannot be positioned relative to itself (managed workspace path).
        let err = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            r("refs/heads/A"),
            Anchor::AtReference {
                ref_name: Cow::Borrowed(r("refs/heads/A")),
                position: Below,
            },
            None,
        )
        .unwrap_err();
        snapbox::assert_data_eq!(
            err.to_string(),
            snapbox::str!["Reference 'A' already exists"]
        );
        Ok(())
    }

    #[test]
    fn error1() -> anyhow::Result<()> {
        let (repo, mut meta, mut db) = named_read_only_in_memory_scenario(
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

        let graph = but_graph::Graph::from_head(
            &repo,
            &*meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;

        // The header describes local main; incoming history still comes from its upstream.
        assert_eq!(
            ws.graph
                .project_meta
                .remote_target_ref(&repo)?
                .map(|name| name.to_string()),
            Some("refs/remotes/origin/main".to_owned()),
            "the fixture must configure the target upstream"
        );
        assert_eq!(
            ws.incoming_target_commit_ids(&repo)?,
            Vec::from([repo.rev_parse_single("origin/main")?.detach()]),
            "incoming upstream commits must remain visible independently of the local target count"
        );
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on bce0c5e

"#]]
        );

        let (ws_id, ws_ref_name) = id_at(&repo, "gitbutler/workspace");
        let main_remote_id = id_by_rev(&repo, "@~1");
        for anchor in [
            (Anchor::at_id(main_remote_id, Above)),
            (Anchor::at_segment(r("refs/remotes/origin/main"), Above)),
        ] {
            let err = crate::utils::create_reference(
                &repo,
                &mut ws,
                &mut *meta,
                &mut db,
                ws_ref_name.as_ref(),
                anchor.clone(),
                None,
            )
            .unwrap_err();

            assert_eq!(
                err.to_string(),
                "Reference 'gitbutler/workspace' already exists",
                "cannot overwrite the workspace ref"
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
        let (repo, mut meta, mut db) = named_read_only_in_memory_scenario(
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

        let graph = but_graph::Graph::from_head(
            &repo,
            &*meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;

        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️⚠️:gitbutler/workspace[🌳] <> ✓refs/heads/main on 3183e43
└── ≡📙:A on 3183e43 {0}
    └── 📙:A
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
            let err = crate::utils::create_reference(
                &repo,
                &mut ws,
                &mut *meta,
                &mut db,
                ws_ref_name.as_ref(),
                anchor.clone(),
                None,
            )
            .unwrap_err();

            assert_eq!(
                err.to_string(),
                "Reference 'gitbutler/workspace' already exists",
                "cannot overwrite the workspace ref"
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
            let err = crate::utils::create_reference(
                &repo,
                &mut ws,
                &mut *meta,
                &mut db,
                ws_ref_name.as_ref(),
                anchor.clone(),
                None,
            )
            .unwrap_err();

            assert_eq!(
                err.to_string(),
                "Reference 'gitbutler/workspace' already exists",
                "cannot overwrite the workspace ref"
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
        let err = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut *meta,
            &mut db,
            outside_ref.as_ref(),
            None,
            None,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "Reference 'extra' already exists",
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
        let err = crate::utils::create_reference(
            &repo,
            &mut ws,
            &mut *meta,
            &mut db,
            new_name,
            Anchor::AtSegment {
                ref_name: rc("refs/heads/bogus"),
                position: Below,
            },
            None,
        )
        .unwrap_err();

        assert_eq!(
            err.to_string(),
            "The anchor reference 'bogus' isn't part of the workspace",
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
        let (_tmp, repo, mut meta, mut db) =
            named_writable_scenario("stack-below-advanced-target")?;
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

        let graph = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta(&repo)?,
            &mut db,
            Options::limited(),
        )?;
        let mut ws = graph.into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main⇣1 on 3183e43
└── ≡📙:A on 3183e43 {0}
    └── 📙:A
        └── ·49d4b34 (🏘️)

"#]]
        );

        // Precondition (see `⇣1` above): the target tip (M2) is one commit ahead of A's base,
        // so it sits OUTSIDE the workspace — the situation the no-anchor path mishandled.
        let target_id = ws
            .target_commit
            .as_ref()
            .map(|target| target.commit_id)
            .expect("the scenario sets a default target");
        assert!(
            ws.find_owner_indexes_by_commit_id(target_id).is_none(),
            "the target tip must be outside the workspace for this repro"
        );

        // Creating the no-anchor branch now succeeds (it used to bail).
        let new_name = rc("refs/heads/new-branch");
        let new_ref = new_name.as_ref();
        let updated_ws = crate::utils::create_reference(
            &repo, &mut ws, &mut meta, &mut db, new_ref, None, None,
        )?;

        // `new-branch` emerges as its own standalone stack/segment, based at M1.
        snapbox::assert_data_eq!(
            graph_workspace(&updated_ws).to_string(),
            snapbox::str![[r#"
📕🏘️:gitbutler/workspace[🌳] <> ✓refs/heads/main⇣1 on 3183e43
├── ≡📙:A on 3183e43 {0}
│   └── 📙:A
│       └── ·49d4b34 (🏘️)
└── ≡📙:new-branch on 3183e43 {[..]}
    └── 📙:new-branch

"#]]
        );

        // `new-branch` was written at M1 (== merge_base(target, ws-commit)), the commit just
        // below the advanced target — inside the workspace, not at the out-of-workspace tip.
        let new_tip = repo.find_reference(new_ref)?.peel_to_id()?.detach();
        assert_eq!(
            new_tip,
            repo.rev_parse_single("main~1")?,
            "the new branch must be anchored at M1, inside the workspace"
        );
        Ok(())
    }
}

#[test]
fn errors() -> anyhow::Result<()> {
    let (repo, mut meta, mut db) = named_read_only_in_memory_scenario("unborn-empty", "")?;
    let graph = but_graph::Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta::default(),
        &mut db,
        Options::limited(),
    )?;
    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]

"#]]
    );

    // Below first in history
    let new_name = r("refs/heads/does-not-matter");
    let err = crate::utils::create_reference(
        &repo,
        &mut ws,
        &mut *meta,
        &mut db,
        new_name,
        Anchor::AtSegment {
            ref_name: Cow::Borrowed(r("refs/heads/main")),
            position: Above,
        },
        None,
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "Cannot create reference on unborn branch");

    let (repo, mut meta, mut db) =
        named_read_only_in_memory_scenario("with-remotes-no-workspace", "remote")?;
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 89cc2d3 (A) change in A
* d79bba9 new file in A
* c166d42 (HEAD -> main) init-integration

"#]]
    );

    let graph = but_graph::Graph::from_head(
        &repo,
        &*meta,
        but_core::ref_metadata::ProjectMeta {
            target_ref: None,
            ..project_meta(&repo)?
        },
        &mut db,
        Options::limited(),
    )?;
    let mut ws = graph.into_workspace()?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓! on c166d42
└── ≡:main[🌳] on c166d42 {1}
    └── :main[🌳]

"#]]
    );

    let (id, ref_name) = id_at(&repo, "main");
    let err = crate::utils::create_reference(
        &repo,
        &mut ws,
        &mut *meta,
        &mut db,
        new_name,
        Anchor::at_id(id, Below),
        None,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Cannot create independent branch 'does-not-matter' outside of a workspace",
        "a base anchor must not create a branch absent from the resulting projection"
    );
    assert!(
        repo.try_find_reference(new_name)?.is_none(),
        "the reference isn't physically available"
    );
    assert!(
        meta.branch(ref_name.as_ref())?.is_default(),
        "no data was stored"
    );

    // Misaligned workspace - commit not included.
    let (id, ref_name) = id_at(&repo, "A");
    for anchor in [Anchor::at_id(id, Below), Anchor::at_id(id, Above)] {
        let err = crate::utils::create_reference(
            &repo, &mut ws, &mut *meta, &mut db, new_name, anchor, None,
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
        let err = crate::utils::create_reference(
            &repo, &mut ws, &mut *meta, &mut db, new_name, anchor, None,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "The anchor reference 'A' isn't part of the workspace",
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
        &mut db,
        Options::limited(),
    )?;
    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:A <> ✓!
└── ≡:A {1}
    ├── :A
    │   ├── ·89cc2d3
    │   └── ·d79bba9
    └── :main[🌳]
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
        let err = crate::utils::create_reference(
            &repo, &mut ws, &mut *meta, &mut db, a_ref, anchor, None,
        )
        .unwrap_err();
        assert_eq!(err.to_string(), "Reference 'A' already exists");
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
        but_core::ref_metadata::ProjectMeta {
            target_commit_id: Some(main_id.detach()),
            ..Default::default()
        },
        &mut db,
        Options {
            commits_limit_hint: 0.into(),
            ..Options::limited()
        },
    )?;
    let mut ws = graph.into_workspace()?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:A <> ✓!
└── ≡:A {1}
    └── :A
        └── ✂️·89cc2d3

"#]]
    );

    // The single commit of `A` is visible, but nothing below it is.
    let err = crate::utils::create_reference(
        &repo,
        &mut ws,
        &mut *meta,
        &mut db,
        new_name,
        Anchor::at_segment(a_ref, Below),
        None,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Commit d79bba960b112dbd25d45921c47eeda22288022b isn't part of the workspace",
    );
    assert!(meta.branch(a_ref)?.is_default(), "no data was stored");
    assert_ne!(
        repo.find_reference(a_ref)?.id(),
        main_id,
        "it shouldn't actually have changed the ref"
    );
    Ok(())
}

#[test]
fn journey_with_commits() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-branch-with-3-commits")?;
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
        &mut db,
        but_graph::init::Options::default(),
    )?;
    let mut ws = graph.into_workspace()?;

    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    └── :main[🌳]
        ├── ·281da94
        ├── ·12995d7
        └── ·3d57fc1

"#]]
    );

    let (main_id, main_ref) = id_at(&repo, "main");
    let new_name = r("refs/heads/below-main");
    let err = crate::utils::create_reference(
        &repo,
        &mut ws,
        &mut meta,
        &mut db,
        new_name,
        Anchor::at_segment(main_ref.as_ref(), Below),
        None,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Commit 3d57fc18d679a1ba45bc7f79e394a5e2606719ee is the first in history and no branch can point below it"
    );

    let mut ws = crate::utils::create_reference(
        &repo,
        &mut ws,
        &mut meta,
        &mut db,
        new_name,
        Anchor::at_id(main_id, Below),
        None,
    )
    .expect("this works as the branch is unique");

    // We always add metadata to new branches.
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    ├── :main[🌳]
    │   └── ·281da94
    └── :below-main
        ├── ·12995d7
        └── ·3d57fc1

"#]]
    );
    assert!(
        repo.find_reference(new_name).is_ok(),
        "It should just have been created"
    );

    let err = crate::utils::create_reference(
        &repo,
        &mut ws,
        &mut meta,
        &mut db,
        new_name,
        Anchor::at_id(main_id, Below),
        None,
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "Reference 'below-main' already exists");

    // the last possible branch without a workspace.
    let ws = crate::utils::create_reference(
        &repo,
        &mut ws,
        &mut meta,
        &mut db,
        rc("refs/heads/two-below-main"),
        Anchor::at_id(id_by_rev(&repo, ":/2"), Below),
        None,
    )?;
    snapbox::assert_data_eq!(
        graph_workspace(&ws).to_string(),
        snapbox::str![[r#"
⌂:main[🌳] <> ✓!
└── ≡:main[🌳] {1}
    ├── :main[🌳]
    │   └── ·281da94
    ├── :below-main
    │   └── ·12995d7
    └── :two-below-main
        └── ·3d57fc1

"#]]
    );

    Ok(())
}

#[test]
fn existing_git_ref_is_rejected() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-branch-4-commits")?;
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&repo)?,
        &mut db,
        Options::limited(),
    )?;
    let mut ws = graph.into_workspace()?;

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

    let err =
        crate::utils::create_reference(&repo, &mut ws, &mut meta, &mut db, test_ref, None, None)
            .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Reference 'created-with-git' already exists"
    );

    Ok(())
}

#[test]
fn detached_head() -> anyhow::Result<()> {
    let (_tmp, repo, mut meta, mut db) = named_writable_scenario("single-branch-with-3-commits")?;
    repo.reference(
        "HEAD",
        id_by_rev(&repo, "main~1"),
        PreviousValue::Any,
        "detach below main",
    )?;
    let graph = but_graph::Graph::from_head(
        &repo,
        &meta,
        project_meta(&repo)?,
        &mut db,
        Options::limited(),
    )?;
    let mut ws = graph.into_workspace()?;

    crate::utils::create_reference(
        &repo,
        &mut ws,
        &mut meta,
        &mut db,
        rc("refs/heads/first"),
        Anchor::at_id(id_by_rev(&repo, "main~2"), Above),
        None,
    )?;
    let err = crate::utils::create_reference(
        &repo,
        &mut ws,
        &mut meta,
        &mut db,
        rc("refs/heads/independent"),
        None,
        None,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Cannot create independent branch 'independent' outside of a workspace",
        "a detached HEAD has no workspace commit to parent an independent branch"
    );
    snapbox::assert_data_eq!(
        visualize_commit_graph_all(&repo)?,
        snapbox::str![[r#"
* 281da94 (main) 3
* 12995d7 (HEAD) 2
* 3d57fc1 (first) 1

"#]]
    );

    Ok(())
}

/// [`Anchor::AtReference`] in an ad-hoc (single-branch) workspace, where the tip-to-base order of
/// same-commit branches lives in the `branch_order` table rather than workspace metadata.
mod ad_hoc_at_reference {
    use super::*;
    use but_workspace::branch::create_reference::Position;

    /// A single-branch workspace checked out on `main` (2 commits above the target) with a
    /// *writable* branch-order backend, so `AtReference` placements can persist their order.
    fn ad_hoc_workspace() -> anyhow::Result<(
        tempfile::TempDir,
        gix::Repository,
        BranchOrderMetadata,
        but_core::ref_metadata::ProjectMeta,
        but_graph::Workspace,
        but_db::DbHandle,
    )> {
        let (tmp, repo, _legacy_meta, mut db) =
            named_writable_scenario("single-branch-with-3-commits")?;
        // Exercise branch ordering with a stored cutoff, not a configured target branch.
        let project_meta = but_core::ref_metadata::ProjectMeta {
            target_commit_id: Some(repo.rev_parse_single("main~2")?.detach()),
            ..Default::default()
        };
        let meta = branch_order_meta(&repo)?;
        let ws = but_graph::Graph::from_head(
            &repo,
            &meta,
            project_meta.clone(),
            &mut db,
            Options::limited(),
        )?
        .into_workspace()?;
        Ok((tmp, repo, meta, project_meta, ws, db))
    }

    /// Create `new_ref` positioned relative to `anchor_ref` and return the resulting workspace.
    fn create(
        repo: &gix::Repository,
        ws: &mut but_graph::Workspace,
        meta: &mut BranchOrderMetadata,
        db: &mut but_db::DbHandle,
        new_ref: &gix::refs::FullNameRef,
        anchor_ref: &gix::refs::FullNameRef,
        position: Position,
    ) -> anyhow::Result<but_graph::Workspace> {
        crate::utils::create_reference(
            repo,
            ws,
            meta,
            db,
            new_ref,
            Anchor::at_reference(anchor_ref, position),
            None,
        )
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
        let (_tmp, repo, mut meta, _project_meta, mut ws, mut db) = ad_hoc_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:main[🌳] <> ✓! on 3d57fc1
└── ≡:main[🌳] on 3d57fc1 {1}
    └── :main[🌳]
        ├── ·281da94
        └── ·12995d7

"#]]
        );

        let main_ref = r("refs/heads/main");
        let main_id = repo.find_reference(main_ref)?.id();
        let below_ref = r("refs/heads/new-below");
        create(
            &repo, &mut ws, &mut meta, &mut db, below_ref, main_ref, Below,
        )?;
        assert_order(
            &meta,
            main_ref,
            &["refs/heads/main", "refs/heads/new-below"],
        );
        assert_eq!(repo.find_reference(below_ref)?.id(), main_id);

        // A remote reference created after the editor's graph isn't part of the workspace.
        let remote_ref = r("refs/remotes/origin/main");
        repo.reference(
            remote_ref,
            main_id,
            PreviousValue::Any,
            "test remote anchor",
        )?;
        let err = create(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            r("refs/heads/new-remote-anchor"),
            remote_ref,
            Above,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "The anchor reference 'origin/main' isn't part of the workspace"
        );
        Ok(())
    }

    #[test]
    fn below_checked_out_branch_is_projected() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta, mut ws, mut db) = ad_hoc_workspace()?;

        let bottom_ref = r("refs/heads/empty-bottom");
        let main_ref = r("refs/heads/main");
        create(
            &repo, &mut ws, &mut meta, &mut db, bottom_ref, main_ref, Below,
        )?;

        let ws =
            but_graph::Graph::from_head(&repo, &meta, project_meta, &mut db, Options::limited())?
                .into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:main[🌳] <> ✓! on 3d57fc1
└── ≡:main[🌳] on 3d57fc1 {1}
    ├── :main[🌳]
    └── :empty-bottom
        ├── ·281da94
        └── ·12995d7

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
        let (_tmp, repo, mut meta, project_meta, ws, mut db) = ad_hoc_workspace()?;

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
            ws = create(
                &repo, &mut ws, &mut meta, &mut db, ref_name, anchor_ref, Below,
            )?;
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

        let ws =
            but_graph::Graph::from_head(&repo, &meta, project_meta, &mut db, Options::limited())?
                .into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:main[🌳] <> ✓! on 3d57fc1
└── ≡:main[🌳] on 3d57fc1 {1}
    ├── :main[🌳]
    ├── :empty-middle
    ├── :inserted-below-middle
    └── :empty-bottom
        ├── ·281da94
        └── ·12995d7

"#]]
        );
        Ok(())
    }

    #[test]
    fn above_checked_out_branch_is_projected_as_new_tip() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, _project_meta, mut ws, mut db) = ad_hoc_workspace()?;

        let top_ref = r("refs/heads/empty-top");
        let main_ref = r("refs/heads/main");
        let ws = create(&repo, &mut ws, &mut meta, &mut db, top_ref, main_ref, Above)?;

        // Creating above the checked-out branch makes the new empty branch the projected tip.
        // `create_reference` does not move `HEAD`; the caller checks the new tip out (see its docs).
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:empty-top[🌳] <> ✓! on 3d57fc1
└── ≡:empty-top[🌳] on 3d57fc1 {1}
    ├── :empty-top[🌳]
    └── :main
        ├── ·281da94
        └── ·12995d7

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
    fn order_survives_a_metadata_reload() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta, mut ws, mut db) = ad_hoc_workspace()?;
        let main_ref = r("refs/heads/main");
        let middle_ref = r("refs/heads/empty-middle");
        let bottom_ref = r("refs/heads/empty-bottom");

        let mut ws = create(
            &repo, &mut ws, &mut meta, &mut db, bottom_ref, main_ref, Below,
        )?;
        create(
            &repo, &mut ws, &mut meta, &mut db, middle_ref, main_ref, Below,
        )?;
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
        let ws =
            but_graph::Graph::from_head(&repo, &meta, project_meta, &mut db, Options::limited())?
                .into_workspace()?;
        snapbox::assert_data_eq!(
            graph_workspace(&ws).to_string(),
            snapbox::str![[r#"
⌂:main[🌳] <> ✓! on 3d57fc1
└── ≡:main[🌳] on 3d57fc1 {1}
    ├── :main[🌳]
    ├── :empty-middle
    └── :empty-bottom
        ├── ·281da94
        └── ·12995d7

"#]]
        );
        Ok(())
    }

    #[test]
    fn rejects_a_missing_anchor() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, _project_meta, mut ws, mut db) = ad_hoc_workspace()?;
        let new_ref = r("refs/heads/new");
        let missing_anchor = r("refs/heads/does-not-exist");

        let err = create(
            &repo,
            &mut ws,
            &mut meta,
            &mut db,
            new_ref,
            missing_anchor,
            Above,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("isn't part of the workspace"),
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
        let (_tmp, repo, mut meta, _project_meta, mut ws, mut db) = ad_hoc_workspace()?;
        let main_ref = r("refs/heads/main");

        // Positioning a ref relative to itself must be a clean validation error, not a panic.
        let err = create(
            &repo, &mut ws, &mut meta, &mut db, main_ref, main_ref, Above,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("already exists"),
            "self-referential placement must be rejected: {err}"
        );
        assert_eq!(meta.branch_stack_order(main_ref)?, None);
        Ok(())
    }

    #[test]
    fn reusing_an_existing_ref_for_a_different_commit_fails_without_mutation() -> anyhow::Result<()>
    {
        let (_tmp, repo, mut meta, _project_meta, mut ws, mut db) = ad_hoc_workspace()?;
        let main_ref = r("refs/heads/main");

        // `existing` already points at an older commit than `main`'s tip.
        let existing_ref = r("refs/heads/existing");
        let older = id_by_rev(&repo, "main~1").detach();
        repo.reference(existing_ref, older, PreviousValue::Any, "pre-existing ref")?;

        assert!(
            create(
                &repo,
                &mut ws,
                &mut meta,
                &mut db,
                existing_ref,
                main_ref,
                Above
            )
            .is_err(),
            "reusing an existing ref that points at a different commit must be rejected"
        );
        // The failure must be atomic: the ref is untouched and no order was persisted.
        assert_eq!(repo.find_reference(existing_ref)?.id(), older);
        assert_eq!(meta.branch_stack_order(main_ref)?, None);
        Ok(())
    }

    #[test]
    fn rejects_a_name_colliding_with_an_existing_branch() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, _project_meta, mut ws, mut db) = ad_hoc_workspace()?;
        let main_ref = r("refs/heads/main");

        // `refs/heads/main` exists as a file, so `refs/heads/main/child` cannot be created.
        let colliding = r("refs/heads/main/child");
        let err = create(
            &repo, &mut ws, &mut meta, &mut db, colliding, main_ref, Above,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("collides with existing branch"),
            "a name colliding with an existing branch should be reported clearly: {err}"
        );
        assert_eq!(meta.branch_stack_order(main_ref)?, None);
        Ok(())
    }

    #[test]
    fn interleaved_insertions_keep_a_consistent_order() -> anyhow::Result<()> {
        let (_tmp, repo, mut meta, project_meta, ws, mut db) = ad_hoc_workspace()?;
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
            ws = create(
                &repo, &mut ws, &mut meta, &mut db, new_ref, anchor_ref, Below,
            )?;
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
        let mut ws =
            but_graph::Graph::from_head(&repo, &meta, project_meta, &mut db, Options::limited())?
                .into_workspace()?;
        create(&repo, &mut ws, &mut meta, &mut db, crown, main_ref, Above)?;
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
