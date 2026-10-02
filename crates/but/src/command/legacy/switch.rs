use std::time::SystemTime;

use anyhow::Context as _;
use bstr::ByteSlice as _;
use but_core::{
    WORKSPACE_REF_NAME,
    sync::{RepoExclusive, RepoShared},
};
use but_ctx::Context;
use gix::{
    refs::{Category, FullName},
    utils::AsBStr,
};
use itertools::Itertools as _;
use nonempty::NonEmpty;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::{atoms::BranchArg, switch::Platform},
    bad_input,
    command::legacy::branch::{
        self,
        new::{NewOperation, NewUnstackedBranchOperation},
    },
    print_deprecation_warning,
    theme::{self, Theme},
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils, time::format_relative_time,
    },
};

mod fuzzy_search_branch_picker_tui;

pub fn switch(
    ctx: &mut Context,
    mut out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<SwitchOutcome> {
    let mut guard = ctx.exclusive_worktree_access();

    if args.new {
        print_deprecation_warning(
            "`--new/-n` is deprecated and will be removed in a future release. \
                Use `but branch new --switch` instead",
        );
    }

    let operation = resolve(ctx, guard.read_permission(), &mut out, args)?;

    Ok(run(ctx, guard.write_permission(), operation)?)
}

fn resolve(
    ctx: &Context,
    perm: &RepoShared,
    out: &mut IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<SwitchOperation> {
    let Platform {
        target,
        workspace,
        new,
    } = args;

    if workspace {
        return Ok(SwitchOperation::Workspace);
    }

    if new {
        let name = target
            .map(|target| {
                let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm)?;
                BranchArg(target.0).resolve_for_creation(&repo, &ws)
            })
            .transpose()?;

        return Ok(SwitchOperation::NewBranch { name });
    }

    if let Some(target) = target {
        let repo = ctx.repo.get()?;
        let id_map = IdMap::new_from_context(ctx, perm)?;
        let branch = target.resolve_existing_local_branch(&repo, &id_map)?;
        Ok(SwitchOperation::Branch { branch })
    } else {
        let Some(mut input) = out.prepare_for_terminal_input() else {
            return Err(bad_input(
                "Terminal input not available. Specify a branch or use `--workspace` or `--new`",
            )
            .into());
        };

        let Some(items) = switch_branch_items(ctx, perm, None, true)? else {
            return Err(bad_input("Found no branches to switch to").into());
        };

        let Some(item) =
            fuzzy_search_branch_picker_tui::run_fuzzy_branch_picker(&mut input, items)?
        else {
            return Err(bad_input("No branch picked").into());
        };

        Ok(match item {
            SwitchBranchItem::Workspace => SwitchOperation::Workspace,
            SwitchBranchItem::Branch { name, .. } => {
                let branch = Category::LocalBranch.to_full_name(&*name)?;
                SwitchOperation::Branch { branch }
            }
        })
    }
}

pub fn run(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    operation: SwitchOperation,
) -> anyhow::Result<SwitchOutcome> {
    match operation {
        SwitchOperation::Workspace => {
            let workspace_exists = {
                let repo = ctx.repo.get()?;
                repo.try_find_reference(WORKSPACE_REF_NAME)?.is_some()
            };
            if workspace_exists {
                let result = but_api::workspace::workspace_recreate_with_perm(ctx, perm)?;
                if result.already_on_workspace {
                    Ok(SwitchOutcome::AlreadyOnWorkspace)
                } else {
                    Ok(SwitchOutcome::Workspace {
                        conflicting_stacks: result.conflicting_stacks,
                    })
                }
            } else {
                but_api::legacy::virtual_branches::switch_back_to_workspace_with_perm(ctx, perm)?;
                Ok(SwitchOutcome::Workspace {
                    conflicting_stacks: Default::default(),
                })
            }
        }
        SwitchOperation::Branch { branch } => {
            but_api::branch::branch_checkout_with_perm(ctx, branch.clone(), perm)?;

            Ok(SwitchOutcome::Branch { branch })
        }
        SwitchOperation::NewBranch { name } => {
            let mut meta = ctx.meta()?;
            let outcome = branch::new::run(
                ctx,
                &mut meta,
                perm,
                NewOperation::NewUnstackedBranch(NewUnstackedBranchOperation {
                    name,
                    switch: true,
                }),
            )?;

            Ok(SwitchOutcome::CreatedBranch {
                branch: outcome.name,
            })
        }
    }
}

pub enum SwitchOperation {
    Workspace,
    Branch { branch: FullName },
    NewBranch { name: Option<FullName> },
}

#[must_use]
pub enum SwitchOutcome {
    Workspace { conflicting_stacks: Vec<FullName> },
    AlreadyOnWorkspace,
    Branch { branch: FullName },
    CreatedBranch { branch: FullName },
}

impl CliOutputHuman for SwitchOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        theme: &'static Theme,
    ) -> anyhow::Result<()> {
        match self {
            SwitchOutcome::Workspace { conflicting_stacks } => {
                writeln!(out, "Switched to workspace")?;

                if !conflicting_stacks.is_empty() {
                    writeln!(out)?;
                    writeln!(
                        out,
                        "{} Failed to apply {} due to conflicts with existing {}",
                        theme.sym().warning,
                        conflicting_stacks.iter().map(theme::Branch).join(", "),
                        if conflicting_stacks.len() == 1 {
                            "stack"
                        } else {
                            "stacks"
                        },
                    )?;
                }
            }
            SwitchOutcome::AlreadyOnWorkspace => {
                writeln!(out, "Already on workspace")?;
            }
            SwitchOutcome::Branch { branch } => {
                writeln!(out, "Switched to branch {}", theme::Branch(branch))?
            }
            SwitchOutcome::CreatedBranch { branch } => {
                writeln!(out, "Created branch {}", theme::Branch(branch))?
            }
        }

        Ok(())
    }
}

impl CliOutput for SwitchOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(
            tag = "type",
            rename_all = "camelCase",
            rename_all_fields = "camelCase"
        )]
        enum Output {
            CreatedBranch { branch: String },
            SwitchedToWorkspace { conflicting_branches: Vec<String> },
        }

        match self {
            SwitchOutcome::Workspace { conflicting_stacks } => Some(Output::SwitchedToWorkspace {
                conflicting_branches: conflicting_stacks
                    .into_iter()
                    .map(|b| b.shorten().to_string())
                    .collect(),
            }),
            SwitchOutcome::AlreadyOnWorkspace => Some(Output::SwitchedToWorkspace {
                conflicting_branches: Default::default(),
            }),
            SwitchOutcome::Branch { .. } => None,
            SwitchOutcome::CreatedBranch { branch } => Some(Output::CreatedBranch {
                branch: branch.shorten().to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SwitchBranchItem {
    Workspace,
    Branch {
        name: String,
        updated_at: Option<i64>,
        updated_at_display: String,
    },
}

pub fn switch_branch_items(
    ctx: &Context,
    perm: &RepoShared,
    priority_branch: Option<&str>,
    include_workspace: bool,
) -> anyhow::Result<Option<NonEmpty<SwitchBranchItem>>> {
    let current_branch = {
        let repo = ctx.repo.get()?;
        repo.head_ref()?
            .map(|head_ref| head_ref.name().shorten().to_owned())
    };

    let branch_listings = but_api::branch::branch_list_with_perm(ctx, perm)
        .context("Failed to list branches available to switch to")?
        .into_iter()
        .flat_map(|stack| stack.branches)
        .map(|listed_branch| listed_branch.branch)
        .filter(|branch| branch.has_local)
        .filter(|branch| {
            current_branch
                .as_ref()
                .is_none_or(|current_branch| current_branch.as_bstr() != *branch.display_name)
        });

    let now = SystemTime::now();
    let mut branches = branch_listings
        .map(|listing| SwitchBranchItem::Branch {
            name: listing.display_name.to_str_lossy().into_owned(),
            updated_at: listing.updated_at_ms,
            updated_at_display: listing
                .updated_at_ms
                .map(|updated_at| format_relative_time(now, updated_at / 1000))
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    branches.sort_by(|a, b| match (a, b) {
        (SwitchBranchItem::Workspace, _) | (_, SwitchBranchItem::Workspace) => {
            std::cmp::Ordering::Less
        }
        (
            SwitchBranchItem::Branch {
                name: a_name,
                updated_at: a_updated_at,
                ..
            },
            SwitchBranchItem::Branch {
                name: b_name,
                updated_at: b_updated_at,
                ..
            },
        ) => (priority_branch == Some(b_name.as_str()))
            .cmp(&(priority_branch == Some(a_name.as_str())))
            .then_with(|| b_updated_at.cmp(a_updated_at))
            .then_with(|| a_name.cmp(b_name)),
    });

    let items = if crate::utils::in_single_branch_mode_with_perm(ctx, perm)? && include_workspace {
        NonEmpty {
            head: SwitchBranchItem::Workspace,
            tail: branches,
        }
    } else {
        let Some(items) = NonEmpty::from_vec(branches) else {
            return Ok(None);
        };
        items
    };

    Ok(Some(items))
}
