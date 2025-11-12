use anyhow::{Context, Result};
use but_meta::virtual_branches_legacy_types;
use but_oxidize::{ObjectIdExt, OidExt};
use gitbutler_reference::RemoteRefname;
use gitbutler_repo::RepositoryExt;

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
    pub sha: git2::Oid,
    /// The name of the remote to push to.
    pub push_remote_name: Option<String>,
}

impl Target {
    pub fn push_remote_name(&self) -> String {
        match &self.push_remote_name {
            Some(remote) => remote.clone(),
            None => self.branch.remote().to_owned(),
        }
    }

    /// Returns the head sha of the remote branch this target is tracking.
    pub fn remote_head(&self, repo: &git2::Repository) -> Result<git2::Oid> {
        let branch = repo.find_branch_by_refname(&self.branch.clone().into())?;
        let oid = branch
            .get()
            .target()
            .context("failed to get default commit")?;
        Ok(oid)
    }
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
            sha: sha.to_git2(),
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
            sha: sha.to_gix(),
            push_remote_name,
        }
    }
}
