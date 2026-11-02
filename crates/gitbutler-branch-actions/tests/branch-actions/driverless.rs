use anyhow::Result;
use but_core::{
    GitConfigSettings, RepositoryExt,
    ref_metadata::{
        ProjectMeta, StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch,
    },
};
use but_ctx::Context;
use but_testsupport::{gix_testtools, open_repo};
use tempfile::TempDir;

#[derive(Clone, Copy)]
struct StackSpec<'a> {
    id: u128,
    branches_base_to_top: &'a [&'a str],
    in_workspace: bool,
}

#[test]
fn read_only_contexts_have_independent_metadata() -> Result<()> {
    let first = read_only_context("for-listing.sh", "one-vbranch-in-workspace")?;
    let workspace_ref = gix::refs::FullName::try_from("refs/heads/gitbutler/workspace")?;
    {
        let mut db = first.db.get_cache_mut()?;
        let mut workspace = db.meta().workspace(workspace_ref.as_ref())?;
        workspace.stacks.clear();
        db.meta_mut()?.set_workspace(&workspace)?;
    }

    let second = read_only_context("for-listing.sh", "one-vbranch-in-workspace")?;
    let stacks = second
        .db
        .get_cache()?
        .meta()
        .workspace(workspace_ref.as_ref())?
        .stacks
        .len();
    assert_eq!(
        stacks, 1,
        "each context starts with its own seeded metadata"
    );
    Ok(())
}

pub fn writable_context(script_name: &str, repo_name: &str) -> Result<(Context, TempDir)> {
    let script_name = script_name.to_owned();
    let repo_name = repo_name.to_owned();
    let repo_name_for_post = repo_name.clone();
    let script_name_for_post = script_name.clone();
    let (tmp, _) = gix_testtools::scripted_fixture_writable_with_args_with_post(
        script_name.clone(),
        None::<String>,
        if script_name == "workspace-commit.sh" || script_name == "for-listing.sh" {
            gix_testtools::Creation::Execute
        } else {
            gix_testtools::Creation::CopyFromReadOnly
        },
        7,
        move |fixture| {
            if fixture.is_uninitialized() {
                let repo = open_repo(&fixture.path().join(&repo_name_for_post))?;
                seed_fixture(&repo, &script_name_for_post, &repo_name_for_post)?;
            }
            Ok(())
        },
    )
    .map_err(anyhow::Error::from_boxed)?;
    let repo = open_repo(tmp.path().join(repo_name).as_path())?;
    Ok((Context::from_repo_for_testing(repo)?, tmp))
}

pub fn read_only_context(script_name: &str, repo_name: &str) -> Result<Context> {
    let script_name = script_name.to_owned();
    let repo_name = repo_name.to_owned();
    let repo_name_for_post = repo_name.clone();
    let script_name_for_post = script_name.clone();
    let (root, _) = gix_testtools::scripted_fixture_read_only_with_post(
        script_name.clone(),
        6,
        move |fixture| {
            if fixture.is_uninitialized() {
                if script_name_for_post == "for-listing.sh" {
                    for repo_name in [
                        "one-vbranch-in-workspace",
                        "one-vbranch-in-workspace-one-commit",
                    ] {
                        let repo = open_repo(&fixture.path().join(repo_name))?;
                        seed_fixture(&repo, &script_name_for_post, repo_name)?;
                    }
                } else {
                    let repo = open_repo(&fixture.path().join(&repo_name_for_post))?;
                    seed_fixture(&repo, &script_name_for_post, &repo_name_for_post)?;
                }
            }
            Ok(())
        },
    )
    .map_err(anyhow::Error::from_boxed)?;
    let repo = open_repo(root.join(&repo_name).as_path())?;
    let mut ctx = Context::from_repo_for_testing(repo)?;
    ctx.db = but_utils::OnDemandCache::new(but_testsupport::in_memory_db);
    {
        let mut db = ctx.db.get_cache_mut()?;
        write_workspace_metadata(&mut db, &fixture_stacks(&script_name, &repo_name)?)?;
    }
    Ok(ctx)
}

fn seed_fixture(repo: &gix::Repository, script_name: &str, repo_name: &str) -> Result<()> {
    disable_gitbutler_commit_signing(repo)?;

    let mut db = but_testsupport::project_db(repo)?;
    write_workspace_metadata(&mut db, &fixture_stacks(script_name, repo_name)?)?;
    ProjectMeta {
        target_ref: Some(but_core::ref_metadata::LocalRefName::new(
            "refs/heads/main".try_into()?,
        )?),
        target_commit_id: Some(repo.rev_parse_single("refs/remotes/origin/main")?.detach()),
        push_remote: Some("origin".to_owned()),
    }
    .persist(repo)?;
    Ok(())
}

fn fixture_stacks(script_name: &str, repo_name: &str) -> Result<Vec<StackSpec<'static>>> {
    Ok(match (script_name, repo_name) {
        ("for-workspace-migration.sh", "workspace-migration") => vec![StackSpec {
            id: 1,
            branches_base_to_top: &["virtual"],
            in_workspace: true,
        }],
        ("for-listing.sh", "one-vbranch-in-workspace") => vec![StackSpec {
            id: 1,
            branches_base_to_top: &["virtual"],
            in_workspace: true,
        }],
        ("for-listing.sh", "one-vbranch-in-workspace-one-commit") => vec![StackSpec {
            id: 1,
            branches_base_to_top: &["virtual"],
            in_workspace: true,
        }],
        ("for-details.sh", "complex-repo") => vec![StackSpec {
            id: 1,
            branches_base_to_top: &["a-branch-1"],
            in_workspace: true,
        }],
        ("workspace-commit.sh", "conflicting-stacks") => vec![
            StackSpec {
                id: 1,
                branches_base_to_top: &["stack_a"],
                in_workspace: true,
            },
            StackSpec {
                id: 2,
                branches_base_to_top: &["stack_b"],
                in_workspace: true,
            },
        ],
        ("workspace-commit.sh", "adjacent-stacks") => vec![
            StackSpec {
                id: 1,
                branches_base_to_top: &["stack_a"],
                in_workspace: true,
            },
            StackSpec {
                id: 2,
                branches_base_to_top: &["stack_b"],
                in_workspace: true,
            },
        ],
        ("workspace-commit.sh", "diverged-stacks") => vec![
            StackSpec {
                id: 1,
                branches_base_to_top: &["stack_c"],
                in_workspace: true,
            },
            StackSpec {
                id: 2,
                branches_base_to_top: &["stack_d"],
                in_workspace: true,
            },
        ],
        unsupported => anyhow::bail!("unsupported driverless fixture {unsupported:?}"),
    })
}

fn disable_gitbutler_commit_signing(repo: &gix::Repository) -> Result<()> {
    repo.set_git_settings(&GitConfigSettings {
        gitbutler_sign_commits: Some(false),
        ..Default::default()
    })
}

fn write_workspace_metadata(db: &mut but_db::DbHandle, stacks: &[StackSpec<'_>]) -> Result<()> {
    let workspace_ref = gix::refs::FullName::try_from("refs/heads/gitbutler/workspace")?;
    let mut workspace = db.meta().workspace(workspace_ref.as_ref())?;
    workspace.stacks = stacks
        .iter()
        .map(|stack| {
            Ok(WorkspaceStack {
                id: StackId::from_number_for_testing(stack.id),
                workspacecommit_relation: if stack.in_workspace {
                    WorkspaceCommitRelation::Merged
                } else {
                    WorkspaceCommitRelation::Outside
                },
                branches: stack
                    .branches_base_to_top
                    .iter()
                    .rev()
                    .map(|name| {
                        Ok(WorkspaceStackBranch {
                            ref_name: gix::refs::FullName::try_from(format!("refs/heads/{name}"))?,
                            archived: false,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    db.meta_mut()?.set_workspace(&workspace)?;

    Ok(())
}
