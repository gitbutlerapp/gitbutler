use bstr::BString;
use nonempty::NonEmpty;
use serde::Serialize;

use crate::{
    CliError, CliId, CliResult, IdMap,
    args::atoms::BranchArg,
    bad_input,
    id::{ShortId, UncommittedHunkOrFile},
};

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
    #[expect(missing_docs)]
    pub const TARGET_MISSING_HINT: &str = "Run `but status` for applicable targets.";

    /// Resolve the argument to something that exists in the workspace.
    ///
    /// Returns an error if attempting to resolve a branch that isn't applied, since its not in the
    /// workspace.
    pub fn resolve_in_workspace(
        &self,
        repo: &gix::Repository,
        id_map: &IdMap,
        purpose: Purpose,
        priority: Option<Priority>,
    ) -> CliResult<ResolvedCliIdArg> {
        if let Some(id) = self.try_resolve(repo, id_map, purpose, priority)? {
            Ok(id)
        } else {
            Err(bad_input(format!("Could not find {purpose}: '{self}'"))
                .hint(Self::TARGET_MISSING_HINT)
                .into())
        }
    }

    /// Try and resolve the argument to something that might exist in the workspace.
    ///
    /// Returns `Ok(None)` if it doesn't exist in the workspace.
    pub fn try_resolve(
        &self,
        repo: &gix::Repository,
        id_map: &IdMap,
        purpose: Purpose,
        priority: Option<Priority>,
    ) -> CliResult<Option<ResolvedCliIdArg>> {
        let Some(id) = try_resolve_cli_id(self, repo, id_map, purpose, priority)? else {
            return Ok(None);
        };
        Ok(Some(match id {
            CliId::Branch { name, .. } => ResolvedCliIdArg::Branch(BranchArg(name)),
            CliId::Commit { commit_id, .. } => ResolvedCliIdArg::Commit(commit_id),
            CliId::UncommittedHunkOrFile(uncommitted) => {
                ResolvedCliIdArg::UncommittedHunkOrFile(Box::new(uncommitted))
            }
            CliId::PathPrefix { .. } => ResolvedCliIdArg::PathPrefix,
            CliId::CommittedFile {
                commit_id,
                path,
                id,
            } => ResolvedCliIdArg::CommittedFile {
                commit_id,
                path,
                id,
            },
            CliId::Uncommitted { .. } => ResolvedCliIdArg::Uncommitted,
            CliId::Stack { .. } => ResolvedCliIdArg::Stack,
        }))
    }

    /// Resolve the argument to a commit that exists in the workspace.
    pub fn resolve_commit_in_workspace(
        &self,
        repo: &gix::Repository,
        id_map: &IdMap,
    ) -> CliResult<gix::ObjectId> {
        if let Some(commit) = self.try_resolve_commit(repo, id_map)? {
            Ok(commit)
        } else {
            Err(bad_input(format!("Could not find commit: '{self}'"))
                .hint(Self::TARGET_MISSING_HINT)
                .into())
        }
    }

    /// Try and resolve the argument a commit that might exist in the workspace.
    ///
    /// Returns `Ok(None)` if it doesn't exist in the workspace.
    pub fn try_resolve_commit(
        &self,
        repo: &gix::Repository,
        id_map: &IdMap,
    ) -> CliResult<Option<gix::ObjectId>> {
        let Some(id) =
            try_resolve_cli_id(self, repo, id_map, Purpose::Commit, Some(Priority::Commit))?
        else {
            return Ok(None);
        };
        match id {
            CliId::Commit { commit_id, .. } => Ok(Some(commit_id)),
            _ => Ok(None),
        }
    }

    /// Resolve the argument to a branch that exists in the workspace.
    pub fn resolve_branch_in_workspace(
        &self,
        repo: &gix::Repository,
        id_map: &IdMap,
    ) -> CliResult<BranchArg> {
        if let Some(branch) = self.try_resolve_branch(repo, id_map)? {
            Ok(branch)
        } else {
            Err(bad_input(format!("Could not find branch: '{self}'"))
                .hint(Self::TARGET_MISSING_HINT)
                .into())
        }
    }

    /// Resolve the argument to an existing local branch reference or workspace branch CLI ID.
    pub fn resolve_existing_local_branch(
        &self,
        repo: &gix::Repository,
        id_map: &IdMap,
    ) -> CliResult<gix::refs::FullName> {
        let branch = BranchArg(self.0.clone());
        if let Some(branch_ref) = branch.try_resolve_existing_local_branch(repo)? {
            return Ok(branch_ref);
        }

        self.resolve_branch_in_workspace(repo, id_map)?
            .resolve_existing_local_branch(repo)
    }

    /// Try and resolve the argument a branch that might exist in the workspace.
    ///
    /// Returns `Ok(None)` if it doesn't exist in the workspace.
    pub fn try_resolve_branch(
        &self,
        repo: &gix::Repository,
        id_map: &IdMap,
    ) -> CliResult<Option<BranchArg>> {
        let Some(id) =
            try_resolve_cli_id(self, repo, id_map, Purpose::Branch, Some(Priority::Branch))?
        else {
            return Ok(None);
        };
        match id {
            CliId::Branch { name, .. } => Ok(Some(BranchArg(name))),
            _ => Ok(None),
        }
    }

    /// TODO: docs
    pub fn try_resolve_uncommitted(
        &self,
        repo: &gix::Repository,
        id_map: &IdMap,
    ) -> CliResult<Option<Vec<UncommittedHunkOrFile>>> {
        let Some(id) = try_resolve_cli_id(
            self,
            repo,
            id_map,
            Purpose::Uncommitted,
            Some(Priority::Uncommitted),
        )?
        else {
            return Ok(None);
        };
        match id {
            CliId::UncommittedHunkOrFile(uncommitted) => Ok(Some(vec![uncommitted])),
            CliId::PathPrefix {
                id: _,
                hunk_assignments,
            } => Ok(Some(
                hunk_assignments
                    .into_iter()
                    .map(|(id, assignment)| UncommittedHunkOrFile {
                        id,
                        hunk_assignments: NonEmpty::new(assignment),
                        // In a world without staging, all these hunk assignments should be turned
                        // into "entire file" assignments for every file under the given PathPrefix.
                        // However, currently, already assigned changes are not resolved by
                        // PathPrefix. This should all be fixed at the level of resolving the
                        // PathPrefix rather than here, though.
                        is_entire_file: false,
                    })
                    .collect(),
            )),
            _ => Ok(None),
        }
    }

    /// TODO
    pub fn resolve_uncommitted(
        &self,
        repo: &gix::Repository,
        id_map: &IdMap,
    ) -> CliResult<Vec<UncommittedHunkOrFile>> {
        if let Some(uncommitted) = self.try_resolve_uncommitted(repo, id_map)? {
            Ok(uncommitted)
        } else {
            Err(
                bad_input(format!("Could not find uncommitted change: '{self}'"))
                    .hint(Self::TARGET_MISSING_HINT)
                    .into(),
            )
        }
    }

    #[expect(dead_code)]
    fn wrong_kind_error(&self, id: &CliId, expected: &'static str) -> CliError {
        let kind = match id {
            CliId::Branch { .. } => "a branch",
            CliId::Commit { .. } => "a commit",
            CliId::UncommittedHunkOrFile(..) => "an uncommitted change",
            CliId::PathPrefix { .. } => "a path",
            CliId::CommittedFile { .. } => "a committed file",
            CliId::Uncommitted { .. } => "uncommitted changes",
            CliId::Stack { .. } => "a stack",
        };
        bad_input(format!("Invalid {expected}. '{self}' is {kind}")).into()
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
    /// Prioritize branches and commits.
    BranchAndCommit,
    /// Prioritize uncommitted changes.
    Uncommitted,
}

// intentionally private since callers should use the more specific resolution methods on
// `CliIdArg`
//
// returns `Option` because the IdMap doesn't contain things that aren't in the workspace such as
// unapplied branches or commits outside the workspace. Lots of commands do support things outside
// the workspace so we need a specific type for that.
fn try_resolve_cli_id(
    arg: &CliIdArg,
    repo: &gix::Repository,
    id_map: &IdMap,
    purpose: Purpose,
    priority: Option<Priority>,
) -> CliResult<Option<CliId>> {
    let mut target_ids = id_map
        .parse_using_repo(&arg.0, repo)?
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
        let mut uncommitted = Vec::new();
        for id in std::iter::once(target).chain(target_ids) {
            match id {
                CliId::Branch { .. } => branches.push(id),
                CliId::Commit { .. } => commits.push(id),
                CliId::UncommittedHunkOrFile(..) => uncommitted.push(id),
                CliId::PathPrefix { .. }
                | CliId::CommittedFile { .. }
                | CliId::Uncommitted { .. }
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
            Priority::Uncommitted => {
                if uncommitted.len() == 1 {
                    return Ok(Some(uncommitted.pop().unwrap()));
                }
            }
            Priority::BranchAndCommit => match (branches.len(), commits.len()) {
                (1, 0) => {
                    return Ok(Some(branches.pop().unwrap()));
                }
                (0, 1) => {
                    return Ok(Some(commits.pop().unwrap()));
                }
                _ => {}
            },
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
    #[expect(missing_docs)]
    Source,
    #[expect(missing_docs)]
    Uncommitted,
}

impl std::fmt::Display for Purpose {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Purpose::Anchor => f.write_str("anchor"),
            Purpose::Branch => f.write_str("branch"),
            Purpose::Target => f.write_str("target"),
            Purpose::Source => f.write_str("source"),
            Purpose::Commit => f.write_str("commit"),
            Purpose::Uncommitted => f.write_str("uncommitted change"),
        }
    }
}

/// A [`CliIdArg`] that has actually been resolved.
#[derive(Debug, Clone)]
#[expect(missing_docs)]
pub enum ResolvedCliIdArg {
    Commit(gix::ObjectId),
    Branch(BranchArg),
    UncommittedHunkOrFile(Box<UncommittedHunkOrFile>),
    CommittedFile {
        commit_id: gix::ObjectId,
        path: BString,
        id: ShortId,
    },
    // These have no data because we don't have any commands that use them. So just add data if you
    // have a use case
    PathPrefix,
    Uncommitted,
    Stack,
}

impl ResolvedCliIdArg {
    /// Convert this into either a branch or a commit.
    pub fn into_branch_or_commit(self) -> CliResult<BranchOrCommit> {
        let kind = match self {
            ResolvedCliIdArg::Commit(commit) => return Ok(BranchOrCommit::Commit(commit)),
            ResolvedCliIdArg::Branch(branch) => return Ok(BranchOrCommit::Branch(branch)),
            ResolvedCliIdArg::UncommittedHunkOrFile(..) => "an uncommitted change",
            ResolvedCliIdArg::CommittedFile { .. } => "a committed file",
            ResolvedCliIdArg::PathPrefix => "a path",
            ResolvedCliIdArg::Uncommitted => "uncommitted changes",
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
            ResolvedCliIdArg::UncommittedHunkOrFile(..) => f.write_str("uncommitted file or hunk"),
            ResolvedCliIdArg::PathPrefix => f.write_str("path"),
            ResolvedCliIdArg::CommittedFile { .. } => f.write_str("committed file"),
            ResolvedCliIdArg::Uncommitted => f.write_str("uncommitted changes"),
            ResolvedCliIdArg::Stack => f.write_str("stack"),
        }
    }
}

/// Most commands need cli ids that point to either branches or commits.
/// [`ResolvedCliIdArg::into_branch_or_commit`] facilitates that via this enum.
#[derive(Debug, Clone)]
#[expect(missing_docs)]
pub enum BranchOrCommit {
    Commit(gix::ObjectId),
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
