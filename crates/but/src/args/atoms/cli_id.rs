use but_core::ref_metadata::StackId;
use nonempty::NonEmpty;
use serde::Serialize;

use crate::{
    CliError, CliId, CliResult, IdMap,
    args::atoms::BranchArg,
    bad_input,
    id::{CommitId, CommitIdRef, CommittedFileId, IdAndHunk, UncommittedHunkOrFile},
    theme,
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

    /// Parse this argument into all matching CLI IDs in the workspace.
    pub fn parse(&self, repo: &gix::Repository, id_map: &IdMap) -> CliResult<Vec<CliId>> {
        Ok(id_map.parse_using_repo(&self.0, repo)?)
    }

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
        let id = if matches!(purpose, Purpose::Uncommitted) {
            debug_assert!(
                priority.is_none(),
                "uncommitted-only resolution does not accept cross-kind priority"
            );
            match self.try_resolve_uncommitted_id(repo, id_map)? {
                Some(id) => Some(id),
                None => try_resolve_cli_id(self, repo, id_map, purpose, priority)?,
            }
        } else {
            try_resolve_cli_id(self, repo, id_map, purpose, priority)?
        };
        let Some(id) = id else {
            return Ok(None);
        };
        Ok(Some(match id {
            CliId::Branch(branch) => ResolvedCliIdArg::Branch(BranchArg(branch.name)),
            CliId::Commit { commit, .. } => ResolvedCliIdArg::Commit(commit),
            CliId::UncommittedHunkOrFile(uncommitted) => {
                ResolvedCliIdArg::UncommittedHunkOrFile(Box::new(uncommitted))
            }
            CliId::PathPrefix { id, hunks } => ResolvedCliIdArg::PathPrefix { id, hunks },
            CliId::CommittedFile { committed_file, .. } => {
                ResolvedCliIdArg::CommittedFile(committed_file)
            }
            CliId::Uncommitted { .. } => ResolvedCliIdArg::Uncommitted,
            CliId::Stack { id, stack_id } => ResolvedCliIdArg::Stack { id, stack_id },
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
            CliId::Commit {
                commit: CommitId { commit_id, .. },
                ..
            } => Ok(Some(commit_id)),
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
            CliId::Branch(branch) => Ok(Some(BranchArg(branch.name))),
            _ => Ok(None),
        }
    }

    /// TODO: docs
    pub fn try_resolve_uncommitted(
        &self,
        repo: &gix::Repository,
        id_map: &IdMap,
    ) -> CliResult<Option<Vec<UncommittedHunkOrFile>>> {
        let Some(target) = self.try_resolve_uncommitted_id(repo, id_map)? else {
            return Ok(None);
        };
        match target {
            CliId::UncommittedHunkOrFile(uncommitted) => Ok(Some(vec![uncommitted])),
            CliId::PathPrefix { id: _, hunks } => Ok(Some(
                hunks
                    .into_iter()
                    .map(|id_and_hunk| UncommittedHunkOrFile {
                        id: id_and_hunk.id.clone(),
                        hunks: NonEmpty::new(id_and_hunk),
                        // In a world without staging, all these hunks should be turned
                        // into "entire file" IDs for every file under the given PathPrefix.
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

    fn try_resolve_uncommitted_id(
        &self,
        repo: &gix::Repository,
        id_map: &IdMap,
    ) -> CliResult<Option<CliId>> {
        let mut target_ids = id_map
            .parse_uncommitted_using_repo(&self.0, repo)?
            .into_iter()
            .peekable();
        let Some(target) = target_ids.next() else {
            return Ok(None);
        };
        let target = if target_ids.peek().is_none() {
            target
        } else {
            let mut uncommitted = std::iter::once(target)
                .chain(target_ids)
                .filter(|id| matches!(id, CliId::UncommittedHunkOrFile(_)))
                .collect::<Vec<_>>();
            match uncommitted.len() {
                0 => return Ok(None),
                1 => uncommitted.pop().expect("exactly one item"),
                _ => {
                    return Err(bad_input(format!(
                        "Ambiguous uncommitted change '{self}', matches multiple items"
                    ))
                    .hint("Use a longer ID to disambiguate")
                    .into());
                }
            }
        };
        Ok(Some(target))
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
            CliId::Branch(..) => "a branch",
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
    let mut target_ids = arg.parse(repo, id_map)?.into_iter().peekable();
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
                CliId::Branch(..) => branches.push(id),
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
    .hint("Use a longer ID to disambiguate")
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
    /// Prefer uncommitted file and hunk IDs, then preserve any other matching kind so callers can
    /// report a targeted wrong-kind error. Cross-kind priority is not supported for this purpose.
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
    Commit(CommitId),
    Branch(BranchArg),
    UncommittedHunkOrFile(Box<UncommittedHunkOrFile>),
    CommittedFile(CommittedFileId),
    Uncommitted,
    PathPrefix {
        id: String,
        hunks: NonEmpty<IdAndHunk>,
    },
    Stack {
        id: String,
        stack_id: StackId,
    },
}

impl ResolvedCliIdArg {
    /// Convert this into either a branch or a commit.
    pub fn into_branch_or_commit(self) -> CliResult<BranchOrCommit> {
        let kind = match self {
            ResolvedCliIdArg::Commit(commit) => {
                return Ok(BranchOrCommit::Commit(commit));
            }
            ResolvedCliIdArg::Branch(branch) => return Ok(BranchOrCommit::Branch(branch)),
            other => other.kind_for_humans(),
        };
        Err(bad_input(format!("Expected a commit or a branch, got {kind}")).into())
    }

    /// Convert this into a branch or stack.
    pub fn into_branch_or_stack(self) -> CliResult<BranchOrStack> {
        let kind = match self {
            ResolvedCliIdArg::Branch(branch) => return Ok(BranchOrStack::Branch(branch)),
            ResolvedCliIdArg::Stack { id, stack_id } => {
                return Ok(BranchOrStack::Stack { id, stack_id });
            }
            other => other.kind_for_humans(),
        };
        Err(bad_input(format!("Expected a branch or a stack, got {kind}")).into())
    }

    /// Returns a human-readable description of the entity type.
    pub fn kind_for_humans(&self) -> &'static str {
        match self {
            ResolvedCliIdArg::UncommittedHunkOrFile { .. } => "an uncommitted file or hunk",
            ResolvedCliIdArg::PathPrefix { .. } => "a path prefix",
            ResolvedCliIdArg::CommittedFile { .. } => "a committed file",
            ResolvedCliIdArg::Branch { .. } => "a branch",
            ResolvedCliIdArg::Commit { .. } => "a commit",
            ResolvedCliIdArg::Uncommitted => "uncommitted changes",
            ResolvedCliIdArg::Stack { .. } => "a stack",
        }
    }

    /// Convert this into a [`ResolvedCliIdArgRef`].
    pub fn as_ref(&self) -> ResolvedCliIdArgRef<'_> {
        match self {
            ResolvedCliIdArg::Commit(commit) => ResolvedCliIdArgRef::Commit(commit.as_ref()),
            ResolvedCliIdArg::Branch(branch_arg) => ResolvedCliIdArgRef::Branch(&branch_arg.0),
            ResolvedCliIdArg::UncommittedHunkOrFile(hunk) => {
                ResolvedCliIdArgRef::UncommittedHunkOrFile(hunk)
            }
            ResolvedCliIdArg::CommittedFile(committed_file) => {
                ResolvedCliIdArgRef::CommittedFile(committed_file)
            }
            ResolvedCliIdArg::PathPrefix { id, hunks } => {
                ResolvedCliIdArgRef::PathPrefix { id, hunks }
            }
            ResolvedCliIdArg::Uncommitted => ResolvedCliIdArgRef::Uncommitted,
            ResolvedCliIdArg::Stack { id, stack_id } => ResolvedCliIdArgRef::Stack {
                id,
                stack_id: *stack_id,
            },
        }
    }
}

impl PartialEq<CliId> for ResolvedCliIdArg {
    fn eq(&self, other: &CliId) -> bool {
        match self {
            ResolvedCliIdArg::Commit(lhs) => {
                if let CliId::Commit { commit: rhs, .. } = other {
                    return lhs == rhs;
                }
            }
            ResolvedCliIdArg::Branch(lhs) => {
                if let CliId::Branch(rhs) = other {
                    return lhs.0 == rhs.name;
                }
            }
            ResolvedCliIdArg::UncommittedHunkOrFile(lhs) => {
                if let CliId::UncommittedHunkOrFile(rhs) = other {
                    return &**lhs == rhs;
                }
            }
            ResolvedCliIdArg::CommittedFile(lhs) => {
                if let CliId::CommittedFile {
                    committed_file: rhs,
                    ..
                } = other
                {
                    return lhs == rhs;
                }
            }
            ResolvedCliIdArg::Uncommitted => {
                return matches!(other, CliId::Uncommitted { .. });
            }
            ResolvedCliIdArg::PathPrefix {
                id: lhs_id,
                hunks: lhs_hunks,
            } => {
                if let CliId::PathPrefix {
                    id: rhs_id,
                    hunks: rhs_hunks,
                } = other
                {
                    return lhs_id == rhs_id && lhs_hunks == rhs_hunks;
                }
            }
            ResolvedCliIdArg::Stack {
                stack_id: lhs,
                id: _,
            } => {
                if let CliId::Stack {
                    stack_id: rhs,
                    id: _,
                } = other
                {
                    return lhs == rhs;
                }
            }
        }
        false
    }
}

impl std::fmt::Display for ResolvedCliIdArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvedCliIdArg::Commit(commit) => theme::Commit(commit.as_ref()).fmt(f),
            ResolvedCliIdArg::Branch(inner) => inner.fmt(f),
            ResolvedCliIdArg::UncommittedHunkOrFile(..) => f.write_str("uncommitted file or hunk"),
            ResolvedCliIdArg::PathPrefix { .. } => f.write_str("path"),
            ResolvedCliIdArg::CommittedFile(..) => f.write_str("committed file"),
            ResolvedCliIdArg::Uncommitted => f.write_str("uncommitted changes"),
            ResolvedCliIdArg::Stack { .. } => f.write_str("stack"),
        }
    }
}

/// A reference to a [`CliIdArg`] that has actually been resolved.
#[derive(Debug, Clone, Copy)]
#[expect(missing_docs)]
pub enum ResolvedCliIdArgRef<'a> {
    Commit(CommitIdRef<'a>),
    Branch(&'a str),
    UncommittedHunkOrFile(&'a UncommittedHunkOrFile),
    CommittedFile(&'a CommittedFileId),
    PathPrefix {
        id: &'a str,
        hunks: &'a NonEmpty<IdAndHunk>,
    },
    Uncommitted,
    Stack {
        id: &'a str,
        stack_id: StackId,
    },
}

/// Most commands need cli ids that point to either branches or commits.
/// [`ResolvedCliIdArg::into_branch_or_commit`] facilitates that via this enum.
#[derive(Debug, Clone)]
#[expect(missing_docs)]
pub enum BranchOrCommit {
    Commit(CommitId),
    Branch(BranchArg),
}

impl std::fmt::Display for BranchOrCommit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BranchOrCommit::Commit(inner) => theme::Commit(inner.as_ref()).fmt(f),
            BranchOrCommit::Branch(inner) => inner.fmt(f),
        }
    }
}

#[derive(Debug, Clone)]
#[expect(missing_docs)]
pub enum BranchOrStack {
    Branch(BranchArg),
    Stack { id: String, stack_id: StackId },
}
