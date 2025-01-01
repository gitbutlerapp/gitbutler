use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
};
#[cfg(target_family = "unix")]
use std::{
    fs::Permissions,
    os::unix::{fs::symlink, prelude::*},
};

use anyhow::{Context, Result};
use bstr::ByteSlice;
use git2::TreeEntry;
use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
use gitbutler_branch_actions::{
    get_applied_status, internal, list_commit_files, update_workspace_commit, verify_branch,
    BranchManagerExt, Get,
};
use gitbutler_commit::{commit_ext::CommitExt, commit_headers::CommitHeadersV2};
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::committing::RepositoryExt as _;
use gitbutler_stack::{BranchOwnershipClaims, Target, VirtualBranchesHandle};
use gitbutler_testsupport::{commit_all, virtual_branches::set_test_target, Case, Suite};
use pretty_assertions::assert_eq;

#[test]
fn commit_on_branch_then_change_file_then_get_status() -> Result<()> {
    let suite = Suite::default();
    let Case { project, ctx, .. } = &suite.new_case_with_files(HashMap::from([
        (PathBuf::from("test.txt"), "line1\nline2\nline3\nline4\n"),
        (PathBuf::from("test2.txt"), "line5\nline6\nline7\nline8\n"),
    ]));

    set_test_target(ctx)?;

    let mut guard = project.exclusive_worktree_access();
    let stack1_id = ctx
        .branch_manager()
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;

    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.series[0].clone()?.patches.len(), 0);

    // commit
    internal::commit(ctx, stack1_id, "test commit", None, false)?;

    // status (no files)
    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.series[0].clone()?.patches.len(), 1);

    std::fs::write(
        Path::new(&project.path).join("test2.txt"),
        "line5\nline6\nlineBLAH\nline7\nline8\n",
    )?;

    // should have just the last change now, the other line is committed
    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.series[0].clone()?.patches.len(), 1);

    Ok(())
}

#[test]
fn track_binary_files() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case();

    let file_path = Path::new("test.txt");
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    let file_path2 = Path::new("test2.txt");
    std::fs::write(
        Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\n",
    )?;
    // add a binary file
    let image_data: [u8; 12] = [
        255, 0, 0, // Red pixel
        0, 0, 255, // Blue pixel
        255, 255, 0, // Yellow pixel
        0, 255, 0, // Green pixel
    ];
    let mut file = std::fs::File::create(Path::new(&project.path).join("image.bin"))?;
    file.write_all(&image_data)?;
    commit_all(ctx.repo());

    set_test_target(ctx)?;

    let mut guard = project.exclusive_worktree_access();
    let stack1_id = ctx
        .branch_manager()
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    // test file change
    std::fs::write(
        Path::new(&project.path).join(file_path2),
        "line5\nline6\nline7\nline8\nline9\n",
    )?;

    // add a binary file
    let image_data: [u8; 12] = [
        255, 0, 0, // Red pixel
        0, 255, 0, // Green pixel
        0, 0, 255, // Blue pixel
        255, 255, 0, // Yellow pixel
    ];
    let mut file = std::fs::File::create(Path::new(&project.path).join("image.bin"))?;
    file.write_all(&image_data)?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches[0];
    assert_eq!(branch.files.len(), 2);
    let img_file = &branch
        .files
        .iter()
        .find(|b| b.path.as_os_str() == "image.bin")
        .unwrap();
    assert!(img_file.binary);
    let img_oid_hex = "944996dd82015a616247c72b251e41661e528ae1";
    assert_eq!(
        img_file.hunks[0].diff, img_oid_hex,
        "the binary file was stored in the ODB as otherwise we wouldn't have its contents. \
        It cannot easily be reconstructed from the diff-lines, or we don't attempt it."
    );

    // commit
    internal::commit(ctx, stack1_id, "test commit", None, false)?;

    // status (no files)
    let list_result = internal::list_virtual_branches(ctx, guard.write_permission()).unwrap();
    let branches = list_result.branches;
    let commit_id = &branches[0].series[0].clone()?.patches[0].id;
    let commit_obj = ctx.repo().find_commit(commit_id.to_owned())?;
    let tree = commit_obj.tree()?;
    let files = tree_to_entry_list(ctx.repo(), &tree);
    assert_eq!(files[0].0, "image.bin");
    assert_eq!(
        files[0].3, img_oid_hex,
        "our vbranch commit tree references the binary object we previously stored"
    );

    let image_data: [u8; 12] = [
        0, 255, 0, // Green pixel
        255, 0, 0, // Red pixel
        255, 255, 0, // Yellow pixel
        0, 0, 255, // Blue pixel
    ];
    let mut file = std::fs::File::create(Path::new(&project.path).join("image.bin"))?;
    file.write_all(&image_data)?;

    // commit
    internal::commit(ctx, stack1_id, "test commit", None, false)?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission()).unwrap();
    let branches = list_result.branches;

    let commit_id = &branches[0].series[0].clone()?.patches[0].id;
    // get tree from commit_id
    let commit_obj = ctx.repo().find_commit(commit_id.to_owned())?;
    let tree = commit_obj.tree()?;
    let files = tree_to_entry_list(ctx.repo(), &tree);

    assert_eq!(files[0].0, "image.bin");
    assert_eq!(files[0].3, "ea6901a04d1eed6ebf6822f4360bda9f008fa317");

    Ok(())
}

#[test]
fn create_branch_with_ownership() -> Result<()> {
    let suite = Suite::default();
    let Case { project, ctx, .. } = &suite.new_case();

    set_test_target(ctx)?;

    let file_path = Path::new("test.txt");
    std::fs::write(Path::new(&project.path).join(file_path), "line1\nline2\n").unwrap();

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let branch0 = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch");

    get_applied_status(ctx, None).expect("failed to get status");

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let stack0 = vb_state.get_stack_in_workspace(branch0.id).unwrap();

    let branch1 = branch_manager
        .create_virtual_branch(
            &BranchCreateRequest {
                ownership: Some(stack0.ownership),
                ..Default::default()
            },
            guard.write_permission(),
        )
        .expect("failed to create virtual branch");

    let statuses = get_applied_status(ctx, None)
        .expect("failed to get status")
        .branches;

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&stack0.id].len(), 0);
    assert_eq!(files_by_branch_id[&branch1.id].len(), 1);

    Ok(())
}

#[test]
fn create_branch_in_the_middle() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case();

    set_test_target(ctx)?;

    let branch_manager = ctx.branch_manager();
    branch_manager
        .create_virtual_branch(
            &BranchCreateRequest::default(),
            project.exclusive_worktree_access().write_permission(),
        )
        .expect("failed to create virtual branch");
    branch_manager
        .create_virtual_branch(
            &BranchCreateRequest::default(),
            project.exclusive_worktree_access().write_permission(),
        )
        .expect("failed to create virtual branch");
    branch_manager
        .create_virtual_branch(
            &BranchCreateRequest {
                order: Some(1),
                ..Default::default()
            },
            project.exclusive_worktree_access().write_permission(),
        )
        .expect("failed to create virtual branch");

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let mut stacks = vb_state
        .list_stacks_in_workspace()
        .expect("failed to read branches");
    stacks.sort_by_key(|b| b.order);
    assert_eq!(stacks.len(), 3);
    assert_eq!(stacks[0].name, "Lane");
    assert_eq!(stacks[1].name, "Lane 2");
    assert_eq!(stacks[2].name, "Lane 1");

    Ok(())
}

#[test]
fn create_branch_no_arguments() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case();

    set_test_target(ctx)?;

    let branch_manager = ctx.branch_manager();
    branch_manager
        .create_virtual_branch(
            &BranchCreateRequest::default(),
            project.exclusive_worktree_access().write_permission(),
        )
        .expect("failed to create virtual branch");

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let stacks = vb_state
        .list_stacks_in_workspace()
        .expect("failed to read branches");
    assert_eq!(stacks.len(), 1);
    assert_eq!(stacks[0].name, "Lane");
    assert_eq!(stacks[0].ownership, BranchOwnershipClaims::default());
    assert_eq!(stacks[0].order, 0);

    Ok(())
}

#[test]
fn hunk_expantion() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case();

    set_test_target(ctx)?;

    let file_path = Path::new("test.txt");
    std::fs::write(Path::new(&project.path).join(file_path), "line1\nline2\n")?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;
    let stack2_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    let statuses = get_applied_status(ctx, None)
        .expect("failed to get status")
        .branches;

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&stack1_id].len(), 1);
    assert_eq!(files_by_branch_id[&stack2_id].len(), 0);

    // even though selected branch has changed
    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: stack1_id,
            order: Some(1),
            ..Default::default()
        },
    )?;
    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: stack2_id,
            order: Some(0),
            ..Default::default()
        },
    )?;

    // a slightly different hunk should still go to the same branch
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\n",
    )?;

    let statuses = get_applied_status(ctx, None)
        .expect("failed to get status")
        .branches;
    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&stack1_id].len(), 1);
    assert_eq!(files_by_branch_id[&stack2_id].len(), 0);

    Ok(())
}

#[test]
fn get_status_files_by_branch_no_hunks_no_branches() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, .. } = &suite.new_case();

    set_test_target(ctx)?;

    let statuses = get_applied_status(ctx, None)
        .expect("failed to get status")
        .branches;

    assert_eq!(statuses.len(), 0);

    Ok(())
}

#[test]
fn get_status_files_by_branch() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case();

    set_test_target(ctx)?;

    let file_path = Path::new("test.txt");
    std::fs::write(Path::new(&project.path).join(file_path), "line1\nline2\n")?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;
    let stack2_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    let statuses = get_applied_status(ctx, None)
        .expect("failed to get status")
        .branches;
    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&stack1_id].len(), 1);
    assert_eq!(files_by_branch_id[&stack2_id].len(), 0);

    Ok(())
}

#[test]
fn move_hunks_multiple_sources() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case_with_files(HashMap::from([(
        PathBuf::from("test.txt"),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\n",
    )]));

    set_test_target(ctx)?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;
    let stack2_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;
    let stack3_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
    )?;

    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let mut stack2 = vb_state.get_stack_in_workspace(stack2_id)?;
    stack2.ownership = BranchOwnershipClaims {
        claims: vec!["test.txt:1-5".parse()?],
    };
    vb_state.set_stack(stack2.clone())?;
    let mut stack1 = vb_state.get_stack_in_workspace(stack1_id)?;
    stack1.ownership = BranchOwnershipClaims {
        claims: vec!["test.txt:11-15".parse()?],
    };
    vb_state.set_stack(stack1.clone())?;

    let statuses = get_applied_status(ctx, None)
        .expect("failed to get status")
        .branches;

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 3);
    assert_eq!(files_by_branch_id[&stack1_id].len(), 1);
    // assert_eq!(files_by_branch_id[&stack1_id][0].hunks.len(), 1);
    assert_eq!(files_by_branch_id[&stack2_id].len(), 1);
    // assert_eq!(files_by_branch_id[&stack2_id][0].hunks.len(), 1);
    assert_eq!(files_by_branch_id[&stack3_id].len(), 0);

    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: stack3_id,
            ownership: Some("test.txt:1-5,11-15".parse()?),
            ..Default::default()
        },
    )?;

    let statuses = get_applied_status(ctx, None)
        .expect("failed to get status")
        .branches;

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 3);
    assert_eq!(files_by_branch_id[&stack1_id].len(), 0);
    assert_eq!(files_by_branch_id[&stack2_id].len(), 0);
    assert_eq!(files_by_branch_id[&stack3_id].len(), 1);
    assert_eq!(
        files_by_branch_id[&stack3_id]
            .get(Path::new("test.txt"))
            .unwrap()
            .hunks
            .len(),
        2
    );
    assert_eq!(
        files_by_branch_id[&stack3_id]
            .get(Path::new("test.txt"))
            .unwrap()
            .hunks[0]
            .diff,
        "@@ -1,3 +1,4 @@\n+line0\n line1\n line2\n line3\n"
    );
    assert_eq!(
        files_by_branch_id[&stack3_id]
            .get(Path::new("test.txt"))
            .unwrap()
            .hunks[1]
            .diff,
        "@@ -10,3 +11,4 @@ line9\n line10\n line11\n line12\n+line13\n"
    );
    Ok(())
}

#[test]
fn move_hunks_partial_explicitly() -> Result<()> {
    let suite = Suite::default();
    let Case {
        ctx,
        project,
        ..
    } = &suite.new_case_with_files(HashMap::from([(
        PathBuf::from("test.txt"),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\n",
    )]));

    set_test_target(ctx)?;

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\n",
    )?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    let stack2_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    let statuses = get_applied_status(ctx, None)
        .expect("failed to get status")
        .branches;
    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&stack1_id].len(), 1);
    // assert_eq!(files_by_branch_id[&stack1_id][0].hunks.len(), 2);
    assert_eq!(files_by_branch_id[&stack2_id].len(), 0);

    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: stack2_id,
            ownership: Some("test.txt:1-5".parse()?),
            ..Default::default()
        },
    )?;

    let statuses = get_applied_status(ctx, None)
        .expect("failed to get status")
        .branches;

    let files_by_branch_id = statuses
        .iter()
        .map(|(branch, files)| (branch.id, files))
        .collect::<HashMap<_, _>>();

    assert_eq!(files_by_branch_id.len(), 2);
    assert_eq!(files_by_branch_id[&stack1_id].len(), 1);
    assert_eq!(
        files_by_branch_id[&stack1_id]
            .get(Path::new("test.txt"))
            .unwrap()
            .hunks
            .len(),
        1
    );
    assert_eq!(
        files_by_branch_id[&stack1_id]
            .get(Path::new("test.txt"))
            .unwrap()
            .hunks[0]
            .diff,
        "@@ -11,3 +12,4 @@ line10\n line11\n line12\n line13\n+line14\n"
    );

    assert_eq!(files_by_branch_id[&stack2_id].len(), 1);
    assert_eq!(
        files_by_branch_id[&stack2_id]
            .get(Path::new("test.txt"))
            .unwrap()
            .hunks
            .len(),
        1
    );
    assert_eq!(
        files_by_branch_id[&stack2_id]
            .get(Path::new("test.txt"))
            .unwrap()
            .hunks[0]
            .diff,
        "@@ -1,3 +1,4 @@\n+line0\n line1\n line2\n line3\n"
    );

    Ok(())
}

#[test]
fn add_new_hunk_to_the_end() -> Result<()> {
    let suite = Suite::default();
    let Case {
        ctx,
        project,
        ..
    } = &suite.new_case_with_files(HashMap::from([(
        PathBuf::from("test.txt"),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline13\nline14\n",
    )]));

    set_test_target(ctx)?;

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\nline15\n",
    )?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch");

    let statuses = get_applied_status(ctx, None)
        .expect("failed to get status")
        .branches;
    assert_eq!(
        statuses[0].1.get(Path::new("test.txt")).unwrap().hunks[0].diff,
        "@@ -11,5 +11,5 @@ line10\n line11\n line12\n line13\n-line13\n line14\n+line15\n"
    );

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\nline12\nline13\nline14\nline15\n",
    )?;

    let statuses = get_applied_status(ctx, None)
        .expect("failed to get status")
        .branches;

    assert_eq!(
        statuses[0].1.get(Path::new("test.txt")).unwrap().hunks[0].diff,
        "@@ -11,5 +12,5 @@ line10\n line11\n line12\n line13\n-line13\n line14\n+line15\n"
    );
    assert_eq!(
        statuses[0].1.get(Path::new("test.txt")).unwrap().hunks[1].diff,
        "@@ -1,3 +1,4 @@\n+line0\n line1\n line2\n line3\n"
    );

    Ok(())
}

#[test]
fn commit_id_can_be_generated_or_specified() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case();

    let file_path = Path::new("test.txt");
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    commit_all(ctx.repo());

    // lets make sure a change id is generated
    let target_oid = ctx.repo().head().unwrap().target().unwrap();
    let target = ctx.repo().find_commit(target_oid).unwrap();
    let change_id = target.change_id();

    // make sure we created a change-id
    assert!(change_id.is_some());

    // ok, make another change and specify a change-id
    let file_path = Path::new("test.txt");
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nline5\n",
    )?;

    let repository = ctx.repo();
    let mut index = repository.index().expect("failed to get index");
    index
        .add_all(["."], git2::IndexAddOption::DEFAULT, None)
        .expect("failed to add all");
    index.write().expect("failed to write index");
    let oid = index.write_tree().expect("failed to write tree");
    let signature = git2::Signature::now("test", "test@email.com").unwrap();
    let head = repository.head().expect("failed to get head");
    let refname: Refname = head.name().unwrap().parse().unwrap();
    ctx.repo()
        .commit_with_signature(
            Some(&refname),
            &signature,
            &signature,
            "some commit",
            &repository.find_tree(oid).expect("failed to find tree"),
            &[&repository
                .find_commit(
                    repository
                        .refname_to_id("HEAD")
                        .expect("failed to get head"),
                )
                .expect("failed to find commit")],
            // The change ID should always be generated by calling CommitHeadersV2::new
            Some(CommitHeadersV2 {
                change_id: "my-change-id".to_string(),
                conflicted: None,
            }),
        )
        .expect("failed to commit");

    let target_oid = ctx.repo().head().unwrap().target().unwrap();
    let target = ctx.repo().find_commit(target_oid).unwrap();
    let change_id = target.change_id();

    // the change id should be what we specified, rather than randomly generated
    assert_eq!(change_id, Some("my-change-id".to_string()));
    Ok(())
}

/// This sets up the following scenario:
///
/// Target commit:
/// test.txt: line1\nline2\nline3\nline4\n
///
/// Make commit "last push":
/// test.txt: line1\nline2\nline3\nline4\nupstream\n
///
/// "Server side" origin/master:
/// test.txt: line1\nline2\nline3\nline4\nupstream\ncoworker work\n
///
/// Write uncommited:
/// test.txt: line1\nline2\nline3\nline4\nupstream\n
/// test2.txt: file2\n
///
/// Create vbranch:
///    - set head to "last push"
///
/// Inspect Virtual branch:
/// commited: test.txt: line1\nline2\nline3\nline4\n+upstream\n
/// uncommited: test2.txt: file2\n
#[test]
fn merge_vbranch_upstream_clean_rebase() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &mut suite.new_case();

    // create a commit and set the target
    let file_path = Path::new("test.txt");
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    commit_all(ctx.repo());
    let target_oid = ctx.repo().head().unwrap().target().unwrap();

    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    // add a commit to the target branch it's pointing to so there is something "upstream"
    commit_all(ctx.repo());
    let last_push = ctx.repo().head().unwrap().target().unwrap();

    // coworker adds some work
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\ncoworker work\n",
    )?;

    commit_all(ctx.repo());
    let coworker_work = ctx.repo().head().unwrap().target().unwrap();

    //update repo ref refs/remotes/origin/master to up_target oid
    ctx.repo().reference(
        "refs/remotes/origin/master",
        coworker_work,
        true,
        "update target",
    )?;

    // revert to our file
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;

    set_test_target(ctx)?;
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    vb_state.set_default_target(Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: target_oid,
        push_remote_name: None,
    })?;

    // add some uncommitted work
    let file_path2 = Path::new("test2.txt");
    std::fs::write(Path::new(&project.path).join(file_path2), "file2\n")?;

    // Update workspace commit
    update_workspace_commit(&vb_state, ctx)?;

    let remote_branch: RemoteRefname = "refs/remotes/origin/master".parse().unwrap();
    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let mut branch = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch");

    branch.upstream = Some(remote_branch.clone());
    branch.set_stack_head(ctx, last_push, None)?;

    // create the branch
    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    assert_eq!(branches.len(), 1);
    let branch1 = &branches[0];

    assert_eq!(
        branch1.files.len(),
        1,
        "test2.txt contains uncommited changes"
    );
    assert_eq!(branch1.files[0].path.to_str().unwrap(), "test2.txt");
    assert_eq!(
        branch1.files[0].hunks[0].diff.to_str().unwrap(),
        "@@ -0,0 +1 @@\n+file2\n"
    );

    assert_eq!(
        branch1.series[0].clone()?.patches.len(),
        1,
        "test.txt is commited inside this commit"
    );
    let commit1_files = list_commit_files(project, branch1.series[0].clone()?.patches[0].id)?;
    assert_eq!(commit1_files.len(), 1);
    assert_eq!(commit1_files[0].path.to_str().unwrap(), "test.txt");
    assert_eq!(
        commit1_files[0].hunks[0].diff_lines.to_str().unwrap(),
        "@@ -2,3 +2,4 @@ line1\n line2\n line3\n line4\n+upstream\n"
    );
    // assert_eq!(branch1.upstream.as_ref().unwrap().series[0].clone()?.patches.len(), 1);

    internal::branch_upstream_integration::integrate_upstream_commits(
        ctx,
        branch1.id,
        guard.write_permission(),
    )?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch1 = &branches[0];

    let contents = std::fs::read(Path::new(&project.path).join(file_path))?;
    assert_eq!(
        "line1\nline2\nline3\nline4\nupstream\ncoworker work\n",
        String::from_utf8(contents)?
    );
    let contents = std::fs::read(Path::new(&project.path).join(file_path2))?;
    assert_eq!("file2\n", String::from_utf8(contents)?);
    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.series[0].clone()?.patches.len(), 2);
    // assert_eq!(branch1.upstream.as_ref().unwrap().series[0].clone()?.patches.len(), 0);

    Ok(())
}

#[test]
fn merge_vbranch_upstream_conflict() -> Result<()> {
    let suite = Suite::default();
    let mut case = suite.new_case();

    case = case.refresh(&suite);
    let ctx = &case.ctx;
    let project = &case.project;

    // create a commit and set the target
    let file_path = Path::new("test.txt");
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    commit_all(ctx.repo());
    let target_oid = ctx.repo().head().unwrap().target().unwrap();

    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    // add a commit to the target branch it's pointing to so there is something "upstream"
    commit_all(ctx.repo());
    let last_push = ctx.repo().head().unwrap().target().unwrap();

    // coworker adds some work
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\ncoworker work\n",
    )?;

    commit_all(ctx.repo());
    let coworker_work = ctx.repo().head().unwrap().target().unwrap();

    //update repo ref refs/remotes/origin/master to up_target oid
    ctx.repo().reference(
        "refs/remotes/origin/master",
        coworker_work,
        true,
        "update target",
    )?;

    // revert to our file
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;

    set_test_target(ctx)?;
    let vb_state = VirtualBranchesHandle::new(project.gb_dir());
    vb_state.set_default_target(Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "origin".to_string(),
        sha: target_oid,
        push_remote_name: None,
    })?;

    // add some uncommitted work
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\nother side\n",
    )?;

    let remote_branch: RemoteRefname = "refs/remotes/origin/master".parse().unwrap();
    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let mut branch = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch");
    branch.upstream = Some(remote_branch.clone());
    branch.set_stack_head(ctx, last_push, None)?;

    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: branch.id,
            allow_rebasing: Some(false),
            ..Default::default()
        },
    )
    .unwrap();

    // create the branch
    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch1 = &branches[0];

    assert_eq!(branch1.files.len(), 1);
    assert_eq!(branch1.series[0].clone()?.patches.len(), 1);
    // assert_eq!(branch1.upstream.as_ref().unwrap().series[0].clone()?.patches.len(), 1);

    internal::branch_upstream_integration::integrate_upstream_commits(
        ctx,
        branch1.id,
        guard.write_permission(),
    )?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch1 = &branches[0];
    let contents = std::fs::read(Path::new(&project.path).join(file_path))?;

    assert_eq!(
        "line1\nline2\nline3\nline4\nupstream\ncoworker work\n",
        String::from_utf8(contents)?
    );

    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.series[0].clone()?.patches.len(), 3); // Local commits including the merge commit
    assert_eq!(branch1.series[0].clone().unwrap().patches.len(), 3);
    assert_eq!(branch1.series[0].clone().unwrap().upstream_patches.len(), 0);
    assert!(!branch1.conflicted);

    Ok(())
}

#[test]
fn unapply_ownership_partial() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case_with_files(HashMap::from([(
        PathBuf::from("test.txt"),
        "line1\nline2\nline3\nline4\n",
    )]));

    set_test_target(ctx)?;

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line1\nline2\nline3\nline4\nbranch1\n",
    )?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch");

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 1);
    assert_eq!(branches[0].ownership.claims.len(), 1);
    assert_eq!(branches[0].files[0].hunks.len(), 1);
    assert_eq!(branches[0].ownership.claims[0].hunks.len(), 1);
    assert_eq!(
        std::fs::read_to_string(Path::new(&project.path).join("test.txt"))?,
        "line1\nline2\nline3\nline4\nbranch1\n"
    );

    internal::unapply_ownership(
        ctx,
        &"test.txt:2-6".parse().unwrap(),
        None,
        guard.write_permission(),
    )
    .unwrap();

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].files.len(), 0);
    assert_eq!(branches[0].ownership.claims.len(), 0);
    assert_eq!(
        std::fs::read_to_string(Path::new(&project.path).join("test.txt"))?,
        "line1\nline2\nline3\nline4\n"
    );

    Ok(())
}

#[test]
fn unapply_branch() -> Result<()> {
    let suite = Suite::default();
    let Case { project, ctx, .. } = &suite.new_case();

    // create a commit and set the target
    let file_path = Path::new("test.txt");
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    commit_all(ctx.repo());

    set_test_target(ctx)?;

    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nbranch1\n",
    )?;
    let file_path2 = Path::new("test2.txt");
    std::fs::write(Path::new(&project.path).join(file_path2), "line5\nline6\n")?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;
    let stack2_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: stack2_id,
            ownership: Some("test2.txt:1-3".parse()?),
            ..Default::default()
        },
    )?;

    let contents = std::fs::read(Path::new(&project.path).join(file_path))?;
    assert_eq!(
        "line1\nline2\nline3\nline4\nbranch1\n",
        String::from_utf8(contents)?
    );
    let contents = std::fs::read(Path::new(&project.path).join(file_path2))?;
    assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();
    assert_eq!(branch.files.len(), 1);
    assert!(branch.active);

    let branch_manager = ctx.branch_manager();
    let real_branch = branch_manager.save_and_unapply(stack1_id, guard.write_permission())?;

    let contents = std::fs::read(Path::new(&project.path).join(file_path))?;
    assert_eq!("line1\nline2\nline3\nline4\n", String::from_utf8(contents)?);
    let contents = std::fs::read(Path::new(&project.path).join(file_path2))?;
    assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    assert!(!branches.iter().any(|b| b.id == stack1_id));

    let branch_manager = ctx.branch_manager();
    let stack1_id = branch_manager.create_virtual_branch_from_branch(
        &Refname::from_str(&real_branch)?,
        None,
        None,
        guard.write_permission(),
    )?;
    let contents = std::fs::read(Path::new(&project.path).join(file_path))?;
    assert_eq!(
        "line1\nline2\nline3\nline4\nbranch1\n",
        String::from_utf8(contents)?
    );
    let contents = std::fs::read(Path::new(&project.path).join(file_path2))?;
    assert_eq!("line5\nline6\n", String::from_utf8(contents)?);

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();
    assert_eq!(branch.files.len(), 1);
    assert!(branch.active);

    Ok(())
}

#[test]
fn apply_unapply_added_deleted_files() -> Result<()> {
    let suite = Suite::default();
    let Case { project, ctx, .. } = &suite.new_case();

    // create a commit and set the target
    let file_path = Path::new("test.txt");
    std::fs::write(Path::new(&project.path).join(file_path), "file1\n")?;
    let file_path2 = Path::new("test2.txt");
    std::fs::write(Path::new(&project.path).join(file_path2), "file2\n")?;
    commit_all(ctx.repo());

    set_test_target(ctx)?;

    // rm file_path2, add file3
    std::fs::remove_file(Path::new(&project.path).join(file_path2))?;
    let file_path3 = Path::new("test3.txt");
    std::fs::write(Path::new(&project.path).join(file_path3), "file3\n")?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack2_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;
    let stack3_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: stack2_id,
            ownership: Some("test2.txt:0-0".parse()?),
            ..Default::default()
        },
    )?;
    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: stack3_id,
            ownership: Some("test3.txt:1-2".parse()?),
            ..Default::default()
        },
    )?;

    internal::list_virtual_branches(ctx, guard.write_permission()).unwrap();

    let branch_manager = ctx.branch_manager();
    let real_branch_2 = branch_manager.save_and_unapply(stack2_id, guard.write_permission())?;

    // check that file2 is back
    let contents = std::fs::read(Path::new(&project.path).join(file_path2))?;
    assert_eq!("file2\n", String::from_utf8(contents)?);

    let real_branch_3 = branch_manager.save_and_unapply(stack3_id, guard.write_permission())?;
    // check that file3 is gone
    assert!(!Path::new(&project.path).join(file_path3).exists());

    branch_manager
        .create_virtual_branch_from_branch(
            &Refname::from_str(&real_branch_2).unwrap(),
            None,
            None,
            guard.write_permission(),
        )
        .unwrap();

    // check that file2 is gone
    assert!(!Path::new(&project.path).join(file_path2).exists());

    branch_manager
        .create_virtual_branch_from_branch(
            &Refname::from_str(&real_branch_3).unwrap(),
            None,
            None,
            guard.write_permission(),
        )
        .unwrap();

    // check that file3 is back
    let contents = std::fs::read(Path::new(&project.path).join(file_path3))?;
    assert_eq!("file3\n", String::from_utf8(contents)?);

    Ok(())
}

// Verifies that we are able to detect when a remote branch is conflicting with the current applied branches.
#[test]
fn detect_mergeable_branch() -> Result<()> {
    let suite = Suite::default();
    let Case { project, ctx, .. } = &suite.new_case();

    // create a commit and set the target
    let file_path = Path::new("test.txt");
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    commit_all(ctx.repo());

    set_test_target(ctx)?;

    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nbranch1\n",
    )?;
    let file_path4 = Path::new("test4.txt");
    std::fs::write(Path::new(&project.path).join(file_path4), "line5\nline6\n")?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;
    let stack2_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: stack2_id,
            ownership: Some("test4.txt:1-3".parse()?),
            ..Default::default()
        },
    )
    .expect("failed to update branch");

    // unapply both branches and create some conflicting ones
    let branch_manager = ctx.branch_manager();
    branch_manager.save_and_unapply(stack1_id, guard.write_permission())?;
    branch_manager.save_and_unapply(stack2_id, guard.write_permission())?;

    ctx.repo().set_head("refs/heads/master")?;
    ctx.repo()
        .checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // create an upstream remote conflicting commit
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nupstream\n",
    )?;
    commit_all(ctx.repo());
    let up_target = ctx.repo().head().unwrap().target().unwrap();
    ctx.repo().reference(
        "refs/remotes/origin/remote_branch",
        up_target,
        true,
        "update target",
    )?;

    // revert content and write a mergeable branch
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\n",
    )?;
    let file_path3 = Path::new("test3.txt");
    std::fs::write(Path::new(&project.path).join(file_path3), "file3\n")?;
    commit_all(ctx.repo());
    let up_target = ctx.repo().head().unwrap().target().unwrap();
    ctx.repo().reference(
        "refs/remotes/origin/remote_branch2",
        up_target,
        true,
        "update target",
    )?;
    // remove file_path3
    std::fs::remove_file(Path::new(&project.path).join(file_path3))?;

    ctx.repo().set_head("refs/heads/gitbutler/workspace")?;
    ctx.repo()
        .checkout_head(Some(&mut git2::build::CheckoutBuilder::default().force()))?;

    // create branches that conflict with our earlier branches
    let branch_manager = ctx.branch_manager();
    branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch");
    let stack4_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    // branch3 conflicts with branch1 and remote_branch
    std::fs::write(
        Path::new(&project.path).join(file_path),
        "line1\nline2\nline3\nline4\nbranch3\n",
    )?;

    // branch4 conflicts with branch2
    let file_path2 = Path::new("test2.txt");
    std::fs::write(
        Path::new(&project.path).join(file_path2),
        "line1\nline2\nline3\nline4\nbranch4\n",
    )?;

    let vb_state = VirtualBranchesHandle::new(project.gb_dir());

    let mut stack4 = vb_state.get_stack_in_workspace(stack4_id)?;
    stack4.ownership = BranchOwnershipClaims {
        claims: vec!["test2.txt:1-6".parse()?],
    };
    vb_state.set_stack(stack4.clone())?;

    assert!(!internal::is_remote_branch_mergeable(
        ctx,
        &"refs/remotes/origin/remote_branch".parse().unwrap()
    )
    .unwrap());

    assert!(internal::is_remote_branch_mergeable(
        ctx,
        &"refs/remotes/origin/remote_branch2".parse().unwrap()
    )
    .unwrap());

    Ok(())
}

#[test]
fn upstream_integrated_vbranch() -> Result<()> {
    // ok, we need a vbranch with some work and an upstream target that also includes that work, but the base is behind
    // plus a branch with work not in upstream so we can see that it is not included in the vbranch

    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case_with_files(HashMap::from([
        (PathBuf::from("test.txt"), "file1\n"),
        (PathBuf::from("test2.txt"), "file2\n"),
        (PathBuf::from("test3.txt"), "file3\n"),
    ]));

    let vb_state = VirtualBranchesHandle::new(project.gb_dir());

    let base_commit = ctx.repo().head().unwrap().target().unwrap();

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "file1\nversion2\n",
    )?;
    commit_all(ctx.repo());

    let upstream_commit = ctx.repo().head().unwrap().target().unwrap();
    ctx.repo().reference(
        "refs/remotes/origin/master",
        upstream_commit,
        true,
        "update target",
    )?;

    vb_state.set_default_target(Target {
        branch: "refs/remotes/origin/master".parse().unwrap(),
        remote_url: "http://origin.com/project".to_string(),
        sha: base_commit,
        push_remote_name: None,
    })?;
    ctx.repo().remote("origin", "http://origin.com/project")?;
    update_workspace_commit(&vb_state, ctx)?;

    // create vbranches, one integrated, one not
    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;
    let stack2_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;
    let stack3_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        Path::new(&project.path).join("test2.txt"),
        "file2\nversion2\n",
    )?;

    std::fs::write(
        Path::new(&project.path).join("test3.txt"),
        "file3\nversion2\n",
    )?;

    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: stack1_id,
            name: Some("integrated".to_string()),
            ownership: Some("test.txt:1-2".parse()?),
            ..Default::default()
        },
    )?;

    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: stack2_id,
            name: Some("not integrated".to_string()),
            ownership: Some("test2.txt:1-2".parse()?),
            ..Default::default()
        },
    )?;

    internal::update_branch(
        ctx,
        &BranchUpdateRequest {
            id: stack3_id,
            name: Some("not committed".to_string()),
            ownership: Some("test3.txt:1-2".parse()?),
            ..Default::default()
        },
    )?;

    // create a new virtual branch from the remote branch
    internal::commit(ctx, stack1_id, "integrated commit", None, false)?;
    internal::commit(ctx, stack2_id, "non-integrated commit", None, false)?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;

    let branch1 = &branches.iter().find(|b| b.id == stack1_id).unwrap();
    assert!(branch1.series[0]
        .clone()?
        .patches
        .iter()
        .any(|c| c.is_integrated));
    assert_eq!(branch1.files.len(), 0);
    assert_eq!(branch1.series[0].clone()?.patches.len(), 1);

    let branch2 = &branches.iter().find(|b| b.id == stack2_id).unwrap();
    assert!(!branch2.series[0]
        .clone()?
        .patches
        .iter()
        .any(|c| c.is_integrated));
    assert_eq!(branch2.files.len(), 0);
    assert_eq!(branch2.series[0].clone()?.patches.len(), 1);

    let branch3 = &branches.iter().find(|b| b.id == stack3_id).unwrap();
    assert!(!branch3.series[0]
        .clone()?
        .patches
        .iter()
        .any(|c| c.is_integrated));
    assert_eq!(branch3.files.len(), 1);
    assert_eq!(branch3.series[0].clone()?.patches.len(), 0);

    Ok(())
}

#[test]
fn commit_same_hunk_twice() -> Result<()> {
    let suite = Suite::default();
    let Case {
        ctx,
        project,
        ..
    } = &suite.new_case_with_files(HashMap::from([(
        PathBuf::from("test.txt"),
        "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
    )]));

    set_test_target(ctx)?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
    )?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 1);
    assert_eq!(branch.series[0].clone()?.patches.len(), 0);

    // commit
    internal::commit(ctx, stack1_id, "first commit to test.txt", None, false)?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    assert_eq!(branch.files.len(), 0, "no files expected");

    assert_eq!(
        branch.series[0].clone()?.patches.len(),
        1,
        "file should have been commited"
    );

    let commit1_files = list_commit_files(project, branch.series[0].clone()?.patches[0].id)?;
    assert_eq!(commit1_files.len(), 1, "hunks expected");
    assert_eq!(
        commit1_files[0].hunks.len(),
        1,
        "one hunk should have been commited"
    );

    // update same lines

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line1\nPATCH1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
    )?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    assert_eq!(branch.files.len(), 1, "one file should be changed");
    assert_eq!(
        branch.series[0].clone()?.patches.len(),
        1,
        "commit is still there"
    );

    internal::commit(ctx, stack1_id, "second commit to test.txt", None, false)?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    assert_eq!(
        branch.files.len(),
        0,
        "all changes should have been commited"
    );

    let commit1_files = list_commit_files(project, branch.series[0].clone()?.patches[0].id)?;
    let commit2_files = list_commit_files(project, branch.series[0].clone()?.patches[1].id)?;
    assert_eq!(
        branch.series[0].clone()?.patches.len(),
        2,
        "two commits expected"
    );
    assert_eq!(commit1_files.len(), 1);
    assert_eq!(commit1_files[0].hunks.len(), 1);
    assert_eq!(commit2_files.len(), 1);
    assert_eq!(commit2_files[0].hunks.len(), 1);

    Ok(())
}

#[test]
fn commit_same_file_twice() -> Result<()> {
    let suite = Suite::default();
    let Case {
        ctx,
        project,
        ..
    } = &suite.new_case_with_files(HashMap::from([(
        PathBuf::from("test.txt"),
        "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
    )]));

    set_test_target(ctx)?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
    )?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 1);
    assert_eq!(branch.series[0].clone()?.patches.len(), 0);

    // commit
    internal::commit(ctx, stack1_id, "first commit to test.txt", None, false)?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    assert_eq!(branch.files.len(), 0, "no files expected");

    let commit1_files = list_commit_files(project, branch.series[0].clone()?.patches[0].id)?;
    assert_eq!(
        branch.series[0].clone()?.patches.len(),
        1,
        "file should have been commited"
    );
    assert_eq!(commit1_files.len(), 1, "hunks expected");
    assert_eq!(
        commit1_files[0].hunks.len(),
        1,
        "one hunk should have been commited"
    );

    // add second patch

    std::fs::write(
        Path::new(&project.path).join("file.txt"),
        "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\npatch2\nline11\nline12\n",
    )?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    assert_eq!(branch.files.len(), 1, "one file should be changed");
    assert_eq!(
        branch.series[0].clone()?.patches.len(),
        1,
        "commit is still there"
    );

    internal::commit(ctx, stack1_id, "second commit to test.txt", None, false)?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    assert_eq!(
        branch.files.len(),
        0,
        "all changes should have been commited"
    );

    let commit1_files = list_commit_files(project, branch.series[0].clone()?.patches[0].id)?;
    let commit2_files = list_commit_files(project, branch.series[0].clone()?.patches[1].id)?;
    assert_eq!(
        branch.series[0].clone()?.patches.len(),
        2,
        "two commits expected"
    );
    assert_eq!(commit1_files.len(), 1);
    assert_eq!(commit1_files[0].hunks.len(), 1);
    assert_eq!(commit2_files.len(), 1);
    assert_eq!(commit2_files[0].hunks.len(), 1);

    Ok(())
}

#[test]
fn commit_partial_by_hunk() -> Result<()> {
    let suite = Suite::default();
    let Case {
        ctx,
        project,
        ..
    } = &suite.new_case_with_files(HashMap::from([(
        PathBuf::from("test.txt"),
        "line1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\nline11\nline12\n",
    )]));

    set_test_target(ctx)?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line1\npatch1\nline2\nline3\nline4\nline5\nmiddle\nmiddle\nmiddle\nmiddle\nline6\nline7\nline8\nline9\nline10\nmiddle\nmiddle\nmiddle\npatch2\nline11\nline12\n",
    )?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 2);
    assert_eq!(branch.series[0].clone()?.patches.len(), 0);

    // commit
    internal::commit(
        ctx,
        stack1_id,
        "first commit to test.txt",
        Some(&"test.txt:1-6".parse::<BranchOwnershipClaims>().unwrap()),
        false,
    )?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();
    let commit1_files = list_commit_files(project, branch.series[0].clone()?.patches[0].id)?;

    assert_eq!(branch.files.len(), 1);
    assert_eq!(branch.files[0].hunks.len(), 1);
    assert_eq!(branch.series[0].clone()?.patches.len(), 1);
    assert_eq!(commit1_files.len(), 1);
    assert_eq!(commit1_files[0].hunks.len(), 1);

    internal::commit(
        ctx,
        stack1_id,
        "second commit to test.txt",
        Some(&"test.txt:16-22".parse::<BranchOwnershipClaims>().unwrap()),
        false,
    )?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch = &branches.iter().find(|b| b.id == stack1_id).unwrap();
    let commit1_files = list_commit_files(project, branch.series[0].clone()?.patches[0].id)?;
    let commit2_files = list_commit_files(project, branch.series[0].clone()?.patches[1].id)?;

    assert_eq!(branch.files.len(), 0);
    assert_eq!(branch.series[0].clone()?.patches.len(), 2);
    assert_eq!(commit1_files.len(), 1);
    assert_eq!(commit1_files[0].hunks.len(), 1);
    assert_eq!(commit2_files.len(), 1);
    assert_eq!(commit2_files[0].hunks.len(), 1);

    Ok(())
}

#[test]
fn commit_partial_by_file() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case_with_files(HashMap::from([
        (PathBuf::from("test.txt"), "file1\n"),
        (PathBuf::from("test2.txt"), "file2\n"),
    ]));

    let commit1_oid = ctx.repo().head().unwrap().target().unwrap();
    let commit1 = ctx.repo().find_commit(commit1_oid).unwrap();

    set_test_target(ctx)?;

    // remove file
    std::fs::remove_file(Path::new(&project.path).join("test2.txt"))?;
    // add new file
    let file_path3 = Path::new("test3.txt");
    std::fs::write(Path::new(&project.path).join(file_path3), "file3\n")?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    // commit
    internal::commit(ctx, stack1_id, "branch1 commit", None, false)?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch1 = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    // branch one test.txt has just the 1st and 3rd hunks applied
    let commit2 = &branch1.series[0].clone()?.patches[0].id;
    let commit2 = ctx
        .repo()
        .find_commit(commit2.to_owned())
        .expect("failed to get commit object");

    let tree = commit1.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(ctx.repo(), &tree);
    assert_eq!(file_list, vec!["test.txt", "test2.txt"]);

    // get the tree
    let tree = commit2.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(ctx.repo(), &tree);
    assert_eq!(file_list, vec!["test.txt", "test3.txt"]);

    Ok(())
}

#[test]
fn commit_add_and_delete_files() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case_with_files(HashMap::from([
        (PathBuf::from("test.txt"), "file1\n"),
        (PathBuf::from("test2.txt"), "file2\n"),
    ]));

    let commit1_oid = ctx.repo().head().unwrap().target().unwrap();
    let commit1 = ctx.repo().find_commit(commit1_oid).unwrap();

    set_test_target(ctx)?;

    // remove file
    std::fs::remove_file(Path::new(&project.path).join("test2.txt"))?;
    // add new file
    let file_path3 = Path::new("test3.txt");
    std::fs::write(Path::new(&project.path).join(file_path3), "file3\n")?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    // commit
    internal::commit(ctx, stack1_id, "branch1 commit", None, false)?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch1 = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    // branch one test.txt has just the 1st and 3rd hunks applied
    let commit2 = &branch1.series[0].clone()?.patches[0].id;
    let commit2 = ctx
        .repo()
        .find_commit(commit2.to_owned())
        .expect("failed to get commit object");

    let tree = commit1.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(ctx.repo(), &tree);
    assert_eq!(file_list, vec!["test.txt", "test2.txt"]);

    // get the tree
    let tree = commit2.tree().expect("failed to get tree");
    let file_list = tree_to_file_list(ctx.repo(), &tree);
    assert_eq!(file_list, vec!["test.txt", "test3.txt"]);

    Ok(())
}

#[test]
#[cfg(target_family = "unix")]
fn commit_executable_and_symlinks() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case_with_files(HashMap::from([
        (PathBuf::from("test.txt"), "file1\n"),
        (PathBuf::from("test2.txt"), "file2\n"),
    ]));

    set_test_target(ctx)?;

    // add symlinked file
    let file_path3 = Path::new("test3.txt");
    let src = Path::new(&project.path).join("test2.txt");
    let dst = Path::new(&project.path).join(file_path3);
    symlink(src, dst)?;

    // add executable
    let file_path4 = Path::new("test4.bin");
    let exec = Path::new(&project.path).join(file_path4);
    std::fs::write(&exec, "exec\n")?;
    let permissions = std::fs::metadata(&exec)?.permissions();
    let new_permissions = Permissions::from_mode(permissions.mode() | 0o111); // Add execute permission
    std::fs::set_permissions(&exec, new_permissions)?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    // commit
    internal::commit(ctx, stack1_id, "branch1 commit", None, false)?;

    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let branches = list_result.branches;
    let branch1 = &branches.iter().find(|b| b.id == stack1_id).unwrap();

    let commit = &branch1.series[0].clone()?.patches[0].id;
    let commit = ctx
        .repo()
        .find_commit(commit.to_owned())
        .expect("failed to get commit object");

    let tree = commit.tree().expect("failed to get tree");

    let list = tree_to_entry_list(ctx.repo(), &tree);
    assert_eq!(list[0].0, "test.txt");
    assert_eq!(list[0].1, "100644");
    assert_eq!(list[1].0, "test2.txt");
    assert_eq!(list[1].1, "100644");
    assert_eq!(list[2].0, "test3.txt");
    assert_eq!(list[2].1, "120000");
    assert_eq!(list[2].2, "test2.txt");
    assert_eq!(list[3].0, "test4.bin");
    assert_eq!(list[3].1, "100755");

    Ok(())
}

fn tree_to_file_list(repository: &git2::Repository, tree: &git2::Tree) -> Vec<String> {
    let mut file_list = Vec::new();
    walk(tree, |_, entry| {
        let path = entry.name().unwrap();
        let entry = tree.get_path(Path::new(path)).unwrap();
        let object = entry.to_object(repository).unwrap();
        if object.kind() == Some(git2::ObjectType::Blob) {
            file_list.push(path.to_string());
        }
        TreeWalkResult::Continue
    })
    .expect("failed to walk tree");
    file_list
}

fn tree_to_entry_list(
    repository: &git2::Repository,
    tree: &git2::Tree,
) -> Vec<(String, String, String, String)> {
    let mut file_list = Vec::new();
    walk(tree, |_root, entry| {
        let path = entry.name().unwrap();
        let entry = tree.get_path(Path::new(path)).unwrap();
        let object = entry.to_object(repository).unwrap();
        let blob = object.as_blob().expect("failed to get blob");
        // convert content to string
        let octal_mode = format!("{:o}", entry.filemode());
        if let Ok(content) =
            std::str::from_utf8(blob.content()).context("failed to convert content to string")
        {
            file_list.push((
                path.to_string(),
                octal_mode,
                content.to_string(),
                blob.id().to_string(),
            ));
        } else {
            file_list.push((
                path.to_string(),
                octal_mode,
                "BINARY".to_string(),
                blob.id().to_string(),
            ));
        }
        TreeWalkResult::Continue
    })
    .expect("failed to walk tree");
    file_list
}

#[test]
fn verify_branch_commits_to_workspace() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case();

    set_test_target(ctx)?;

    let mut guard = project.exclusive_worktree_access();
    verify_branch(ctx, guard.write_permission()).unwrap();

    //  write two commits
    let file_path2 = Path::new("test2.txt");
    std::fs::write(Path::new(&project.path).join(file_path2), "file")?;
    commit_all(ctx.repo());
    std::fs::write(Path::new(&project.path).join(file_path2), "update")?;
    commit_all(ctx.repo());

    // verify puts commits onto the virtual branch
    verify_branch(ctx, guard.write_permission()).unwrap();

    // one virtual branch with two commits was created
    let list_result = internal::list_virtual_branches(ctx, guard.write_permission())?;
    let virtual_branches = list_result.branches;
    assert_eq!(virtual_branches.len(), 1);

    let branch = &virtual_branches.first().unwrap();
    assert_eq!(branch.series[0].clone()?.patches.len(), 2);
    assert_eq!(branch.series[0].clone()?.patches.len(), 2);

    Ok(())
}

#[test]
fn verify_branch_not_workspace() -> Result<()> {
    let suite = Suite::default();
    let Case { ctx, project, .. } = &suite.new_case();

    set_test_target(ctx)?;

    let mut guard = project.exclusive_worktree_access();
    verify_branch(ctx, guard.write_permission()).unwrap();

    ctx.repo().set_head("refs/heads/master")?;

    let verify_result = verify_branch(ctx, guard.write_permission());
    assert!(verify_result.is_err());
    assert_eq!(
        format!("{:#}", verify_result.unwrap_err()),
        "<verification-failed>: project is on refs/heads/master. Please checkout gitbutler/workspace to continue"
    );

    Ok(())
}

#[test]
fn pre_commit_hook_rejection() -> Result<()> {
    let suite = Suite::default();
    let Case { project, ctx, .. } = &suite.new_case_with_files(HashMap::from([
        (PathBuf::from("test.txt"), "line1\nline2\nline3\nline4\n"),
        (PathBuf::from("test2.txt"), "line5\nline6\nline7\nline8\n"),
    ]));

    set_test_target(ctx)?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let hook = b"#!/bin/sh
    echo 'rejected'
    exit 1
            ";

    git2_hooks::create_hook(ctx.repo(), git2_hooks::HOOK_PRE_COMMIT, hook);

    let res = internal::commit(ctx, stack1_id, "test commit", None, true);

    let err = res.unwrap_err();
    assert_eq!(
        err.source().unwrap().to_string(),
        "commit hook rejected: rejected"
    );

    Ok(())
}

#[test]
fn post_commit_hook() -> Result<()> {
    let suite = Suite::default();
    let Case { project, ctx, .. } = &suite.new_case_with_files(HashMap::from([
        (PathBuf::from("test.txt"), "line1\nline2\nline3\nline4\n"),
        (PathBuf::from("test2.txt"), "line5\nline6\nline7\nline8\n"),
    ]));

    set_test_target(ctx)?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let hook = b"#!/bin/sh
    touch hook_ran
            ";

    git2_hooks::create_hook(ctx.repo(), git2_hooks::HOOK_POST_COMMIT, hook);

    let hook_ran_proof = ctx.repo().path().parent().unwrap().join("hook_ran");

    assert!(!hook_ran_proof.exists());

    internal::commit(ctx, stack1_id, "test commit", None, true)?;

    assert!(hook_ran_proof.exists());

    Ok(())
}

#[test]
fn commit_msg_hook_rejection() -> Result<()> {
    let suite = Suite::default();
    let Case { project, ctx, .. } = &suite.new_case_with_files(HashMap::from([
        (PathBuf::from("test.txt"), "line1\nline2\nline3\nline4\n"),
        (PathBuf::from("test2.txt"), "line5\nline6\nline7\nline8\n"),
    ]));

    set_test_target(ctx)?;

    let branch_manager = ctx.branch_manager();
    let mut guard = project.exclusive_worktree_access();
    let stack1_id = branch_manager
        .create_virtual_branch(&BranchCreateRequest::default(), guard.write_permission())
        .expect("failed to create virtual branch")
        .id;

    std::fs::write(
        Path::new(&project.path).join("test.txt"),
        "line0\nline1\nline2\nline3\nline4\n",
    )?;

    let hook = b"#!/bin/sh
    echo 'rejected'
    exit 1
            ";

    git2_hooks::create_hook(ctx.repo(), git2_hooks::HOOK_COMMIT_MSG, hook);

    let res = internal::commit(ctx, stack1_id, "test commit", None, true);

    let err = res.unwrap_err();
    assert_eq!(
        err.source().unwrap().to_string(),
        "commit-msg hook rejected: rejected"
    );

    Ok(())
}

fn walk<C>(tree: &git2::Tree, mut callback: C) -> Result<()>
where
    C: FnMut(&str, &TreeEntry) -> TreeWalkResult,
{
    tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
        match callback(root, &entry.clone()) {
            TreeWalkResult::Continue => git2::TreeWalkResult::Ok,
        }
    })
    .map_err(Into::into)
}

enum TreeWalkResult {
    Continue,
}
