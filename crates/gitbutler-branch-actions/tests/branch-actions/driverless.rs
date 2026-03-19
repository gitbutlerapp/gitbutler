use anyhow::Result;
use but_core::{
    GitConfigSettings, RefMetadata, RepositoryExt,
    ref_metadata::{StackId, WorkspaceCommitRelation, WorkspaceStack, WorkspaceStackBranch},
};
use but_ctx::Context;
use but_meta::VirtualBranchesTomlMetadata;
use but_testsupport::{gix_testtools, open_repo};
use gitbutler_stack::{Target, VirtualBranchesHandle};
use tempfile::TempDir;

#[derive(Clone, Copy)]
struct StackSpec<'a> {
    id: u128,
    branches_base_to_top: &'a [&'a str],
    in_workspace: bool,
}

pub fn writable_context(script_name: &str, repo_name: &str) -> Result<(Context, TempDir)> {
    let script_name = script_name.to_owned();
    let repo_name = repo_name.to_owned();
    let repo_name_for_post = repo_name.clone();
    let script_name_for_post = script_name.clone();
    let (tmp, _) = gix_testtools::scripted_fixture_writable_with_args_with_post(
        script_name.clone(),
        None::<String>,
        if script_name == "reorder.sh" {
            gix_testtools::Creation::Execute
        } else {
            gix_testtools::Creation::CopyFromReadOnly
        },
        4,
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
    Ok((Context::from_repo(repo)?, tmp))
}

pub fn read_only_context(script_name: &str, repo_name: &str) -> Result<Context> {
    let script_name = script_name.to_owned();
    let repo_name = repo_name.to_owned();
    let repo_name_for_post = repo_name.clone();
    let script_name_for_post = script_name.clone();
    let (root, _) = gix_testtools::scripted_fixture_read_only_with_post(
        script_name.clone(),
        4,
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
    let repo = open_repo(root.join(repo_name).as_path())?;
    Ok(Context::from_repo(repo)?.with_memory_cache())
}

fn seed_fixture(repo: &gix::Repository, script_name: &str, repo_name: &str) -> Result<()> {
    disable_gitbutler_commit_signing(repo)?;

    let stacks = match (script_name, repo_name) {
        ("reorder.sh", "multiple-commits") => vec![
            StackSpec {
                id: 1,
                branches_base_to_top: &["other_stack"],
                in_workspace: true,
            },
            StackSpec {
                id: 2,
                branches_base_to_top: &["my_stack", "top-series"],
                in_workspace: true,
            },
        ],
        ("reorder.sh", "multiple-commits-small") => vec![
            StackSpec {
                id: 1,
                branches_base_to_top: &["other_stack"],
                in_workspace: true,
            },
            StackSpec {
                id: 2,
                branches_base_to_top: &["my_stack", "top-series"],
                in_workspace: true,
            },
        ],
        ("reorder.sh", "multiple-commits-empty-top") => vec![
            StackSpec {
                id: 1,
                branches_base_to_top: &["other_stack"],
                in_workspace: true,
            },
            StackSpec {
                id: 2,
                branches_base_to_top: &["my_stack", "top-series"],
                in_workspace: true,
            },
        ],
        ("reorder.sh", "overlapping-commits") => vec![
            StackSpec {
                id: 1,
                branches_base_to_top: &["other_stack"],
                in_workspace: true,
            },
            StackSpec {
                id: 2,
                branches_base_to_top: &["my_stack", "top-series"],
                in_workspace: true,
            },
        ],
        ("squash.sh", "multiple-commits") => vec![StackSpec {
            id: 1,
            branches_base_to_top: &["my_stack", "a-branch-2", "a-branch-3"],
            in_workspace: true,
        }],
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
        unsupported => anyhow::bail!("unsupported driverless fixture {unsupported:?}"),
    };

    write_workspace_metadata(repo, &stacks)?;
    Ok(())
}

fn disable_gitbutler_commit_signing(repo: &gix::Repository) -> Result<()> {
    repo.set_git_settings(&GitConfigSettings {
        gitbutler_sign_commits: Some(false),
        ..Default::default()
    })
}

fn write_workspace_metadata(repo: &gix::Repository, stacks: &[StackSpec<'_>]) -> Result<()> {
    let mut meta = VirtualBranchesTomlMetadata::from_path(
        repo.gitbutler_storage_path()?.join("virtual_branches.toml"),
    )?;
    let workspace_ref = gix::refs::FullName::try_from("refs/heads/gitbutler/workspace")?;
    let mut workspace = meta.workspace(workspace_ref.as_ref())?;
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
    meta.set_workspace(&workspace)?;
    meta.set_changed_to_necessitate_write();
    meta.write_unreconciled()?;

    VirtualBranchesHandle::new(repo.gitbutler_storage_path()?).set_default_target(Target {
        branch: "refs/remotes/origin/main".parse()?,
        remote_url: ".".to_owned(),
        sha: repo.rev_parse_single("refs/remotes/origin/main")?.detach(),
        push_remote_name: Some("origin".to_owned()),
    })?;
    Ok(())
}
