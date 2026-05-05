use anyhow::{Context as _, Result, anyhow};
use but_ctx::Context;
use but_error::Code;
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

impl Target {
    pub fn push_remote_name(&self) -> String {
        match &self.push_remote_name {
            Some(remote) => remote.clone(),
            None => self.branch.remote().to_owned(),
        }
    }

    /// Returns the head sha of the remote branch this target is tracking.
    pub fn remote_head(&self, repo: &gix::Repository) -> Result<gix::ObjectId> {
        let oid = repo
            .find_reference(&self.branch.to_string())?
            .try_id()
            .context("failed to get default commit")?;
        Ok(oid.detach())
    }

    /// The local branch ref that this target tracks (e.g. `refs/heads/master`
    /// when the remote branch is `origin/master`).
    pub fn local_ref(&self) -> String {
        format!("refs/heads/{}", self.branch.branch())
    }

    /// Resolve `sha` from the local tracking branch, creating it from the
    /// currently stored `sha` if it doesn't exist yet.
    ///
    /// This should be called eagerly after constructing a `Target` from storage
    /// so that `sha` always reflects the current tip of the local branch.
    pub fn resolve_sha(&mut self, repo: &gix::Repository) -> Result<()> {
        let local_ref = self.local_ref();
        self.sha = match repo.find_reference(&local_ref) {
            Ok(reference) => reference
                .try_id()
                .context("local branch is not a direct reference")?
                .detach(),
            Err(_) => {
                // Local branch doesn't exist — seed it from the stored target SHA
                // so we preserve the current workspace base while establishing
                // the ref as the source of truth going forward.
                repo.reference(
                    local_ref,
                    self.sha,
                    gix::refs::transaction::PreviousValue::MustNotExist,
                    "gitbutler: create local tracking branch",
                )?;
                self.sha
            }
        };
        Ok(())
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
    let mut target = ctx
        .legacy_meta()?
        .data()
        .default_target
        .clone()
        .ok_or_else(|| {
            anyhow!("there is no default target").context(Code::DefaultTargetNotFound)
        })?;
    target.resolve_sha(&*ctx.repo.get()?)?;
    Ok(target.sha)
}

pub(crate) fn default_target_push_remote_name(ctx: &Context) -> Result<String> {
    default_target_push_remote_name_from_state(ctx.legacy_meta()?.data().default_target.as_ref())
}

fn default_target_push_remote_name_from_state(
    target: Option<&virtual_branches_legacy_types::Target>,
) -> Result<String> {
    target
        .map(|target| {
            target
                .push_remote_name
                .clone()
                .unwrap_or_else(|| target.branch.remote().to_owned())
        })
        .ok_or_else(|| anyhow!("there is no default target").context(Code::DefaultTargetNotFound))
}
