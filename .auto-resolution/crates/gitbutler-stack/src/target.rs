use anyhow::Result;
use but_ctx::Context;
use but_meta::virtual_branches_legacy_types;
use gitbutler_reference::RemoteRefname;

#[derive(Debug, PartialEq, Clone)]
pub struct Target {
    /// The combination of remote name and branch name, i.e. `origin` and `main`.
    /// The remote name is the one used to fetch from.
    /// It's equivalent to e.g. `refs/remotes/origin/main` , and the type `RemoteRefName`
    /// stores it as `<remote>` and `<suffix>` so that finding references named `<remote>/<suffix>`
    /// will typically find the local tracking branch unambiguously.
    pub branch: RemoteRefname,
    /// The URL of the remote behind the symbolic name.
    pub remote_url: String,
    /// The merge-base between `branch` and the current worktree `HEAD` upon first creation,
    /// but then it's the set to the new destination of e.g. `refs/remotes/origin/main` after
    /// the remote was fetched. This value is used to determine if there was a change,
    /// and if the *workspace* needs to be recalculated/rebased against the new commit.
    // TODO(ST): is it safe/correct to rename this to `branch_target_id`? Should be!
    //           It's just a bit strange it starts life as merge-base, but maybe it ends
    //           up the same anyway? Definitely could use a test then.
    pub sha: gix::ObjectId,
    /// The name of the remote to push to.
    pub push_remote_name: Option<String>,
}

impl From<virtual_branches_legacy_types::Target> for Target {
    fn from(
        virtual_branches_legacy_types::Target {
            branch,
            remote_url,
            sha,
            push_remote_name,
        }: virtual_branches_legacy_types::Target,
    ) -> Self {
        Target {
            branch,
            remote_url,
            sha,
            push_remote_name,
        }
    }
}

impl From<Target> for virtual_branches_legacy_types::Target {
    fn from(
        Target {
            branch,
            remote_url,
            sha,
            push_remote_name,
        }: Target,
    ) -> Self {
        virtual_branches_legacy_types::Target {
            branch,
            remote_url,
            sha,
            push_remote_name,
        }
    }
}

pub(crate) fn default_target_base_oid(ctx: &Context) -> Result<gix::ObjectId> {
    ctx.project_meta()?.target_commit_id_or_err()
}

pub(crate) fn default_target_push_remote_name(ctx: &Context) -> Result<String> {
    let repo = ctx.repo.get()?;
    ctx.project_meta()?.push_remote_name(&repo)
}
