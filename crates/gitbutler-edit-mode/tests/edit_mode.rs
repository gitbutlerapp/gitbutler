use anyhow::{Context as _, Result, anyhow};
use bstr::ByteSlice as _;
use but_core::{
    RefMetadata, RepositoryExt,
    ref_metadata::{StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch},
};
use but_ctx::Context;
use but_meta::{VirtualBranchesTomlMetadata, virtual_branches_legacy_types::Target};
use but_oxidize::OidExt as _;
use but_testsupport::{gix_testtools, open_repo};
use git2::build::CheckoutBuilder;
use gitbutler_edit_mode::commands::{
    abort_and_return_to_workspace, enter_edit_mode, save_and_return_to_workspace,
};
use gitbutler_operating_modes::INTEGRATION_BRANCH_REF;
use tempfile::TempDir;

fn command_ctx(folder: &str) -> Result<(Context, TempDir)> {
    let folder = folder.to_owned();
    let folder_for_post = folder.clone();
    let (tmp, _) = gix_testtools::scripted_fixture_writable_with_args_with_post(
        "edit_mode.sh",
        None::<String>,
        gix_testtools::Creation::Execute,
        2,
        move |fixture| {
            if fixture.is_uninitialized() {
                let repo = open_repo(&fixture.path().join(&folder_for_post))?;
                seed_metadata(&repo)?;
            }
            Ok(())
        },
    )
    .map_err(anyhow::Error::from_boxed)?;
    let repo = open_repo(tmp.path().join(folder).as_path())?;
    Ok((Context::from_repo(repo)?, tmp))
}

fn seed_metadata(repo: &gix::Repository) -> Result<()> {
    let mut meta = VirtualBranchesTomlMetadata::from_path(
        repo.gitbutler_storage_path()?.join("virtual_branches.toml"),
    )?;
    let mut ws = meta.workspace("refs/heads/gitbutler/workspace".try_into()?)?;
    ws.stacks.clear();
    ws.stacks.push(WorkspaceStack {
        id: StackId::from_number_for_testing(1),
        branches: vec![WorkspaceStackBranch {
            ref_name: "refs/heads/branchy".try_into()?,
            archived: false,
        }],
        workspacecommit_relation: WorkspaceCommitRelation::Merged,
    });
    let target = Target {
        branch: "refs/remotes/origin/main".parse()?,
        remote_url: ".".to_owned(),
        sha: repo.rev_parse_single("refs/remotes/origin/main")?.detach(),
        push_remote_name: Some("origin".to_owned()),
    };
    meta.set_workspace(&ws)?;
    meta.data_mut().default_target = Some(target);
    meta.set_changed_to_necessitate_write();
    meta.write_unreconciled()?;
    Ok(())
}

fn stack_id(ctx: &Context) -> Result<StackId> {
    let guard = ctx.shared_worktree_access();
    let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    ws.stacks
        .first()
        .context("expected workspace stack")?
        .id
        .context("expected workspace stack id")
}

#[test]
fn basic_leaving_edit_mode() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.git2_repo.get()?;

    let foobar = repo.head()?.peel_to_commit()?.parent(0)?.id();

    let worktree_dir = repo.path().parent().unwrap().to_path_buf();
    drop(repo);
    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, foobar.to_gix(), stack_id)?;

    std::fs::write(worktree_dir.join("file"), "edited during edit mode\n")?;
    std::fs::write(worktree_dir.join("newfile"), "created during edit mode\n")?;

    save_and_return_to_workspace(&mut ctx)?;

    let repo = ctx.repo.get()?;
    let blob = repo.rev_parse_single(b"HEAD^{/foobar}:file")?.object()?;
    insta::assert_snapshot!(blob.data.as_bstr(), @"edited during edit mode");
    let blob = repo.rev_parse_single(b"HEAD^{/foobar}:newfile")?.object()?;
    insta::assert_snapshot!(blob.data.as_bstr(), @"created during edit mode");

    Ok(())
}

// Fixture:
// * xxx (HEAD -> gitbutler/workspace) GitButler Workspace Commit
// * xxx foobar
// | * 1e2a3a8 (right) right
// |/
// | * f3d2634 (left) left
// |/
// * 7950f06 (origin/main, origin/HEAD, main) init
// Where "left" and "right" contain changes which conflict with each other
#[test]
fn conficted_entries_get_written_when_leaving_edit_mode() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.git2_repo.get()?;

    let foobar = repo.head()?.peel_to_commit()?.parent(0)?.id();

    drop(repo);
    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, foobar.to_gix(), stack_id)?;

    let repo = ctx.git2_repo.get()?;
    let init = repo.find_reference("refs/heads/main")?.peel_to_commit()?;
    let left = repo.find_reference("refs/heads/left")?.peel_to_commit()?;
    let right = repo.find_reference("refs/heads/right")?.peel_to_commit()?;

    let mut merge = repo.merge_trees(
        &init.tree()?,
        &left.tree()?,
        &right.tree()?,
        Default::default(),
    )?;

    repo.checkout_index(
        Some(&mut merge),
        Some(
            CheckoutBuilder::new()
                .force()
                .remove_untracked(true)
                .conflict_style_diff3(true),
        ),
    )?;

    drop((init, left, right));
    drop(repo);
    save_and_return_to_workspace(&mut ctx)?;

    let repo = ctx.git2_repo.get()?;
    insta::assert_snapshot!(
        std::fs::read_to_string(repo.path().parent().unwrap().join("conflict"))?,
        @"
    <<<<<<< ours
    left
    |||||||
    =======
    right
    >>>>>>> theirs
    "
    );

    Ok(())
}

#[test]
fn abort_requires_force_when_changes_were_made() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let repo = ctx.git2_repo.get()?;
    let foobar = repo.head()?.peel_to_commit()?.parent(0)?.id();
    drop(repo);

    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, foobar.to_gix(), stack_id)?;

    let repo = ctx.git2_repo.get()?;
    insta::assert_debug_snapshot!(
        repo.head()?.name(),
        @r#"
    Some(
        "refs/heads/gitbutler/edit",
    )
    "#
    );
    let worktree_dir = repo.path().parent().unwrap().to_path_buf();
    drop(repo);

    std::fs::write(worktree_dir.join("file"), "edited during edit mode\n")?;

    let result = abort_and_return_to_workspace(&mut ctx, false);
    insta::assert_debug_snapshot!(result.as_ref().map(|_| ()).is_err(), @"true");
    let err = result
        .err()
        .ok_or_else(|| anyhow!("expected forced abort to fail without --force"))?;
    insta::assert_snapshot!(
        err,
        @"
    The working tree differs from the original commit. A forced abort is necessary.
    If you are seeing this message, please report it as a bug. The UI should have prevented this line getting hit.
    "
    );
    insta::assert_debug_snapshot!(
        ctx.git2_repo.get()?.head()?.name(),
        @r#"
    Some(
        "refs/heads/gitbutler/edit",
    )
    "#
    );

    abort_and_return_to_workspace(&mut ctx, true)?;
    insta::assert_debug_snapshot!(
        ctx.git2_repo.get()?.head()?.name(),
        @r#"
    Some(
        "refs/heads/gitbutler/workspace",
    )
    "#
    );

    Ok(())
}

#[test]
fn enter_edit_mode_checks_out_conflicted_commit() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("enter_edit_mode_with_conflicted_commit")?;
    let repo = ctx.git2_repo.get()?;
    let conflicted_commit = repo
        .find_reference("refs/tags/conflicted-target")?
        .peel_to_commit()?
        .id();

    drop(repo);

    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, conflicted_commit.to_gix(), stack_id)?;

    let repo = ctx.git2_repo.get()?;
    insta::assert_debug_snapshot!(
        repo.head()?.name(),
        @r#"
    Some(
        "refs/heads/gitbutler/edit",
    )
    "#
    );
    insta::assert_debug_snapshot!(
        repo.head()?.peel_to_commit()?.summary(),
        @r#"
    Some(
        "foobar",
    )
    "#
    );

    insta::assert_snapshot!(
        std::fs::read_to_string(repo.path().parent().unwrap().join("conflict"))?,
        @"
    <<<<<<< New base: foobar
    left
    ||||||| Common ancestor
    base
    =======
    right
    >>>>>>> Current commit: Changes to make millions
    "
    );
    drop(repo);

    abort_and_return_to_workspace(&mut ctx, true)?;
    insta::assert_debug_snapshot!(
        ctx.git2_repo.get()?.head()?.name(),
        @r#"
    Some(
        "refs/heads/gitbutler/workspace",
    )
    "#
    );

    Ok(())
}

#[test]
fn enter_edit_mode_works_with_only_integration_ref_present() -> Result<()> {
    let (mut ctx, _tempdir) = command_ctx("conficted_entries_get_written_when_leaving_edit_mode")?;
    let foobar = {
        let repo = ctx.git2_repo.get()?;
        let workspace_head = repo.head()?.peel_to_commit()?;
        let foobar = workspace_head.parent(0)?.id();
        repo.reference(INTEGRATION_BRANCH_REF, workspace_head.id(), true, "")?;
        repo.set_head(INTEGRATION_BRANCH_REF)?;
        repo.find_reference("refs/heads/gitbutler/workspace")?
            .delete()
            .context("expected workspace ref to exist")?;
        foobar
    };

    let stack_id = stack_id(&ctx)?;
    enter_edit_mode(&mut ctx, foobar.to_gix(), stack_id)?;

    insta::assert_debug_snapshot!(
        ctx.git2_repo.get()?.head()?.name(),
        @r#"
    Some(
        "refs/heads/gitbutler/edit",
    )
    "#
    );

    Ok(())
}
