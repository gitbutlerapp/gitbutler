use bstr::BStr;
use but_workspace::legacy::ui::StackEntry;
use gix::refs::{Category, FullName};

use crate::{CliError, CliResult, bad_input};

/// An argument atom for branches.
#[derive(Debug, Clone)]
pub struct BranchArg(pub String);

impl std::str::FromStr for BranchArg {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_owned()))
    }
}

impl std::fmt::Display for BranchArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl BranchArg {
    /// Resolve the argument to a local branch name like `refs/heads/foo`.
    ///
    /// Doesn't check that the branch actually exists.
    pub fn resolve_local_branch_name(&self) -> CliResult<FullName> {
        Ok(Category::LocalBranch.to_full_name(&*self.0)?)
    }

    /// Resolve the argument to its corresponding segment.
    pub fn resolve_segment(
        &self,
        ctx: &but_ctx::Context,
    ) -> CliResult<but_workspace::ref_info::Segment> {
        let ref_name = self.resolve_local_branch_name()?;
        let head_info = but_api::legacy::workspace::head_info(ctx)?;
        let segment = head_info
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .find(|segment| {
                if let Some(ref_info) = &segment.ref_info {
                    ref_info.ref_name == ref_name
                } else {
                    false
                }
            });
        let Some(segment) = segment else {
            return Err(bad_input(format!("Branch '{self}' not found in any stack")).into());
        };
        Ok(segment.clone())
    }

    /// Validate if the argument is suitable for naming new branches.
    pub fn resolve_for_creation(&self, repo: &gix::Repository) -> CliResult<String> {
        check_can_create_branch_with_user_provided_name(repo, &self.0)?;
        Ok(self.0.clone())
    }

    /// Resolve the argument to a branch that exists in the repository.
    pub fn resolve_branch(
        &self,
        ctx: &but_ctx::Context,
    ) -> CliResult<gitbutler_branch_actions::BranchListing> {
        let branches = but_api::legacy::virtual_branches::list_branches(ctx, None)?;
        let Some(branch) = branches.iter().find(|b| b.name.to_string() == self.0) else {
            return Err(bad_input(format!("Branch '{self}' not found")).into());
        };
        Ok(branch.clone())
    }

    /// Try to resolve the branch to a stack that exists in the workspace.
    ///
    /// Returns `None` if the branch can't be found which might be caused it not being applied.
    pub fn try_resolve_stack(&self, ctx: &but_ctx::Context) -> anyhow::Result<Option<StackEntry>> {
        let stacks = but_api::legacy::workspace::stacks(
            ctx,
            Some(but_workspace::legacy::StacksFilter::InWorkspace),
        )?;

        let stack = stacks
            .iter()
            .find(|stack| stack.heads.iter().any(|head| head.name == self.0));

        Ok(stack.cloned())
    }
}

impl AsRef<str> for BranchArg {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

/// Validate that `user_provided_branch_name` is a valid branch name that does not already exist.
///
/// Unlike the GUI, we don't normalize branch names for users in the CLI, as this could lead to
/// unexpected behavior in scripts. This function rejects names that are possible to normalize.
fn check_can_create_branch_with_user_provided_name(
    repo: &gix::Repository,
    user_provided_branch_name: &str,
) -> Result<(), CliError> {
    let normalized =
        but_core::branch::normalize_short_name(user_provided_branch_name).map_err(|err| {
            CliError::from(
                bad_input(format!("Invalid branch name: {err}"))
                    .arg_name("<BRANCH_NAME>")
                    .arg_value(user_provided_branch_name),
            )
        })?;

    let user_name_bstr: &BStr = user_provided_branch_name.into();
    if normalized != user_name_bstr {
        return Err(bad_input("Invalid branch name")
            .arg_name("<BRANCH_NAME>")
            .arg_value(user_provided_branch_name)
            .hint(format!("Try '{normalized}' instead"))
            .into());
    }

    let branch_ref_name = if user_provided_branch_name.starts_with("refs/heads") {
        user_provided_branch_name.to_string()
    } else {
        format!("refs/heads/{user_provided_branch_name}")
    };

    if repo
        .try_find_reference(&branch_ref_name.to_owned())?
        .is_some()
    {
        return Err(bad_input(format!(
            "A branch named '{user_provided_branch_name}' already exists"
        ))
        .into());
    }

    Ok(())
}
