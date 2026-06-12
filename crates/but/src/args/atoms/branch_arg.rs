use std::error::Error;

use bstr::{BStr, ByteSlice};
use gix::refs::{Category, FullName, FullNameRef};

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
        head_info: &but_workspace::RefInfo,
    ) -> CliResult<but_workspace::ref_info::Segment> {
        let Some(segment) = self.try_resolve_segment(head_info)? else {
            return Err(bad_input(format!("Branch '{self}' not found in any stack")).into());
        };
        Ok(segment.clone())
    }

    /// Resolve the argument to its corresponding segment.
    pub fn try_resolve_segment(
        &self,
        head_info: &but_workspace::RefInfo,
    ) -> CliResult<Option<but_workspace::ref_info::Segment>> {
        let ref_name = self.resolve_local_branch_name()?;
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
        Ok(segment.cloned())
    }

    /// Validate that the argument is a valid branch name that does not already exist.
    ///
    /// Unlike the GUI, we don't normalize branch names for users in the CLI, as this could lead to
    /// unexpected behavior in scripts. This function rejects names that are possible to normalize.
    // TODO(david): might wanna return FullName here
    pub fn resolve_for_creation(
        &self,
        repo: &gix::Repository,
        head_info: &but_workspace::RefInfo,
    ) -> CliResult<String> {
        let branch_name = self.0.as_str();
        let normalized = but_core::branch::normalize_short_name(branch_name).map_err(|err| {
            CliError::from(bad_input(format!("Invalid branch name: {err}")).arg_value(branch_name))
        })?;

        if normalized != <&BStr>::from(branch_name) {
            return Err(bad_input("Invalid branch name")
                .arg_value(branch_name)
                .hint(format!("Try '{normalized}' instead"))
                .into());
        }

        let local_name = self.resolve_local_branch_name()?;
        if head_info
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .flat_map(|segment| &segment.ref_info)
            .any(|ref_info| ref_info.ref_name == local_name)
        {
            return Err(
                bad_input(format!("A branch named '{branch_name}' is already applied")).into(),
            );
        }

        match repo.try_find_reference(&local_name) {
            Ok(None) => Ok(self.0.clone()),
            Ok(Some(_)) => Err(bad_input(format!(
                "A branch named '{branch_name}' exists but is not applied"
            ))
            .into()),
            Err(gix::reference::find::Error::Find(err)) => {
                match err
                    .source()
                    .and_then(|err| err.downcast_ref::<std::io::Error>())
                {
                    Some(io_err) if io_err.kind() == std::io::ErrorKind::NotADirectory => {
                        // This happens when there is a prefix path collision with the name you're
                        // looking for (e.g. 'branch' exists, and you look for 'branch/new').
                        let prefix_branch_name =
                            self.find_prefix_branch_for_display(repo, &local_name)?;
                        Err(bad_input(format!("Branch name '{branch_name}' collides with existing branch '{prefix_branch_name}'")).into())
                    }
                    _ => Err(gix::reference::find::Error::Find(err).into()),
                }
            }
        }
    }

    /// Find a prefix branch of the given branch, specifically to display it to the user. May entail
    /// lossy conversion so don't use this for programmatic needs.
    ///
    /// This should only be called if you're certain a prefix branch exists.
    fn find_prefix_branch_for_display(
        &self,
        repo: &gix::Repository,
        branch_name: &FullName,
    ) -> CliResult<String> {
        let ancestors = std::iter::successors(Some(branch_name.as_bstr()), |name| {
            name.rsplit_once_str(b"/")
                .map(|(parent, _)| BStr::new(parent))
        });

        for ancestor in ancestors {
            if let Ok(Some(_)) = repo.try_find_reference(ancestor) {
                let prefix_branch_name = <&FullNameRef>::try_from(ancestor)?.shorten().to_string();
                return Ok(prefix_branch_name);
            }
        }

        Err(anyhow::anyhow!(
            "BUG: Expected to find prefix branch of '{branch_name}', but none was found",
        )
        .into())
    }

    /// Resolve the argument to a branch that exists in the repository.
    pub fn resolve_branch(&self, ctx: &but_ctx::Context) -> CliResult<ResolvedBranchRef> {
        let repo = ctx.repo.get()?;

        for category in [Category::LocalBranch, Category::RemoteBranch] {
            let branch_name = category.to_full_name(&*self.0)?;
            if let Some(resolved) = resolve_branch_ref(&repo, &branch_name)? {
                return Ok(resolved);
            }
        }

        for remote_name in repo.remote_names() {
            let branch_name = Category::RemoteBranch.to_full_name(&*format!(
                "{}/{}",
                remote_name.as_bstr().to_str_lossy(),
                self.0
            ))?;
            if let Some(resolved) = resolve_branch_ref(&repo, &branch_name)? {
                return Ok(resolved);
            }
        }

        Err(bad_input(format!("Branch '{self}' not found")).into())
    }

    /// Try to resolve the branch to a stack that exists in the workspace.
    ///
    /// Returns `None` if the branch can't be found which might be caused it not being applied.
    pub fn try_resolve_stack(
        &self,
        ctx: &but_ctx::Context,
    ) -> anyhow::Result<Option<crate::legacy::workspace::HeadInfoStack>> {
        let stacks = crate::legacy::workspace::applied_stacks(ctx)?;

        let stack = stacks.iter().find(|stack| stack.contains_branch(&self.0));

        Ok(stack.cloned())
    }
}

impl AsRef<str> for BranchArg {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

fn resolve_branch_ref(
    repo: &gix::Repository,
    branch_name: &FullName,
) -> CliResult<Option<ResolvedBranchRef>> {
    let Some(mut ref_info) = repo.try_find_reference(branch_name)? else {
        return Ok(None);
    };

    let Ok(commit) = ref_info.peel_to_id() else {
        return Ok(None);
    };

    Ok(Some(ResolvedBranchRef {
        head: commit.detach(),
    }))
}

#[expect(missing_docs, reason = "only used internally by CLI command helpers")]
pub struct ResolvedBranchRef {
    pub head: gix::ObjectId,
}
