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
        priority: Option<Priority>,
    ) -> CliResult<ResolvedCliIdArg> {
        if let Some(id) = self.try_resolve(ctx, id_map, purpose, priority)? {
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
        priority: Option<Priority>,
    ) -> CliResult<Option<ResolvedCliIdArg>> {
        let Some(id) = try_resolve_cli_id(self, ctx, id_map, purpose, priority)? else {
            return Ok(None);
        };
        Ok(Some(match id {
            CliId::Branch { name, .. } => ResolvedCliIdArg::Branch(BranchArg(name)),
            CliId::Commit { commit_id, .. } => ResolvedCliIdArg::Commit(commit_id),
            CliId::Uncommitted(..) => ResolvedCliIdArg::Uncommitted,
            CliId::PathPrefix { .. } => ResolvedCliIdArg::PathPrefix,
            CliId::CommittedFile { .. } => ResolvedCliIdArg::CommittedFile,
            CliId::Unassigned { .. } => ResolvedCliIdArg::Unassigned,
            CliId::Stack { .. } => ResolvedCliIdArg::Stack,
        }))
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
        let Some(id) =
            try_resolve_cli_id(self, ctx, id_map, Purpose::Commit, Some(Priority::Commit))?
        else {
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
        let Some(id) =
            try_resolve_cli_id(self, ctx, id_map, Purpose::Branch, Some(Priority::Branch))?
        else {
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

/// Which kinds of objects id resolution should prioritize in the event of ambiguity.
///
/// For example "foo" might match a branch called "foo" or an uncommitted file called "foo". By
/// using `Priority::Branch` we'd get the branch.
///
/// If there are multiple objects of the same type matched and prioritized (i.e. multiple branches)
/// then the resolution is still ambiguous.
#[derive(Copy, Clone, Debug)]
pub enum Priority {
    /// Prioritize branches.
    Branch,
    /// Prioritize commits.
    Commit,
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
    priority: Option<Priority>,
) -> CliResult<Option<CliId>> {
    let mut target_ids = id_map
        .parse_using_context(&arg.0, ctx)?
        .into_iter()
        .peekable();
    let Some(target) = target_ids.next() else {
        return Ok(None);
    };

    if target_ids.peek().is_none() {
        return Ok(Some(target));
    }

    if let Some(priority) = priority {
        let mut commits = Vec::new();
        let mut branches = Vec::new();
        for id in std::iter::once(target).chain(target_ids) {
            match id {
                CliId::Branch { .. } => branches.push(id),
                CliId::Commit { .. } => commits.push(id),
                CliId::Uncommitted(..)
                | CliId::PathPrefix { .. }
                | CliId::CommittedFile { .. }
                | CliId::Unassigned { .. }
                | CliId::Stack { .. } => {}
            }
        }

        match priority {
            Priority::Branch => {
                if branches.len() == 1 {
                    return Ok(Some(branches.pop().unwrap()));
                }
            }
            Priority::Commit => {
                if commits.len() == 1 {
                    return Ok(Some(commits.pop().unwrap()));
                }
            }
        }
    }

    Err(bad_input(format!(
        "Ambiguous {purpose} '{arg}', matches multiple items"
    ))
    .into())
}

/// The "purpose" of the resolution. Used in error messages.
#[derive(Debug, Copy, Clone)]
pub enum Purpose {
    #[expect(missing_docs)]
    Anchor,
    #[expect(missing_docs)]
    Branch,
    #[expect(missing_docs)]
    Commit,
    #[expect(missing_docs)]
    Target,
}

impl std::fmt::Display for Purpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Purpose::Anchor => f.write_str("anchor"),
            Purpose::Branch => f.write_str("branch"),
            Purpose::Target => f.write_str("target"),
            Purpose::Commit => f.write_str("commit"),
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
    // These have no data because we don't have any commands that use them. So just add data if you
    // have a use case
    #[expect(missing_docs)]
    Uncommitted,
    #[expect(missing_docs)]
    PathPrefix,
    #[expect(missing_docs)]
    CommittedFile,
    #[expect(missing_docs)]
    Unassigned,
    #[expect(missing_docs)]
    Stack,
}

impl ResolvedCliIdArg {
    /// Convert this into either a branch or a commit.
    pub fn into_branch_or_commit(self) -> CliResult<BranchOrCommit> {
        let kind = match self {
            ResolvedCliIdArg::Commit(commit) => return Ok(BranchOrCommit::Commit(commit)),
            ResolvedCliIdArg::Branch(branch) => return Ok(BranchOrCommit::Branch(branch)),
            ResolvedCliIdArg::Uncommitted => "an uncommitted file",
            ResolvedCliIdArg::PathPrefix => "a path",
            ResolvedCliIdArg::CommittedFile => "a committed file",
            ResolvedCliIdArg::Unassigned => "unassigned changes",
            ResolvedCliIdArg::Stack => "a stack",
        };
        Err(bad_input(format!("Expected a commit or a branch, got {kind}")).into())
    }
}

impl std::fmt::Display for ResolvedCliIdArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvedCliIdArg::Commit(inner) => inner.to_hex_with_len(7).fmt(f),
            ResolvedCliIdArg::Branch(inner) => inner.fmt(f),
            ResolvedCliIdArg::Uncommitted => f.write_str("uncommitted file"),
            ResolvedCliIdArg::PathPrefix => f.write_str("path"),
            ResolvedCliIdArg::CommittedFile => f.write_str("committed file"),
            ResolvedCliIdArg::Unassigned => f.write_str("unassigned changes"),
            ResolvedCliIdArg::Stack => f.write_str("stack"),
        }
    }
}

/// Most commands need cli ids that point to either branches or commits.
/// [`ResolvedCliIdArg::into_branch_or_commit`] facilitates that via this enum.
#[derive(Debug, Clone)]
pub enum BranchOrCommit {
    #[expect(missing_docs)]
    Commit(gix::ObjectId),
    #[expect(missing_docs)]
    Branch(BranchArg),
}

impl std::fmt::Display for BranchOrCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BranchOrCommit::Commit(inner) => inner.to_hex_with_len(7).fmt(f),
            BranchOrCommit::Branch(inner) => inner.fmt(f),
        }
    }
}
