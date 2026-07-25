#![expect(
    deprecated,
    reason = "VirtualBranchesHandle should be replaced with ctx.workspace_* helpers"
)]

use std::{fs, path::Path};

use anyhow::{Context as _, Result, bail};
use but_core::{
    RefMetadata, RepositoryExt,
    ref_metadata::{
        ProjectMeta, StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
    },
};
use but_ctx::Context;
use but_db::DbHandle;
use but_error::{AnyhowContextExt as _, Code};
use but_meta::{
    VirtualBranchesTomlMetadata, legacy_storage, virtual_branches_legacy_types as legacy_types,
};
use but_testsupport::{gix_testtools, open_repo};
use gitbutler_git::GitContextExt as _;
use gitbutler_reference::RemoteRefname;
use gitbutler_stack::{Stack, VirtualBranchesHandle};
use gix::refs::transaction::PreviousValue;
use tempfile::TempDir;

#[ctor::ctor]
fn init() {
    // These tests do not function with the askpass broker enabled
    but_askpass::disable();
}

#[test]
fn stack_branch_invalid_name_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let result = Stack::new_empty(&ctx, "name with spaces".into(), test_ctx.commits[0], 0);
    assert_eq!(
        result.err().unwrap().to_string(),
        "Reference name contains invalid byte: \" \""
    );
    Ok(())
}

#[test]
fn update_branch_name_fails_validation() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let result = test_ctx
        .stack
        .rename_branch(&ctx, "virtual".into(), "invalid name".into());
    assert_eq!(result.err().unwrap().to_string(), "Invalid branch name");
    Ok(())
}

#[test]
fn update_branch_name_to_existing_series_fails_atomically() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let stack_before = test_ctx.stack.clone();
    let refs_before = {
        let repo = ctx.repo.get()?;
        (
            repo.find_reference("refs/heads/virtual")?
                .peel_to_id()?
                .detach(),
            repo.find_reference("refs/heads/first_branch")?
                .peel_to_id()?
                .detach(),
        )
    };

    let err = test_ctx
        .stack
        .rename_branch(&ctx, "virtual".into(), "first_branch".into())
        .unwrap_err();

    assert_eq!(
        err.custom_context().map(|context| context.code),
        Some(Code::PreconditionFailed)
    );
    assert_eq!(test_ctx.stack, stack_before);
    assert_eq!(test_ctx.handle.get_stack(test_ctx.stack.id)?, stack_before);
    let repo = ctx.repo.get()?;
    assert_eq!(
        repo.find_reference("refs/heads/virtual")?
            .peel_to_id()?
            .detach(),
        refs_before.0
    );
    assert_eq!(
        repo.find_reference("refs/heads/first_branch")?
            .peel_to_id()?
            .detach(),
        refs_before.1
    );
    Ok(())
}

#[test]
fn update_branch_name_missing_series_fails() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let expected = format!(
        "Series does-not-exist does not exist on stack {}",
        test_ctx.stack.name()
    );
    let result = test_ctx
        .stack
        .rename_branch(&ctx, "does-not-exist".into(), "new-name".into());
    assert_eq!(
        result.unwrap_err().to_string(),
        expected,
        "a missing series must not report a successful rename"
    );
    Ok(())
}

#[test]
fn update_branch_name_to_same_name_is_noop() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let branch_name = String::from("virtual");

    let result = test_ctx
        .stack
        .rename_branch(&ctx, branch_name.clone(), branch_name.clone());

    assert!(result.is_ok());
    assert_eq!(test_ctx.stack.heads[0].name(), &branch_name);

    Ok(())
}

#[test]
fn update_branch_name_success() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let result = test_ctx
        .stack
        .rename_branch(&ctx, "virtual".into(), "new-name".into());
    assert!(result.is_ok());
    assert_eq!(test_ctx.stack.heads[0].name(), "new-name");
    // Assert persisted
    assert_eq!(
        test_ctx.stack,
        test_ctx.handle.get_stack(test_ctx.stack.id)?
    );
    Ok(())
}

#[test]
fn update_name_after_push() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let head = test_ctx.stack.head_oid(&ctx)?;
    ctx.push(
        head,
        RemoteRefname::new("origin", "virtual"),
        false,
        false,
        None,
        Some(Some(test_ctx.stack.id)),
        vec![],
    )?;
    test_ctx
        .stack
        .rename_branch(&ctx, "virtual".into(), "new-name".into())?;

    assert_eq!(test_ctx.stack.heads[0].name(), "new-name");
    let repo = ctx.repo.get()?;
    assert!(repo.try_find_reference("refs/heads/virtual")?.is_none());
    assert_eq!(
        repo.find_reference("refs/heads/new-name")?
            .peel_to_id()?
            .detach(),
        head
    );
    Ok(())
}

#[test]
fn list_series_default_head() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let test_ctx = test_ctx(&ctx)?;
    let branches = test_ctx.stack.branches();
    // the number of series matches the number of heads
    assert_eq!(branches.len(), test_ctx.stack.heads.len());
    assert_eq!(branches[0].name(), "virtual");
    Ok(())
}

#[test]
fn list_series_two_heads_same_commit() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let head_before = Stack::new_empty(
        &ctx,
        "head_before".into(),
        *test_ctx.commits.last().unwrap(),
        0,
    )?
    .branches()
    .remove(0);
    test_ctx.stack.heads.insert(0, head_before);

    let branches = test_ctx.stack.branches();

    // the number of series matches the number of heads
    assert_eq!(branches.len(), test_ctx.stack.heads.len());

    assert_eq!(branches[0].name(), "head_before");
    assert_eq!(branches[1].name(), "virtual");
    Ok(())
}

#[test]
fn list_series_two_heads_different_commit() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let head_before = Stack::new_empty(
        &ctx,
        "head_before".into(),
        *test_ctx.commits.first().unwrap(),
        0,
    )?
    .branches()
    .remove(0);

    test_ctx.stack.heads.insert(0, head_before);
    let branches = test_ctx.stack.branches();
    // the number of series matches the number of heads
    assert_eq!(branches.len(), test_ctx.stack.heads.len());
    assert_eq!(branches[0].name(), "head_before");
    assert_eq!(branches[1].name(), "virtual");

    Ok(())
}

#[test]
fn set_stack_head_commit_invalid() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let mut vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let repo = ctx.repo.get()?;
    let result = test_ctx
        .stack
        .set_stack_head(&mut vb_state, &repo, repo.object_hash().null());
    assert!(result.is_err());
    Ok(())
}

#[test]
fn set_stack_head() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut test_ctx = test_ctx(&ctx)?;
    let commit = *test_ctx.other_commits.last().unwrap();
    let mut vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    let repo = ctx.repo.get()?;
    let result = test_ctx.stack.set_stack_head(&mut vb_state, &repo, commit);
    assert!(result.is_ok());
    let branches = test_ctx.stack.branches();
    assert_eq!(
        commit,
        branches.first().unwrap().head_oid(&*ctx.repo.get()?)?
    );
    assert_eq!(
        test_ctx.stack.head_oid(&ctx)?,
        *test_ctx.other_commits.last().unwrap()
    );
    Ok(())
}

fn command_ctx(name: &str) -> Result<(Context, TempDir)> {
    let name = name.to_owned();
    let name_for_post = name.clone();
    let (tmp, _) = gix_testtools::scripted_fixture_writable_with_args_with_post(
        "stacking.sh",
        None::<String>,
        gix_testtools::Creation::CopyFromReadOnly,
        2,
        move |fixture| {
            if fixture.is_uninitialized() {
                let repo = open_repo(&fixture.path().join(&name_for_post))?;
                seed_metadata(&repo, &name_for_post)?;
            }
            Ok(())
        },
    )
    .map_err(anyhow::Error::from_boxed)?;
    let repo = open_repo(tmp.path().join(name).as_path())?;
    let ctx = Context::from_repo_for_testing(repo)?;
    ctx.set_project_meta(ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(
            ctx.repo
                .get()?
                .rev_parse_single("refs/remotes/origin/main")?
                .detach(),
        ),
        push_remote: Some("origin".into()),
    })?;
    Ok((ctx, tmp))
}

fn seed_metadata(repo: &gix::Repository, name: &str) -> Result<()> {
    if name != "multiple-commits" {
        bail!("unsupported driverless stacking fixture: {name}");
    }

    let mut meta = VirtualBranchesTomlMetadata::from_path(
        repo.gitbutler_storage_path()?.join("virtual_branches.toml"),
    )?;
    let mut ws = meta.workspace("refs/heads/gitbutler/workspace".try_into()?)?;
    ws.stacks.clear();
    ws.stacks.push(WorkspaceStack {
        id: StackId::from_number_for_testing(1),
        branches: vec![WorkspaceStackBranch {
            ref_name: "refs/heads/first_branch".try_into()?,
            archived: false,
        }],
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
    });
    ws.stacks.push(WorkspaceStack {
        id: StackId::from_number_for_testing(2),
        branches: vec![WorkspaceStackBranch {
            ref_name: "refs/heads/virtual".try_into()?,
            archived: false,
        }],
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
    });
    meta.set_workspace(&ws)?;
    meta.set_changed_to_necessitate_write();
    meta.write_unreconciled()?;
    Ok(())
}

fn test_ctx(ctx: &Context) -> Result<TestContext> {
    let handle = VirtualBranchesHandle::new(ctx.project_data_dir());
    let stacks = handle.list_stacks_in_workspace()?;
    let stack = stacks.iter().find(|b| b.name() == "virtual").unwrap();
    let repo = ctx.repo.get()?;
    Ok(TestContext {
        stack: stack.clone(),
        commits: vec![
            repo.rev_parse_single("refs/heads/virtual~2")?.detach(),
            repo.rev_parse_single("refs/heads/virtual~1")?.detach(),
            repo.rev_parse_single("refs/heads/virtual")?.detach(),
        ],
        other_commits: vec![repo.rev_parse_single("refs/heads/first_branch")?.detach()],
        handle,
    })
}

struct TestContext {
    stack: Stack,
    /// Oldest commit first
    commits: Vec<gix::ObjectId>,
    /// Oldest commit first
    other_commits: Vec<gix::ObjectId>,
    handle: VirtualBranchesHandle,
}

#[test]
fn next_order_index_normalizes_only_workspace_stacks() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let mut handle = VirtualBranchesHandle::new(ctx.project_data_dir());
    let first_id = StackId::from_number_for_testing(1);
    let second_id = StackId::from_number_for_testing(2);

    let mut first = handle.get_stack(first_id)?;
    first.order = 7;
    handle.set_stack(first.clone())?;

    let mut second = handle.get_stack(second_id)?;
    second.order = 3;
    handle.set_stack(second)?;

    let mut outside = first;
    outside.id = StackId::from_number_for_testing(3);
    outside.order = 99;
    outside.in_workspace = false;
    handle.set_stack(outside.clone())?;

    assert_eq!(
        handle.next_order_index()?,
        2,
        "next order follows the two active stacks"
    );
    assert_eq!(
        handle.get_stack(second_id)?.order,
        0,
        "lower active order is normalized first"
    );
    assert_eq!(
        handle.get_stack(first_id)?.order,
        1,
        "higher active order is normalized second"
    );
    assert_eq!(
        handle.get_stack(outside.id)?.order,
        99,
        "out-of-workspace order remains untouched"
    );

    Ok(())
}

#[test]
fn next_available_name_avoids_remote_tracking_branches() -> Result<()> {
    let (ctx, _temp_dir) = command_ctx("multiple-commits")?;
    let repo = ctx.repo.get()?;

    let head = repo.rev_parse_single("refs/heads/virtual")?.detach();
    let remote_branch = "refs/remotes/origin/my-test-branch";
    repo.reference(remote_branch, head, PreviousValue::Any, "test")?;
    drop(repo);

    let stack = Stack::new_empty(&ctx, "my-test-branch".to_owned(), head, 0)?;

    assert_eq!(stack.derived_name()?, "my-test-branch-1");

    Ok(())
}

#[test]
fn storage_sync_bootstraps_db_from_existing_toml() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let toml_path = tmp.path().join("virtual_branches.toml");
    write_legacy_toml(&toml_path, &legacy_types::VirtualBranches::default())?;

    let _state = read_virtual_branches(tmp.path())?;
    assert!(toml_path.exists(), "the TOML mirror stays available");

    let db = DbHandle::new_in_directory(tmp.path())?;
    let snapshot = db
        .virtual_branches()
        .get_snapshot()?
        .context("expected DB snapshot after bootstrap")?;
    assert!(snapshot.state.initialized, "TOML bootstrap initializes DB");
    Ok(())
}

#[test]
fn storage_sync_recreates_toml_when_missing() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let _ = read_virtual_branches(tmp.path())?;
    let toml_path = tmp.path().join("virtual_branches.toml");
    assert!(toml_path.exists(), "initial sync creates TOML");

    fs::remove_file(&toml_path)?;
    assert!(!toml_path.exists(), "sanity check: TOML was removed");

    let _ = read_virtual_branches(tmp.path())?;
    assert!(toml_path.exists(), "missing TOML is recreated from DB");
    Ok(())
}

fn read_virtual_branches(base_path: impl AsRef<Path>) -> Result<legacy_types::VirtualBranches> {
    legacy_storage::read_synced_virtual_branches(&base_path.as_ref().join("virtual_branches.toml"))
}

fn write_legacy_toml(path: &Path, data: &legacy_types::VirtualBranches) -> Result<()> {
    fs::write(path, toml::to_string(data)?)?;
    Ok(())
}
