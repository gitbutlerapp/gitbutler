use bstr::{BStr, ByteSlice};
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
    pub fn resolve_for_creation(
        &self,
        repo: &gix::Repository,
        ws: &but_graph::Workspace,
    ) -> CliResult<FullName> {
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
        if ws.is_reachable_from_entrypoint(local_name.as_ref()) {
            return Err(
                bad_input(format!("A branch named '{branch_name}' is already applied")).into(),
            );
        }

        if repo.try_find_reference(&local_name)?.is_some() {
            return Err(bad_input(format!(
                "A branch named '{branch_name}' exists but is not applied"
            ))
            .into());
        }

        Ok(local_name)
    }

    /// Resolve the argument to a branch that exists in the repository.
    pub fn resolve_branch(&self, repo: &gix::Repository) -> CliResult<ResolvedBranchRef> {
        for category in [Category::LocalBranch, Category::RemoteBranch] {
            let branch_name = category.to_full_name(&*self.0)?;
            if let Some(resolved) = resolve_branch_ref(repo, &branch_name)? {
                return Ok(resolved);
            }
        }

        for remote_name in repo.remote_names() {
            let branch_name = Category::RemoteBranch.to_full_name(&*format!(
                "{}/{}",
                remote_name.as_bstr().to_str_lossy(),
                self.0
            ))?;
            if let Some(resolved) = resolve_branch_ref(repo, &branch_name)? {
                return Ok(resolved);
            }
        }

        Err(bad_input(format!("Branch '{self}' not found")).into())
    }

    /// Resolve the argument to an existing local branch reference.
    pub fn resolve_existing_local_branch(&self, repo: &gix::Repository) -> CliResult<FullName> {
        if let Some(branch) = self.try_resolve_existing_local_branch(repo)? {
            return Ok(branch);
        }
        Err(bad_input(format!("Could not find branch: '{self}'")).into())
    }

    /// Try to resolve the argument to an existing local branch reference.
    ///
    /// Returns `Ok(None)` when the argument is not an existing branch and does not look like a
    /// remote branch.
    pub fn try_resolve_existing_local_branch(
        &self,
        repo: &gix::Repository,
    ) -> CliResult<Option<FullName>> {
        if self.0.starts_with("refs/heads/") {
            let branch = FullName::try_from(self.0.as_str())
                .map_err(|_| bad_input(format!("Invalid branch ref '{self}'")))?;
            ensure_existing_local_branch(repo, &branch)?;
            return Ok(Some(branch));
        }

        if self.0.starts_with("refs/remotes/") {
            return Err(
                bad_input(format!("Can only switch to local branches, got '{self}'")).into(),
            );
        }

        if let Ok(branch) = Category::LocalBranch.to_full_name(self.0.as_str())
            && repo.try_find_reference(branch.as_ref())?.is_some()
        {
            return Ok(Some(branch));
        }

        if looks_like_remote_branch(repo, self.0.as_str()) {
            return Err(
                bad_input(format!("Can only switch to local branches, got '{self}'")).into(),
            );
        }

        Ok(None)
    }

    /// Try to resolve the branch to a stack that exists in the workspace.
    ///
    /// Returns `None` if the branch can't be found which might be caused it not being applied.
    #[cfg(feature = "legacy")]
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

fn ensure_existing_local_branch(repo: &gix::Repository, branch: &FullName) -> CliResult<()> {
    if !branch.as_bstr().starts_with_str("refs/heads/") {
        return Err(bad_input(format!("Can only switch to local branches, got '{branch}'")).into());
    }
    if repo.try_find_reference(branch.as_ref())?.is_none() {
        return Err(bad_input(format!("Branch '{}' not found", branch.shorten())).into());
    }
    Ok(())
}

fn looks_like_remote_branch(repo: &gix::Repository, target: &str) -> bool {
    repo.remote_names().iter().any(|remote| {
        target
            .as_bytes()
            .strip_prefix(remote.as_bstr().as_bytes())
            .is_some_and(|rest| rest.starts_with(b"/"))
    })
}

#[expect(missing_docs, reason = "only used internally by CLI command helpers")]
pub struct ResolvedBranchRef {
    pub head: gix::ObjectId,
}
