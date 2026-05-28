use serde::Serialize;

use crate::{CliId, CliResult, IdMap, args::atoms::BranchArg, bad_input};

/// An argument atom for cli ids that can match multiple things like branches, commits, files, etc.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct CliIdArg(pub String);

impl std::str::FromStr for CliIdArg {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_owned()))
    }
}

impl std::fmt::Display for CliIdArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl CliIdArg {
    /// Resolve the argument to something that exists in the workspace.
    ///
    /// Returns an error if attempting to resolve a branch that isn't applied, since its not in the
    /// workspace.
    pub fn resolve_in_workspace(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
        purpose: Purpose,
    ) -> CliResult<ResolvedCliIdArg> {
        if let Some(id) = self.try_resolve(ctx, id_map, purpose)? {
            Ok(id)
        } else {
            Err(bad_input(format!("Could not find {purpose}: '{self}'")).into())
        }
    }

    /// Try and resolve the argument to something that might exist in the workspace.
    ///
    /// Returns `Ok(None)` if it doesn't exist in the workspace.
    pub fn try_resolve(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
        purpose: Purpose,
    ) -> CliResult<Option<ResolvedCliIdArg>> {
        let Some(id) = try_resolve_cli_id(self, ctx, id_map, purpose)? else {
            return Ok(None);
        };
        let kind = match id {
            CliId::Branch { name, .. } => {
                return Ok(Some(ResolvedCliIdArg::Branch(BranchArg(name))));
            }
            CliId::Commit { commit_id, .. } => {
                return Ok(Some(ResolvedCliIdArg::Commit(commit_id)));
            }
            CliId::Uncommitted(..) => "uncommitted file",
            CliId::PathPrefix { .. } => "path",
            CliId::CommittedFile { .. } => "committed file",
            CliId::Unassigned { .. } => "unassigned changes",
            CliId::Stack { .. } => "stack",
        };
        Err(bad_input(format!(
            "Invalid {purpose} '{self}'. Expected a commit or a branch, got {kind}"
        ))
        .into())
    }

    /// Resolve the argument to a commit that exists in the workspace.
    pub fn resolve_commit_in_workspace(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
    ) -> CliResult<gix::ObjectId> {
        if let Some(commit) = self.try_resolve_commit(ctx, id_map)? {
            Ok(commit)
        } else {
            Err(bad_input(format!("Could not find commit: '{self}'")).into())
        }
    }

    /// Try and resolve the argument a commit that might exist in the workspace.
    ///
    /// Returns `Ok(None)` if it doesn't exist in the workspace.
    pub fn try_resolve_commit(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
    ) -> CliResult<Option<gix::ObjectId>> {
        let Some(id) = try_resolve_cli_id(self, ctx, id_map, Purpose::Branch)? else {
            return Ok(None);
        };
        let kind = match id {
            CliId::Commit { commit_id, .. } => {
                return Ok(Some(commit_id));
            }
            CliId::Branch { .. } => "a branch",
            CliId::Uncommitted(..) => "an uncommitted file",
            CliId::PathPrefix { .. } => "a path",
            CliId::CommittedFile { .. } => "a committed file",
            CliId::Unassigned { .. } => "unassigned changes",
            CliId::Stack { .. } => "a stack",
        };
        Err(bad_input(format!("Invalid commit. '{self}' is {kind}")).into())
    }

    /// Resolve the argument to a branch that exists in the workspace.
    pub fn resolve_branch_in_workspace(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
    ) -> CliResult<BranchArg> {
        if let Some(branch) = self.try_resolve_branch(ctx, id_map)? {
            Ok(branch)
        } else {
            Err(bad_input(format!("Could not find branch: '{self}'")).into())
        }
    }

    /// Try and resolve the argument a branch that might exist in the workspace.
    ///
    /// Returns `Ok(None)` if it doesn't exist in the workspace.
    pub fn try_resolve_branch(
        &self,
        ctx: &but_ctx::Context,
        id_map: &IdMap,
    ) -> CliResult<Option<BranchArg>> {
        let Some(id) = try_resolve_cli_id(self, ctx, id_map, Purpose::Branch)? else {
            return Ok(None);
        };
        let kind = match id {
            CliId::Branch { name, .. } => {
                return Ok(Some(BranchArg(name)));
            }
            CliId::Commit { .. } => "a commit",
            CliId::Uncommitted(..) => "an uncommitted file",
            CliId::PathPrefix { .. } => "a path",
            CliId::CommittedFile { .. } => "a committed file",
            CliId::Unassigned { .. } => "unassigned changes",
            CliId::Stack { .. } => "a stack",
        };
        Err(bad_input(format!("Invalid branch. '{self}' is {kind}")).into())
    }
}

// intentionally private since callers should use the more specific resolution methods on
// `CliIdArg`
//
// returns `Option` because the IdMap doesn't contain things that aren't in the workspace such as
// unapplied branches or commits outside the workspace. Lots of commands do support things outside
// the workspace so we need a specific type for that.
fn try_resolve_cli_id(
    arg: &CliIdArg,
    ctx: &but_ctx::Context,
    id_map: &IdMap,
    purpose: Purpose,
) -> CliResult<Option<CliId>> {
    let mut target_ids = id_map.parse_using_context(&arg.0, ctx)?.into_iter();
    let Some(target) = target_ids.next() else {
        // return Err(bad_input(format!("Could not find {purpose}: '{arg};")).into());
        return Ok(None);
    };
    if target_ids.next().is_some() {
        return Err(bad_input(format!(
            "Ambiguous {purpose} '{arg}', matches multiple items"
        ))
        .into());
    }
    Ok(Some(target))
}

/// The "purpose" of the resolution. Used in error messages.
#[derive(Debug, Copy, Clone)]
pub enum Purpose {
    #[expect(missing_docs)]
    Anchor,
    #[expect(missing_docs)]
    Branch,
    #[expect(missing_docs)]
    Target,
}

impl std::fmt::Display for Purpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Purpose::Anchor => f.write_str("anchor"),
            Purpose::Branch => f.write_str("branch"),
            Purpose::Target => f.write_str("target"),
        }
    }
}

/// A [`CliIdArg`] that has actually been resolved.
#[derive(Debug, Clone)]
pub enum ResolvedCliIdArg {
    #[expect(missing_docs)]
    Commit(gix::ObjectId),
    #[expect(missing_docs)]
    Branch(BranchArg),
}

impl std::fmt::Display for ResolvedCliIdArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvedCliIdArg::Commit(inner) => inner.to_hex_with_len(7).fmt(f),
            ResolvedCliIdArg::Branch(inner) => inner.fmt(f),
        }
    }
}
