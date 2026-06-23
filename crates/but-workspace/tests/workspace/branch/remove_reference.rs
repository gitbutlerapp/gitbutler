use but_core::{RefMetadata, ref_metadata::StackId};
use but_graph::init::Options as GraphOptions;
use but_meta::BranchOrderMetadata;
use but_testsupport::{graph_tree, graph_workspace, visualize_commit_graph_all};
use but_workspace::branch::create_reference::{Anchor, Position::*};
use but_workspace::branch::remove_reference;
use gix::refs::{Category, transaction::PreviousValue};

use crate::{
    ref_info::with_workspace_commit::utils::{
        StackState, add_stack_with_segments, named_writable_scenario,
        named_writable_scenario_with_args_and_description_and_graph,
        named_writable_scenario_with_description_and_graph,
    },
    utils::r,
};

fn project_meta(meta: &impl RefMetadata) -> but_core::ref_metadata::ProjectMeta {
    meta.workspace(
        but_core::WORKSPACE_REF_NAME
            .try_into()
            .expect("valid workspace ref"),
    )
    .map(|workspace| workspace.project_meta())
    .unwrap_or_default()
}

fn branch_order_meta(repo: &gix::Repository) -> anyhow::Result<BranchOrderMetadata> {
    BranchOrderMetadata::from_paths(repo.path().join("virtual-branches.toml"), repo.path())
}

fn branch_order_meta_read_only(repo: &gix::Repository) -> anyhow::Result<BranchOrderMetadata> {
    BranchOrderMetadata::from_paths_read_only(
        repo.path().join("virtual-branches.toml"),
        repo.path(),
    )
}

fn stack_id_for_name(_rn: &gix::refs::FullNameRef) -> StackId {
    StackId::generate()
}

fn workspace_from_head(
    repo: &gix::Repository,
    meta: &impl RefMetadata,
    project_meta: but_core::ref_metadata::ProjectMeta,
) -> anyhow::Result<but_graph::Workspace> {
    but_graph::Graph::from_head(repo, meta, project_meta, GraphOptions::limited())?.into_workspace()
}

fn checkout_branch(
    repo: &gix::Repository,
    ref_name: &gix::refs::FullNameRef,
) -> anyhow::Result<()> {
    but_core::update_head_reference(
        repo,
        gix::refs::Target::Symbolic(ref_name.to_owned()),
        false,
        "checkout",
        ref_name.as_bstr(),
        1,
    )
    .map(|_| ())
}

fn create_ad_hoc_reference(
    repo: &gix::Repository,
    ws: &but_graph::Workspace,
    meta: &mut impl RefMetadata,
    ref_name: &gix::refs::FullNameRef,
    anchor_ref: &gix::refs::FullNameRef,
    position: but_workspace::branch::create_reference::Position,
) -> anyhow::Result<()> {
    but_workspace::branch::create_reference(
        ref_name,
        Anchor::AtReference {
            ref_name: std::borrow::Cow::Borrowed(anchor_ref),
            position,
        },
        repo,
        ws,
        meta,
        stack_id_for_name,
        None,
    )?;
    Ok(())
}

fn delete_reference_externally(
    repo: &gix::Repository,
    ref_name: &gix::refs::FullNameRef,
) -> anyhow::Result<()> {
    let reference = repo.find_reference(ref_name)?;
    reference.delete()?;
    Ok(())
}

fn assert_workspace_order(workspace: &but_graph::Workspace, refs: &[&str]) {
    let rendered = graph_workspace(workspace).to_string();
    let mut previous_idx = None;
    for ref_name in refs {
        let short_name = ref_name.trim_start_matches("refs/heads/");
        let idx = rendered
            .find(short_name)
            .unwrap_or_else(|| panic!("workspace should include {short_name}:\n{rendered}"));
        if let Some(previous_idx) = previous_idx {
            assert!(
                previous_idx < idx,
                "{short_name} should appear after the previous expected ref:\n{rendered}"
            );
        }
        previous_idx = Some(idx);
    }
}

#[test]
fn ad_hoc_remove_empty_branch_from_top_of_two_branch_stack() -> anyhow::Result<()> {
    let (_tmp, repo, legacy_meta) = named_writable_scenario("single-branch-with-3-commits")?;
    let project_meta = project_meta(&legacy_meta);
    let mut meta = branch_order_meta(&repo)?;
    let ws = workspace_from_head(&repo, &meta, project_meta.clone())?;

    let top_ref = r("refs/heads/empty-top");
    let bottom_ref = r("refs/heads/empty-bottom");
    let main_ref = r("refs/heads/main");
    create_ad_hoc_reference(&repo, &ws, &mut meta, top_ref, main_ref, Above)?;
    create_ad_hoc_reference(&repo, &ws, &mut meta, bottom_ref, top_ref, Below)?;
    checkout_branch(&repo, bottom_ref)?;
    let ws = workspace_from_head(&repo, &meta, project_meta.clone())?;

    let ws = but_workspace::branch::remove_reference(
        top_ref,
        &repo,
        &ws,
        &mut meta,
        remove_reference::Options::default(),
    )?
    .expect("the ordered empty top branch should be removed");

    assert!(repo.try_find_reference(top_ref)?.is_none());
    assert_eq!(
        meta.branch_stack_order(main_ref)?,
        Some(vec![bottom_ref.to_owned(), main_ref.to_owned()])
    );
    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:1:empty-bottom[🌳] <> ✓! on 281da94
    └── ≡📙:1:empty-bottom[🌳] {1}
        ├── 📙:1:empty-bottom[🌳]
        └── :0:main
            ├── ·281da94
            ├── ·12995d7
            └── ·3d57fc1
    ");

    Ok(())
}

#[test]
fn external_delete_of_ordered_top_branch_does_not_break_workspace_load() -> anyhow::Result<()> {
    let (_tmp, repo, legacy_meta) = named_writable_scenario("single-branch-with-3-commits")?;
    let project_meta = project_meta(&legacy_meta);
    let mut meta = branch_order_meta(&repo)?;
    let ws = workspace_from_head(&repo, &meta, project_meta.clone())?;

    let top_ref = r("refs/heads/empty-top");
    let bottom_ref = r("refs/heads/empty-bottom");
    let main_ref = r("refs/heads/main");
    create_ad_hoc_reference(&repo, &ws, &mut meta, top_ref, main_ref, Above)?;
    create_ad_hoc_reference(&repo, &ws, &mut meta, bottom_ref, top_ref, Below)?;

    checkout_branch(&repo, main_ref)?;
    delete_reference_externally(&repo, top_ref)?;
    drop(meta);
    let read_only_meta = branch_order_meta_read_only(&repo)?;
    assert_eq!(
        read_only_meta.branch_stack_order(main_ref)?,
        Some(vec![
            top_ref.to_owned(),
            bottom_ref.to_owned(),
            main_ref.to_owned()
        ]),
        "read-only metadata should still see the stale persisted order before graph filtering"
    );
    let graph = but_graph::Graph::from_head(
        &repo,
        &read_only_meta,
        project_meta.clone(),
        GraphOptions::limited(),
    )?;
    let rendered_graph = graph_tree(&graph).to_string();
    assert!(
        rendered_graph.contains("empty-bottom"),
        "graph should include surviving ordered ref before projection:\n{rendered_graph}"
    );
    let ws = graph.into_workspace()?;

    assert_workspace_order(&ws, &["refs/heads/empty-bottom", "refs/heads/main"]);
    let rendered = graph_workspace(&ws).to_string();
    assert!(
        !rendered.contains("empty-top"),
        "stale metadata should not keep deleted top branch visible:\n{rendered}"
    );
    Ok(())
}

#[test]
fn ad_hoc_remove_empty_branch_from_middle_of_three_branch_stack() -> anyhow::Result<()> {
    let (_tmp, repo, legacy_meta) = named_writable_scenario("single-branch-with-3-commits")?;
    let project_meta = project_meta(&legacy_meta);
    let mut meta = branch_order_meta(&repo)?;
    let ws = workspace_from_head(&repo, &meta, project_meta.clone())?;

    let top_ref = r("refs/heads/empty-top");
    let middle_ref = r("refs/heads/empty-middle");
    let bottom_ref = r("refs/heads/empty-bottom");
    let main_ref = r("refs/heads/main");
    create_ad_hoc_reference(&repo, &ws, &mut meta, top_ref, main_ref, Above)?;
    create_ad_hoc_reference(&repo, &ws, &mut meta, middle_ref, top_ref, Below)?;
    create_ad_hoc_reference(&repo, &ws, &mut meta, bottom_ref, middle_ref, Below)?;
    checkout_branch(&repo, top_ref)?;
    let ws = workspace_from_head(&repo, &meta, project_meta.clone())?;

    let ws = but_workspace::branch::remove_reference(
        middle_ref,
        &repo,
        &ws,
        &mut meta,
        remove_reference::Options::default(),
    )?
    .expect("the empty middle branch should be removed");

    assert!(repo.try_find_reference(middle_ref)?.is_none());
    assert_eq!(
        meta.branch_stack_order(main_ref)?,
        Some(vec![
            top_ref.to_owned(),
            bottom_ref.to_owned(),
            main_ref.to_owned()
        ])
    );
    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:1:empty-top[🌳] <> ✓! on 281da94
    └── ≡📙:1:empty-top[🌳] {1}
        ├── 📙:1:empty-top[🌳]
        ├── 📙:2:empty-bottom
        └── :0:main
            ├── ·281da94
            ├── ·12995d7
            └── ·3d57fc1
    ");

    Ok(())
}

#[test]
fn external_delete_of_ordered_middle_branch_does_not_break_workspace_load() -> anyhow::Result<()> {
    let (_tmp, repo, legacy_meta) = named_writable_scenario("single-branch-with-3-commits")?;
    let project_meta = project_meta(&legacy_meta);
    let mut meta = branch_order_meta(&repo)?;
    let ws = workspace_from_head(&repo, &meta, project_meta.clone())?;

    let top_ref = r("refs/heads/empty-top");
    let middle_ref = r("refs/heads/empty-middle");
    let bottom_ref = r("refs/heads/empty-bottom");
    let main_ref = r("refs/heads/main");
    create_ad_hoc_reference(&repo, &ws, &mut meta, top_ref, main_ref, Above)?;
    create_ad_hoc_reference(&repo, &ws, &mut meta, middle_ref, top_ref, Below)?;
    create_ad_hoc_reference(&repo, &ws, &mut meta, bottom_ref, middle_ref, Below)?;

    checkout_branch(&repo, main_ref)?;
    delete_reference_externally(&repo, middle_ref)?;
    drop(meta);
    let read_only_meta = branch_order_meta_read_only(&repo)?;
    let ws = workspace_from_head(&repo, &read_only_meta, project_meta)?;

    assert_workspace_order(
        &ws,
        &[
            "refs/heads/empty-top",
            "refs/heads/empty-bottom",
            "refs/heads/main",
        ],
    );
    let rendered = graph_workspace(&ws).to_string();
    assert!(
        !rendered.contains("empty-middle"),
        "stale metadata should not keep deleted middle branch visible:\n{rendered}"
    );
    Ok(())
}

#[test]
fn ad_hoc_remove_empty_branch_from_bottom_of_two_branch_stack() -> anyhow::Result<()> {
    let (_tmp, repo, legacy_meta) = named_writable_scenario("single-branch-with-3-commits")?;
    let project_meta = project_meta(&legacy_meta);
    let mut meta = branch_order_meta(&repo)?;
    let ws = workspace_from_head(&repo, &meta, project_meta.clone())?;

    let top_ref = r("refs/heads/empty-top");
    let bottom_ref = r("refs/heads/empty-bottom");
    let main_ref = r("refs/heads/main");
    create_ad_hoc_reference(&repo, &ws, &mut meta, top_ref, main_ref, Above)?;
    create_ad_hoc_reference(&repo, &ws, &mut meta, bottom_ref, top_ref, Below)?;
    checkout_branch(&repo, top_ref)?;
    let ws = workspace_from_head(&repo, &meta, project_meta.clone())?;

    let ws = but_workspace::branch::remove_reference(
        bottom_ref,
        &repo,
        &ws,
        &mut meta,
        remove_reference::Options::default(),
    )?
    .expect("the empty bottom branch should be removed");

    assert!(repo.try_find_reference(bottom_ref)?.is_none());
    assert_eq!(
        meta.branch_stack_order(main_ref)?,
        Some(vec![top_ref.to_owned(), main_ref.to_owned()])
    );
    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    ⌂:1:empty-top[🌳] <> ✓! on 281da94
    └── ≡📙:1:empty-top[🌳] {1}
        ├── 📙:1:empty-top[🌳]
        └── :0:main
            ├── ·281da94
            ├── ·12995d7
            └── ·3d57fc1
    ");

    Ok(())
}

#[test]
fn external_delete_of_ordered_bottom_branch_does_not_break_workspace_load() -> anyhow::Result<()> {
    let (_tmp, repo, legacy_meta) = named_writable_scenario("single-branch-with-3-commits")?;
    let project_meta = project_meta(&legacy_meta);
    let mut meta = branch_order_meta(&repo)?;
    let ws = workspace_from_head(&repo, &meta, project_meta.clone())?;

    let top_ref = r("refs/heads/empty-top");
    let bottom_ref = r("refs/heads/empty-bottom");
    let main_ref = r("refs/heads/main");
    create_ad_hoc_reference(&repo, &ws, &mut meta, top_ref, main_ref, Above)?;
    create_ad_hoc_reference(&repo, &ws, &mut meta, bottom_ref, top_ref, Below)?;

    checkout_branch(&repo, main_ref)?;
    delete_reference_externally(&repo, bottom_ref)?;
    drop(meta);
    let read_only_meta = branch_order_meta_read_only(&repo)?;
    let ws = workspace_from_head(&repo, &read_only_meta, project_meta)?;

    assert_workspace_order(&ws, &["refs/heads/empty-top", "refs/heads/main"]);
    let rendered = graph_workspace(&ws).to_string();
    assert!(
        !rendered.contains("empty-bottom"),
        "stale metadata should not keep deleted bottom branch visible:\n{rendered}"
    );
    Ok(())
}

#[test]
fn metadata_only_ordered_ref_does_not_create_phantom_branch() -> anyhow::Result<()> {
    let (_tmp, repo, legacy_meta) = named_writable_scenario("single-branch-with-3-commits")?;
    let project_meta = project_meta(&legacy_meta);
    let mut meta = branch_order_meta(&repo)?;
    let phantom_ref = r("refs/heads/phantom");
    let main_ref = r("refs/heads/main");
    meta.set_branch_stack_order(&[phantom_ref.to_owned(), main_ref.to_owned()])?;

    let read_only_meta = branch_order_meta_read_only(&repo)?;
    let ws = workspace_from_head(&repo, &read_only_meta, project_meta)?;
    let rendered = graph_workspace(&ws).to_string();

    assert!(
        !rendered.contains("phantom"),
        "metadata without a Git ref should not create a projected branch:\n{rendered}"
    );
    Ok(())
}

#[test]
fn no_errors_due_to_idempotency_in_empty_workspace() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, desc) =
        named_writable_scenario_with_args_and_description_and_graph(
            "single-branch-no-ws-commit-no-target",
            ["A", "B"],
            |_| {},
        )?;
    insta::assert_snapshot!(desc, @"Single commit, no main remote/target, no ws commit, but ws-reference");

    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, main, B, A) M1");
    let ws = graph.into_workspace()?;
    // the workspace is empty.
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43");

    for name in ["A", "B", "gitbutler/workspace", "main", "nonexisting"] {
        assert!(
            but_workspace::branch::remove_reference(
                Category::LocalBranch.to_full_name(name)?.as_ref(),
                &repo,
                &ws,
                &mut meta,
                remove_reference::Options {
                    keep_metadata: true,
                    ..Default::default()
                },
            )?
            .is_none()
        );

        assert!(
            but_workspace::branch::remove_reference(
                Category::LocalBranch.to_full_name(name)?.as_ref(),
                &repo,
                &ws,
                &mut meta,
                remove_reference::Options {
                    keep_metadata: false,
                    ..Default::default()
                },
            )?
            .is_none()
        );
    }

    // repo and workspace should still look like before.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, main, B, A) M1");
    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43");

    Ok(())
}

#[test]
fn journey_single_branch_no_ws_commit_no_target() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, desc) = named_writable_scenario_with_description_and_graph(
        "single-branch-3-commits-no-ws-commit-more-branches",
        |meta| {
            add_stack_with_segments(meta, 0, "A", StackState::InWorkspace, &[]);
        },
    )?;
    insta::assert_snapshot!(desc, @"Single commit, target, no ws commit, but ws-reference and a named segment, and branches on each commit");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * c2878fb (HEAD -> gitbutler/workspace, A2, A) A2
    * 49d4b34 (A1) A1
    * 3183e43 (origin/main, main) M1
    ");

    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    └── ≡📙:3:A on 3183e43 {0}
        ├── 📙:3:A
        │   └── ·c2878fb (🏘️) ►A2
        └── :4:A1
            └── ·49d4b34 (🏘️)
    ");

    // It's OK to delete all segment names of a stack
    for name in ["A", "A2", "A1"] {
        let r = Category::LocalBranch.to_full_name(name)?;
        ws = but_workspace::branch::remove_reference(
            r.as_ref(),
            &repo,
            &ws,
            &mut meta,
            remove_reference::Options {
                // This is what allows us to delete everything.
                avoid_anonymous_stacks: false,
                ..Default::default()
            },
        )?
        .expect("we deleted something");
    }

    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on 3183e43
    └── ≡:3:anon: on 3183e43
        └── :3:anon:
            ├── ·c2878fb (🏘️)
            └── ·49d4b34 (🏘️)
    ");

    Ok(())
}

#[test]
fn journey_single_branch_ws_commit_no_target() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, desc) = named_writable_scenario_with_description_and_graph(
        "single-branch-4-commits-more-branches",
        |meta| {
            add_stack_with_segments(
                meta,
                0,
                "A",
                StackState::InWorkspace,
                &["A2-3", "A2-2", "A2-1", "A1-1", "A1-2", "A1-3"],
            );
        },
    )?;

    insta::assert_snapshot!(desc, @"Two commits in main, target setup, ws commit, many more usable branches");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"
    * 05240ea (HEAD -> gitbutler/workspace) GitButler Workspace Commit
    * 43f9472 (A2-3, A2-2, A2-1, A) A2
    * 6fdab32 (A1-3, A1-2, A1-1) A1
    * bce0c5e (origin/main, main) M2
    * 3183e43 M1
    ");
    let mut ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡📙:5:A on bce0c5e {0}
        ├── 📙:5:A
        ├── 📙:6:A2-3
        ├── 📙:7:A2-2
        ├── 📙:8:A2-1
        │   └── ·43f9472 (🏘️)
        ├── 📙:9:A1-1
        ├── 📙:10:A1-2
        └── 📙:11:A1-3
            └── ·6fdab32 (🏘️)
    ");

    // Delete a whole segment to see how it pulls up to the top of the stack a branch from below
    for name in ["A2-1", "A2-3", "A", "A2-2"] {
        let r = Category::LocalBranch.to_full_name(name)?;
        ws = but_workspace::branch::remove_reference(
            r.as_ref(),
            &repo,
            &ws,
            &mut meta,
            remove_reference::Options {
                // This causes "A1-1" to become the top of the stack.
                avoid_anonymous_stacks: true,
                ..Default::default()
            },
        )?
        .expect("we deleted something");
    }
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡📙:3:A1-1 on bce0c5e {0}
        ├── 📙:3:A1-1
        │   └── ·43f9472 (🏘️)
        ├── 📙:5:A1-2
        └── 📙:6:A1-3
            └── ·6fdab32 (🏘️)
    ");

    for name in ["A1-1", "A1-2"] {
        let r = Category::LocalBranch.to_full_name(name)?;
        ws = but_workspace::branch::remove_reference(
            r.as_ref(),
            &repo,
            &ws,
            &mut meta,
            remove_reference::Options {
                avoid_anonymous_stacks: true,
                ..Default::default()
            },
        )?
        .expect("we deleted something");
    }
    // Just one segment left.
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️:0:gitbutler/workspace[🌳] <> ✓refs/remotes/origin/main on bce0c5e
    └── ≡📙:3:A1-3 on bce0c5e {0}
        └── 📙:3:A1-3
            ├── ·43f9472 (🏘️)
            └── ·6fdab32 (🏘️)
    ");

    let err = but_workspace::branch::remove_reference(
        r("refs/heads/A1-3"),
        &repo,
        &ws,
        &mut meta,
        remove_reference::Options {
            avoid_anonymous_stacks: true,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "Refusing to delete last named segment 'A1-3' as it would leave an anonymous segment",
        "won't allow to create anon segment by deleting the last one."
    );

    Ok(())
}

#[test]
fn journey_no_ws_commit_no_target() -> anyhow::Result<()> {
    let (_tmp, graph, repo, mut meta, desc) =
        named_writable_scenario_with_args_and_description_and_graph(
            "single-branch-no-ws-commit-no-target",
            ["A", "B", "C", "D", "E"],
            |meta| {
                add_stack_with_segments(meta, 0, "A", StackState::InWorkspace, &["B", "C"]);
                add_stack_with_segments(meta, 1, "D", StackState::InWorkspace, &["E"]);
            },
        )?;
    insta::assert_snapshot!(desc, @"Single commit, no main remote/target, no ws commit, but ws-reference");
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, main, E, D, C, B, A) M1");

    let ws = graph.into_workspace()?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:2:A on 3183e43 {0}
    │   ├── 📙:2:A
    │   ├── 📙:3:B
    │   └── 📙:4:C
    └── ≡📙:5:D on 3183e43 {1}
        ├── 📙:5:D
        └── 📙:6:E
    ");

    let ref_name = r("refs/heads/A");
    let ws = but_workspace::branch::remove_reference(
        ref_name,
        &repo,
        &ws,
        &mut meta,
        remove_reference::Options {
            keep_metadata: true,
            ..Default::default()
        },
    )?
    .expect("we deleted something");

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:2:B on 3183e43 {0}
    │   ├── 📙:2:B
    │   └── 📙:3:C
    └── ≡📙:4:D on 3183e43 {1}
        ├── 📙:4:D
        └── 📙:5:E
    ");

    let main_id = repo.head_id()?;
    repo.reference(
        ref_name,
        main_id,
        PreviousValue::Any,
        "recreate ref to show metadata is present and unchanged",
    )?;

    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:2:A on 3183e43 {0}
    │   ├── 📙:2:A
    │   ├── 📙:3:B
    │   └── 📙:4:C
    └── ≡📙:5:D on 3183e43 {1}
        ├── 📙:5:D
        └── 📙:6:E
    ");

    let mut ws = but_workspace::branch::remove_reference(
        ref_name,
        &repo,
        &ws,
        &mut meta,
        remove_reference::Options::default(),
    )?
    .expect("we deleted something");
    repo.reference(
        ref_name,
        main_id,
        PreviousValue::Any,
        "recreate ref - this time it's not visible as it lacks metadata",
    )?;

    insta::assert_snapshot!(graph_workspace(&ws), @"
    📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43
    ├── ≡📙:2:B on 3183e43 {0}
    │   ├── 📙:2:B
    │   └── 📙:3:C
    └── ≡📙:4:D on 3183e43 {1}
        ├── 📙:4:D
        └── 📙:5:E
    ");

    // Try to delete it again, just to see that it doesn't try to touch it as it's outside the workspace.
    assert!(
        but_workspace::branch::remove_reference(
            ref_name,
            &repo,
            &ws,
            &mut meta,
            remove_reference::Options::default(),
        )?
        .is_none()
    );
    assert!(
        repo.find_reference(ref_name).is_ok(),
        "The reference still exist as we only remove what's in the workspace, nothing arbitrary"
    );

    // We can delete everything.
    for name in ["D", "B", "E", "C"] {
        let r = Category::LocalBranch.to_full_name(name)?;
        ws = but_workspace::branch::remove_reference(
            r.as_ref(),
            &repo,
            &ws,
            &mut meta,
            remove_reference::Options {
                // This has no effect
                avoid_anonymous_stacks: true,
                ..Default::default()
            },
        )?
        .expect("we deleted something");
    }

    // A remains as we recreated it.
    insta::assert_snapshot!(visualize_commit_graph_all(&repo)?, @"* 3183e43 (HEAD -> gitbutler/workspace, main, A) M1");
    let ws = ws.graph.into_workspace_of_redone_traversal(&repo, &meta)?;
    // The workspace is completely empty.
    insta::assert_snapshot!(graph_workspace(&ws), @"📕🏘️⚠️:0:gitbutler/workspace[🌳] <> ✓! on 3183e43");

    assert_eq!(
        meta.iter().count(),
        0,
        "nothing is left in the metadata either"
    );

    Ok(())
}
