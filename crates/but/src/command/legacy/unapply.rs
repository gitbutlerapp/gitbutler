//! Implementation of the `but unapply` command.

use anyhow::Context as _;
use but_core::{ref_metadata::StackId, sync::RepoExclusive};
use but_ctx::Context;
use but_workspace::RefInfo;
use gix::refs::FullName;
use itertools::Itertools as _;
use serde::Serialize;

use crate::{
    CliResult, IdMap,
    args::{
        atoms::{BranchOrStack, CliIdArg, Priority, Purpose},
        unapply::Platform,
    },
    bad_input,
    theme::{self, Theme},
    utils::{CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils},
};

pub struct UnapplyOutcome {
    pub branches_in_stack: Vec<FullName>,
}

impl CliOutputHuman for UnapplyOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let Self { branches_in_stack } = self;

        let branches_in_stack = branches_in_stack.iter().map(theme::Branch).join(", ");

        writeln!(
            out,
            "Unapplied stack with {branches_in_stack} from workspace"
        )?;

        Ok(())
    }
}

impl CliOutput for UnapplyOutcome {
    fn on_json(self) -> impl Serialize {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            branches: Vec<String>,
        }

        let Self { branches_in_stack } = self;

        let branches = branches_in_stack
            .iter()
            .map(|branch| branch.shorten().to_string())
            .collect();

        Output { branches }
    }
}

pub fn unapply(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    args: Platform,
) -> CliResult<UnapplyOutcome> {
    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;
    let head_info = but_api::legacy::workspace::head_info(ctx)?;

    let operation = {
        let repo = ctx.repo.get()?;
        resolve(args, &id_map, &repo, &head_info)?
    };

    Ok(run(ctx, guard.write_permission(), &head_info, operation)?)
}

fn resolve(
    args: Platform,
    id_map: &IdMap,
    repo: &gix::Repository,
    head_info: &RefInfo,
) -> CliResult<UnapplyOperation> {
    let Platform { target } = args;

    let stack = match target
        .resolve_in_workspace(repo, id_map, Purpose::Source, Some(Priority::Branch))?
        .into_branch_or_stack()?
    {
        BranchOrStack::Branch(branch_arg) => {
            let branch = branch_arg.resolve_local_branch_name()?;

            let stack = head_info.stacks.iter().find(|stack| {
                stack.segments.iter().any(|segment| {
                    segment
                        .ref_info
                        .as_ref()
                        .is_some_and(|ref_info| ref_info.ref_name == branch)
                })
            });

            let Some(stack) = stack else {
                return Err(bad_input(format!("Branch {} not found", branch.shorten()))
                    .hint(CliIdArg::TARGET_MISSING_HINT)
                    .into());
            };

            stack
        }
        BranchOrStack::Stack { stack_id, id } => {
            let stack = head_info
                .stacks
                .iter()
                .find(|stack| stack.id.is_some_and(|id| id == stack_id));

            let Some(stack) = stack else {
                return Err(bad_input(format!("Stack {id} not found"))
                    .hint(CliIdArg::TARGET_MISSING_HINT)
                    .into());
            };

            stack
        }
    };

    let stack_id = stack.id.context("BUG: stack has no id")?;

    Ok(UnapplyOperation { stack_id })
}

pub fn run(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    head_info: &RefInfo,
    operation: UnapplyOperation,
) -> anyhow::Result<UnapplyOutcome> {
    let UnapplyOperation { stack_id } = operation;

    let stack = head_info
        .stacks
        .iter()
        .find(|stack| stack.id.is_some_and(|id| id == stack_id))
        .context("BUG: stack not found")?;

    let branches_in_stack = stack
        .segments
        .iter()
        .filter_map(|segment| Some(segment.ref_info.as_ref()?.ref_name.clone()))
        .collect::<Vec<_>>();

    but_api::legacy::virtual_branches::unapply_stack_with_perm(ctx, stack_id, perm)?;

    Ok(UnapplyOutcome { branches_in_stack })
}

pub struct UnapplyOperation {
    pub stack_id: StackId,
}
