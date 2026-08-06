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
use but_meta::{
    VirtualBranchesTomlMetadata, legacy_storage, virtual_branches_legacy_types as legacy_types,
};
use but_testsupport::{gix_testtools, open_repo};
use gitbutler_stack::Stack;
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
    let head = ctx
        .repo
        .get()?
        .rev_parse_single("refs/heads/virtual~2")?
        .detach();
    let result = Stack::new_empty(&ctx, "name with spaces".into(), head, 0);
    assert_eq!(
        result.err().unwrap().to_string(),
        "Reference name contains invalid byte: \" \""
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
