//! Shared helpers for resolving the workspace target commit and merge base.
//!
//! These helpers intentionally require an existing read permission instead of
//! acquiring their own guard. Lock acquisition should happen at the boundary of
//! the caller so we don't accidentally nest repository locks under a wider
//! exclusive operation.

use anyhow::{Context as _, Result};
use but_core::sync::RepoShared;
use but_ctx::Context;

/// The resolved target commit and its associated reference metadata.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedTarget {
    /// The effective target commit ID.
    oid: gix::ObjectId,
    /// The target reference name when the workspace target is branch-backed.
    ref_name: Option<gix::refs::FullName>,
    /// The remote name to push target branches to when one is configured.
    push_remote_name: Option<String>,
}

/// Target information formatted for the `but status` command.
#[derive(Debug, Clone)]
pub(crate) struct StatusTarget {
    /// The commit ID shown as the workspace base in status output.
    pub(crate) commit_id: gix::ObjectId,
    /// The display name shown for the target branch in status output.
    pub(crate) display_name: String,
}

impl ResolvedTarget {
    /// Build a resolved target from workspace projection data.
    pub(crate) fn from_workspace(workspace: &but_graph::Workspace) -> Result<Self> {
        Ok(Self {
            oid: target_oid_from_workspace(workspace)?,
            ref_name: target_ref_name_from_workspace(workspace),
            push_remote_name: target_push_remote_name_from_workspace(workspace),
        })
    }

    /// Resolve the effective workspace target while reusing an existing repository permission.
    pub(crate) fn resolve_with_perm(ctx: &Context, perm: &RepoShared) -> Result<Self> {
        let (_, ws, _) = ctx.workspace_and_db_with_perm(perm)?;
        Self::from_workspace(&ws)
    }

    /// Return the effective target commit ID.
    pub(crate) fn oid(&self) -> gix::ObjectId {
        self.oid
    }

    /// Return the effective target reference name when one is available.
    pub(crate) fn ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.ref_name.as_ref().map(|name| name.as_ref())
    }

    /// Return a shortened display name for the target reference when one is available.
    pub(crate) fn display_name(&self) -> Option<String> {
        self.ref_name().map(|name| name.shorten().to_string())
    }

    /// Return the effective push remote name when one is available.
    pub(crate) fn push_remote_name(&self) -> Option<&str> {
        self.push_remote_name.as_deref()
    }

    /// Build the status-facing target information for display in `but status`.
    pub(crate) fn for_status(
        &self,
        base_branch: Option<&gitbutler_branch_actions::BaseBranch>,
    ) -> StatusTarget {
        if let Some(base_branch) = base_branch {
            return StatusTarget {
                commit_id: base_branch.base_sha,
                display_name: display_name_from_base_branch(base_branch),
            };
        }

        StatusTarget {
            commit_id: self.oid(),
            display_name: self
                .display_name()
                .unwrap_or_else(|| self.oid().to_hex_with_len(7).to_string()),
        }
    }
}

/// Build the display name `but status` should show for a legacy base branch.
fn display_name_from_base_branch(base_branch: &gitbutler_branch_actions::BaseBranch) -> String {
    match (
        base_branch.remote_name.is_empty(),
        base_branch.short_name.is_empty(),
    ) {
        (true, true) => base_branch.branch_name.clone(),
        (true, false) => base_branch.short_name.clone(),
        (false, true) => base_branch.remote_name.clone(),
        (false, false) => format!("{}/{}", base_branch.remote_name, base_branch.short_name),
    }
}

/// Resolve the effective target commit OID from workspace projection data.
fn target_oid_from_workspace(workspace: &but_graph::Workspace) -> Result<gix::ObjectId> {
    workspace.effective_target_commit_id().context(
        "Failed to resolve workspace target: no target information available in workspace.",
    )
}

/// Resolve the effective target reference name from workspace projection data.
fn target_ref_name_from_workspace(workspace: &but_graph::Workspace) -> Option<gix::refs::FullName> {
    workspace
        .target_ref
        .as_ref()
        .map(|target| target.ref_name.clone())
        .or_else(|| {
            workspace
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.target_ref.clone())
        })
}

/// Resolve the effective target push remote name from workspace projection data.
fn target_push_remote_name_from_workspace(workspace: &but_graph::Workspace) -> Option<String> {
    workspace
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.push_remote.clone())
        .or_else(|| workspace.remote_name())
}

/// Find the merge base between `branch_oid` and the effective workspace target.
///
/// Returns the merge-base commit ID together with the resolved target information.
pub(crate) fn merge_base_with_target_with_perm(
    ctx: &Context,
    perm: &RepoShared,
    branch_oid: gix::ObjectId,
) -> Result<(gix::ObjectId, ResolvedTarget)> {
    let (repo, workspace, _) = ctx.workspace_and_db_with_perm(perm)?;

    if let Some((merge_base, target_oid)) = workspace.merge_base_with_target_branch(branch_oid) {
        return Ok((
            merge_base,
            ResolvedTarget {
                oid: target_oid,
                ref_name: target_ref_name_from_workspace(&workspace),
                push_remote_name: target_push_remote_name_from_workspace(&workspace),
            },
        ));
    }

    let target = ResolvedTarget::from_workspace(&workspace)?;
    let merge_base = repo
        .merge_base(branch_oid, target.oid())
        .map(|merge_base| merge_base.detach())
        .context("Failed to find merge base with workspace target")?;
    Ok((merge_base, target))
}

#[cfg(test)]
mod tests {
    use super::{ResolvedTarget, display_name_from_base_branch};

    fn oid(hex: &str) -> gix::ObjectId {
        gix::ObjectId::from_hex(hex.as_bytes()).expect("valid object id")
    }

    fn base_branch(base_sha: gix::ObjectId) -> gitbutler_branch_actions::BaseBranch {
        gitbutler_branch_actions::BaseBranch {
            branch_name: "refs/remotes/origin/main".to_string(),
            remote_name: "origin".to_string(),
            remote_url: "https://example.com/origin".to_string(),
            push_remote_name: "origin".to_string(),
            push_remote_url: "https://example.com/origin".to_string(),
            base_sha,
            current_sha: base_sha,
            behind: 0,
            upstream_commits: vec![],
            recent_commits: vec![],
            last_fetched_ms: None,
            conflicted: false,
            target_sha_ahead_of_ref: false,
            short_name: "main".to_string(),
        }
    }

    #[test]
    fn for_status_prefers_base_branch_for_name_and_commit() {
        let resolved_target = ResolvedTarget {
            oid: oid("1111111111111111111111111111111111111111"),
            ref_name: Some("refs/remotes/upstream/topic".try_into().expect("valid ref")),
            push_remote_name: Some("upstream".to_string()),
        };
        let base_branch = base_branch(oid("2222222222222222222222222222222222222222"));

        let status_target = resolved_target.for_status(Some(&base_branch));

        assert_eq!(status_target.commit_id, base_branch.base_sha);
        assert_eq!(status_target.display_name, "origin/main");
    }

    #[test]
    fn for_status_falls_back_to_short_oid_without_ref_name() {
        let resolved_target = ResolvedTarget {
            oid: oid("3333333333333333333333333333333333333333"),
            ref_name: None,
            push_remote_name: None,
        };

        let status_target = resolved_target.for_status(None);

        assert_eq!(status_target.commit_id, resolved_target.oid());
        assert_eq!(status_target.display_name, "3333333");
    }

    #[test]
    fn base_branch_display_name_avoids_empty_separator() {
        let mut base_branch = base_branch(oid("4444444444444444444444444444444444444444"));
        base_branch.remote_name.clear();

        assert_eq!(display_name_from_base_branch(&base_branch), "main");
    }
}
