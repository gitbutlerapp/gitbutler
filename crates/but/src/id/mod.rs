//! CLI ID generation and mapping for GitButler entities.
//!
//! This module provides a system for generating short, human-friendly IDs for various GitButler
//! entities including branches, commits, and files. These IDs are used in the CLI to make commands
//! more convenient and readable than using full SHA-1 hashes or long branch names.

#![forbid(missing_docs)]

use anyhow::bail;
use bstr::{BStr, BString, ByteSlice};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;
use but_workspace::branch::Stack;
use std::{
    borrow::Borrow,
    collections::{BTreeSet, HashMap, HashSet},
};

#[cfg(test)]
mod tests;

/// A helper to indicate that this is a short-id as a user would see.
type ShortId = String;

fn divmod(a: usize, b: usize) -> (usize, usize) {
    (a / b, a % b)
}

/// An integer representation of a [ShortId] that starts with g-z).
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default, Hash)]
struct UintId(u16);
impl UintId {
    /// First character: g-z (20 options)
    const FIRST_CHARS: &'static [u8] = b"ghijklmnopqrstuvwxyz";
    /// Subsequent characters: 0-9,a-z (36 options)
    const SUBSEQUENT_CHARS: &'static [u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    /// Must be less than this.
    const LIMIT: u16 = 20 * 36 * 37;

    /// If self cannot be represented in 3 characters, `00` is returned.
    fn to_short_id(self) -> ShortId {
        let mut result = String::new();

        let (quo, rem) = divmod(self.0 as usize, 20);
        result.push(Self::FIRST_CHARS[rem] as char);
        let (quo, rem) = divmod(quo, 36);
        result.push(Self::SUBSEQUENT_CHARS[rem] as char);
        let (quo, rem) = divmod(quo, 37);
        if quo > 0 {
            // self is too big even for 3 characters.
            return "00".to_string();
        }
        if rem > 0 {
            result.push(Self::SUBSEQUENT_CHARS[rem - 1] as char);
        }

        result
    }
}

/// Lifecycle
impl UintId {
    /// Pick the first 2 to three characters and see if they are a valid `UintId`.
    /// Return `None` for `value` has more than three characters or less than two.
    fn from_name(value: &[u8]) -> Option<Self> {
        let (first_char, second_char, third_char) = match value {
            [a, b] => (a, b, None),
            [a, b, c] => (a, b, Some(c)),
            _ => {
                return None;
            }
        };

        let mut result: usize = 0;

        let index = Self::FIRST_CHARS.iter().position(|e| e == first_char)?;
        result += index;

        let index = Self::SUBSEQUENT_CHARS
            .iter()
            .position(|e| e == second_char)?;
        result += index * 20;

        if let Some(third_char) = third_char {
            let index = Self::SUBSEQUENT_CHARS
                .iter()
                .position(|e| e == third_char)?;
            result += (index + 1) * 20 * 36;
        }

        let result: u16 = result.try_into().expect("below u16::MAX");
        debug_assert!(
            result < Self::LIMIT,
            "BUG: {result} is beyond limit of {}",
            Self::LIMIT
        );
        Some(Self(result))
    }
}

/// A tracker of which [UintId]s have been used.
#[derive(Default, Debug)]
struct IdUsage {
    /// A [UintId] is used if it's in this set.
    uint_ids_used: HashSet<UintId>,
    /// A [UintId] is used if it's less than this number.
    next_uint_id: UintId,
}

impl IdUsage {
    fn mark_used(&mut self, uint_id: UintId) {
        if self.next_uint_id.0 <= uint_id.0 {
            self.uint_ids_used.insert(uint_id);
        }
    }

    fn next_available(&mut self) -> anyhow::Result<UintId> {
        self.forward_next_uint_id_to_not_conflict_with_marked();
        if self.next_uint_id.0 >= UintId::LIMIT {
            bail!("too many IDs");
        }
        let result = self.next_uint_id;
        self.next_uint_id = UintId(self.next_uint_id.0 + 1);
        Ok(result)
    }

    fn forward_next_uint_id_to_not_conflict_with_marked(&mut self) {
        while self.uint_ids_used.remove(&self.next_uint_id) {
            self.next_uint_id = UintId(self.next_uint_id.0 + 1);
        }
    }
}

/// A mapping from user-friendly CLI IDs to GitButler entities.
///
/// # Lifecycle
///
/// 1. Create an `IdMap` for example using [IdMap::new_for_branches_and_commits]
/// 2. Optionally add file information for example using [IdMap::add_file_info]
/// 3. Use [IdMap::resolve_entity_to_ids] to parse user input into matching IDs
/// 4. Use specific methods like [IdMap::resolve_branch_or_insert], [IdMap::resolve_uncommitted_file_or_unassigned],
///    or [IdMap::resolve_file_changed_in_commit_or_unassigned] to get IDs for specific entities
#[derive(Debug)]
pub struct IdMap {
    /// Maps shortened branch names to their assigned CLI IDs
    branch_name_to_cli_id: HashMap<BString, CliId>,
    /// Tracks all non-commit IDs that have been used to avoid collisions
    id_usage: IdUsage,
    /// Commit IDs reachable from workspace tips with their first parent IDs
    workspace_commit_and_first_parent_ids: Vec<(gix::ObjectId, Option<gix::ObjectId>)>,
    /// Commit IDs that are only on the remote
    remote_commit_ids: Vec<gix::ObjectId>,
    /// The ID representing the unassigned area, i.e. uncommitted files that aren't assigned to a stack.
    unassigned: CliId,

    /// Uncommitted files with their assigned IDs
    uncommitted_files: BTreeSet<UncommittedFile>,
    /// Committed files with their assigned IDs
    committed_files: BTreeSet<CommittedFile>,
}

/// Lifecycle methods for creating and initializing `IdMap` instances.
impl IdMap {
    /// Initializes CLI IDs for all *branches* and *commits* in the given `stacks`.
    ///
    /// This method creates a new `IdMap` with IDs for branches and commits only.
    /// To enable parsing of file IDs, call [IdMap::add_file_info_from_context]
    pub fn new_for_branches_and_commits(stacks: &[Stack]) -> anyhow::Result<Self> {
        let StacksInfo {
            branch_names,
            workspace_commit_and_first_parent_ids,
            remote_commit_ids,
        } = get_stacks_info(stacks)?;

        let mut max_zero_count = 1; // Ensure at least two "0" in ID.
        for branch_name in &branch_names {
            for field in branch_name.fields_with(|c| c != '0') {
                max_zero_count = std::cmp::max(field.len(), max_zero_count);
            }
        }
        let (id_usage, branch_name_to_cli_id) = Self::ids_for_branch_names(branch_names)?;
        Ok(Self {
            branch_name_to_cli_id,
            id_usage,
            workspace_commit_and_first_parent_ids,
            remote_commit_ids,
            unassigned: CliId::Unassigned {
                id: str::repeat("0", max_zero_count + 1),
            },
            uncommitted_files: BTreeSet::new(),
            committed_files: BTreeSet::new(),
        })
    }

    /// Scan short `branch_names` in windows of 2 to 3 (presumed) ascii characters and see if
    /// they resemble [`UintId`]s. If so, use them, otherwise, see if they can be used unambiguously
    /// directly. If not, generate an ID.
    fn ids_for_branch_names(
        branch_names: Vec<BString>,
    ) -> anyhow::Result<(IdUsage, HashMap<BString, CliId>)> {
        let mut short_ids_to_count: HashMap<ShortId, u8> = HashMap::new();
        let mut id_usage = IdUsage::default();
        for branch_name in &branch_names {
            for candidate in branch_name.windows(2).chain(branch_name.windows(3)) {
                if let Some(short_id) = UintId::from_name(candidate)
                    .map(|uint_id| {
                        id_usage.mark_used(uint_id);
                        uint_id.to_short_id()
                    })
                    .or_else(|| {
                        // If it's not a valid UintId, it's still acceptable if it
                        // cannot be confused for a commit ID (and is valid UTF-8).
                        if candidate.iter().all(|c| c.is_ascii_alphanumeric())
                            && !candidate.iter().all(|c| c.is_ascii_hexdigit())
                        {
                            String::from_utf8(candidate.to_vec()).ok()
                        } else {
                            None
                        }
                    })
                {
                    short_ids_to_count
                        .entry(short_id)
                        .and_modify(|count| *count = count.saturating_add(1))
                        .or_insert(1);
                }
            }
        }

        let mut branch_name_to_cli_id: HashMap<BString, CliId> = HashMap::new();
        for branch_name in branch_names {
            let id = 'short_id: {
                // Find first non-conflicting pair or triple (i.e. used in
                // exactly one branch) and use it as CliId.
                for candidate in branch_name.windows(2).chain(branch_name.windows(3)) {
                    if let Ok(short_id) = str::from_utf8(candidate)
                        && let Some(1) = short_ids_to_count.get(short_id)
                    {
                        break 'short_id short_id.to_owned();
                    }
                }
                // If none available, use next available ID.
                id_usage.next_available()?.to_short_id()
            };
            let name = branch_name.to_string();
            branch_name_to_cli_id.insert(branch_name, CliId::Branch { name, id });
        }
        Ok((id_usage, branch_name_to_cli_id))
    }

    /// Creates a new instance from `ctx` for more convenience over calling [IdMap::new_for_branches_and_commits].
    pub fn new_from_context(ctx: &Context) -> anyhow::Result<Self> {
        let guard = ctx.shared_worktree_access();
        let meta = ctx.meta(guard.read_permission())?;
        let repo = &*ctx.repo.get()?;
        let head_info = but_workspace::head_info(
            repo,
            &meta,
            but_workspace::ref_info::Options {
                expensive_commit_info: false,
                ..Default::default()
            },
        )?;
        Self::new_for_branches_and_commits(&head_info.stacks)
    }
}

/// Methods for adding context to enable file ID generation for the entities it contains.
impl IdMap {
    /// Adds file information from a `ctx` to add IDs for changed files in the worktree with their stack assignments
    /// and all changed files of all workspace commits.
    ///
    /// After calling this method, [IdMap::resolve_entity_to_ids] will be able to recognize file IDs in addition to branch and commit IDs.
    pub fn add_file_info_from_context(&mut self, ctx: &mut Context) -> anyhow::Result<()> {
        let worktree_dir = ctx.workdir()?;
        let hunk_assignments = if let Some(worktree_dir) = worktree_dir {
            let changes =
                but_core::diff::ui::worktree_changes_by_worktree_dir(worktree_dir)?.changes;
            let (assignments, _assignments_error) =
                but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes), None)?;
            assignments
        } else {
            Vec::new()
        };
        // TODO Fix this, probably by making `assignments_with_fallback` take a
        //      more specific type instead of `ctx`.
        let repo = &*ctx.repo.get()?;
        self.add_file_info(
            |commit_id, parent_id| {
                let tree_changes = but_core::diff::tree_changes(repo, parent_id, commit_id)?;
                Ok(tree_changes
                    .into_iter()
                    .map(|tree_change| tree_change.path)
                    .collect::<Vec<_>>())
            },
            hunk_assignments,
        )
    }

    /// Trigger the generation of IDs for uncommitted and committed files and store them in the map.
    ///
    /// It generates unique 2-character hash-based IDs for each file, ensuring no collisions with existing branch
    /// and commit IDs.
    ///
    /// * `changed_paths_in_commit_fn(commit, parent)` returns the changed file paths for a given commit
    ///   and its parent. Used to identify all files altered by workspace commits.
    /// * `hunk_assignments` - The list of uncommitted files in the worktree with their stack assignments
    fn add_file_info<F>(
        &mut self,
        changed_paths_in_commit_fn: F,
        hunk_assignments: Vec<HunkAssignment>,
    ) -> anyhow::Result<()>
    where
        F: FnMut(gix::ObjectId, Option<gix::ObjectId>) -> anyhow::Result<Vec<BString>>,
    {
        let FileInfo {
            uncommitted_files,
            committed_files,
        } = get_file_info_from_workspace_commits_and_status(
            &self.workspace_commit_and_first_parent_ids,
            changed_paths_in_commit_fn,
            hunk_assignments,
        )?;

        for assignment_path in uncommitted_files.into_iter() {
            self.uncommitted_files.insert(UncommittedFile {
                assignment_path,
                id: self.id_usage.next_available()?.to_short_id(),
            });
        }
        for commit_oid_path in committed_files.into_iter() {
            self.committed_files.insert(CommittedFile {
                commit_oid_path,
                id: self.id_usage.next_available()?.to_short_id(),
            });
        }

        Ok(())
    }
}

/// Methods for parsing and generating CLI IDs.
impl IdMap {
    /// Parses a user-provided `entity` name into matching CLI IDs, with each ID matching a single entity.
    /// Use it if it's completely unclear what `entity` refers to.
    ///
    /// This method attempts to match `entity` against all known entities
    /// in the following priority order:
    ///
    /// 1. Branch names (partial match)
    /// 2. Commit SHA prefixes (if `entity` is hexadecimal)
    /// 3. File IDs (exact 2-character match)
    /// 4. Unassigned area (if input is all zeros)
    ///
    /// Returns a vector of matching [`CliId`]s, with duplicates removed while preserving order.
    /// Multiple IDs may be returned if the entity matches multiple items; they are returned in
    /// priority order as mentioned above.
    pub fn resolve_entity_to_ids(&self, entity: &str) -> anyhow::Result<Vec<CliId>> {
        // If a branch matches exactly, use only that.
        if let Some((_, cli_id)) = self
            .branch_name_to_cli_id
            .iter()
            .find(|(branch_name, _)| *branch_name == entity.as_bytes())
        {
            return Ok(vec![cli_id.clone()]);
        }

        if entity.len() < 2 {
            return Err(anyhow::anyhow!(
                "Id needs to be at least 2 characters long: {}",
                entity
            ));
        }

        let mut matches = Vec::<CliId>::new();

        // First, try partial branch name match
        matches.extend(self.find_branches_by_name(entity.into()).map(Clone::clone));

        // Only try SHA matching if the input looks like a hex string
        if entity
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            && entity.len() >= 2
        {
            for oid in self
                .workspace_and_remote_commit_ids()
                .filter(|oid| oid.to_string().starts_with(entity))
            {
                matches.push(CliId::Commit(*oid));
            }
        }

        // Then try CliId matching
        if entity.len() == 2 {
            if let Some(UncommittedFile {
                assignment_path: (assignment, path),
                ..
            }) = self.uncommitted_files.get(entity)
            {
                matches.push(CliId::UncommittedFile {
                    assignment: *assignment,
                    path: path.to_owned(),
                    id: entity.to_string(),
                });
            }
            if let Some(CommittedFile {
                commit_oid_path: (commit_oid, path),
                ..
            }) = self.committed_files.get(entity)
            {
                matches.push(CliId::CommittedFile {
                    commit_id: *commit_oid,
                    path: path.to_owned(),
                    id: entity.to_string(),
                });
            }
        }
        if entity.find(|c: char| c != '0').is_none() {
            matches.push(self.unassigned().clone());
        }

        // Remove duplicates while preserving order
        let mut unique_matches = Vec::new();
        for m in matches {
            if !unique_matches.contains(&m) {
                unique_matches.push(m);
            }
        }

        Ok(unique_matches)
    }

    /// Returns the [CliId::UncommittedFile] for an uncommitted file as specified by its `assignment`
    /// and repository-relative `path`.
    /// Note that it returns a default ID of `00` as fallback if it
    /// wasn't added via [IdMap::add_file_info_from_context].
    pub fn resolve_uncommitted_file_or_unassigned(
        &self,
        assignment: Option<StackId>,
        path: &BStr,
    ) -> CliId {
        let sought = (assignment, path.to_owned());
        if let Some(UncommittedFile { id, .. }) = self.uncommitted_files.get(&sought) {
            CliId::UncommittedFile {
                assignment: sought.0,
                path: sought.1,
                id: id.to_string(),
            }
        } else {
            CliId::UncommittedFile {
                assignment: sought.0,
                path: sought.1,
                id: "00".to_string(),
            }
        }
    }

    /// Returns the [`CliId::CommittedFile`] for a changed file at repo-relative `path`
    /// that is contained in the `commit_id`.
    /// Note that the returned short id may be `00` as fallback if it wasn't
    /// added by [IdMap::add_file_info_from_context].
    pub fn resolve_file_changed_in_commit_or_unassigned(
        &self,
        commit_id: gix::ObjectId,
        path: &BStr,
    ) -> CliId {
        let sought = (commit_id, path.to_owned());
        if let Some(CommittedFile { id, .. }) = self.committed_files.get(&sought) {
            CliId::CommittedFile {
                commit_id: sought.0,
                path: sought.1,
                id: id.to_string(),
            }
        } else {
            CliId::CommittedFile {
                commit_id: sought.0,
                path: sought.1,
                id: "00".to_string(),
            }
        }
    }

    /// Returns the [`CliId::Branch`] for a branch by its short `name`.
    ///
    /// If the branch already has an assigned ID, return it. Otherwise, returns
    /// `00` as fallback.
    pub fn resolve_branch(&mut self, name: &BStr) -> CliId {
        self.branch_name_to_cli_id
            .get(name)
            .cloned()
            .unwrap_or_else(|| CliId::Branch {
                name: name.to_string(),
                id: "00".to_string(),
            })
    }

    /// Returns the [`CliId::Unassigned`] for the unassigned area, which is useful as an
    /// ID for a destination of operations.
    ///
    /// The unassigned area represents files and changes that are not assigned to any branch.
    /// Its ID is a string of repeated '0' characters, with enough repetitions to ensure
    /// it doesn't collide with any existing branch name.
    pub fn unassigned(&self) -> &CliId {
        &self.unassigned
    }
}

/// Private helper methods for `IdMap`.
impl IdMap {
    /// Finds all branches whose names contain the given `substring`.
    ///
    /// A vector of [`CliId::Branch`] instances for all matching branches.
    fn find_branches_by_name<'a, 's: 'a>(
        &'s self,
        substring: &'a BStr,
    ) -> impl Iterator<Item = &'s CliId> {
        self.branch_name_to_cli_id
            .iter()
            .filter_map(move |(branch_name, cli_id)| {
                branch_name.contains_str(substring).then_some(cli_id)
            })
    }

    /// Returns an iterator over all commit IDs (workspace and remote) known to
    /// this ID map.
    fn workspace_and_remote_commit_ids(&self) -> impl Iterator<Item = &gix::ObjectId> {
        self.workspace_commit_and_first_parent_ids
            .iter()
            .map(|(commit_id, _parent_id)| commit_id)
            .chain(&self.remote_commit_ids)
    }
}

/// A user-friendly CLI ID that identifies a GitButler entity,
/// with each identified by a variant.
///
/// This enum represents the various types of entities that can be identified
/// by short CLI IDs. Each variant contains the necessary information to
/// uniquely identify the entity along with its short ID that one could use
/// to find it.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum CliId {
    /// An uncommitted file in the worktree.
    UncommittedFile {
        /// The stack to which the file is assigned, if any
        assignment: Option<StackId>,
        /// The file path relative to the repository root
        path: BString,
        /// The short CLI ID for this file (typically 2 characters)
        id: ShortId,
    },
    /// A file that exists in a commit.
    CommittedFile {
        /// The object ID of the commit containing the change to the file
        commit_id: gix::ObjectId,
        /// The file path relative to the repository root
        path: BString,
        /// The short CLI ID for this file (typically 2 characters)
        id: ShortId,
    },
    /// A branch.
    Branch {
        /// The short name of the branch, like `main` or `origin/feat`.
        name: String,
        /// The short CLI ID for this branch (typically 2 characters)
        id: ShortId,
    },
    /// A commit in the workspace identified by its SHA.
    Commit(gix::ObjectId),
    /// The unassigned area, as a designated area that files can be put in.
    Unassigned {
        /// The CLI ID for the unassigned area (a string of 2 or more zeros).
        id: ShortId,
    },
}

/// Methods for accessing `CliId` information.
impl CliId {
    /// Returns a human-readable description of the entity type.
    pub fn kind_for_humans(&self) -> &'static str {
        match self {
            CliId::UncommittedFile { .. } => "an uncommitted file",
            CliId::CommittedFile { .. } => "a committed file",
            CliId::Branch { .. } => "a branch",
            CliId::Commit { .. } => "a commit",
            CliId::Unassigned { .. } => "the unassigned area",
        }
    }

    /// Returns the short ID string for display to users.
    pub fn to_short_string(&self) -> ShortId {
        match self {
            CliId::UncommittedFile { id, .. }
            | CliId::CommittedFile { id, .. }
            | CliId::Branch { id, .. }
            | CliId::Unassigned { id, .. } => id.clone(),
            CliId::Commit(oid) => oid.to_hex_with_len(2).to_string(),
        }
    }
}

/// Information extracted from stacks needed for branch and commit CLI ID generation.
/// It's really just a named return value.
struct StacksInfo {
    /// Shortened branch names in unspecified order.
    branch_names: Vec<BString>,
    /// Commit IDs of commits reachable from workspace tips paired with their
    /// first parent IDs in unspecified order. The parent ID is stored to enable
    /// computing diffs upon an invocation of [IdMap::add_file_info].
    workspace_commit_and_first_parent_ids: Vec<(gix::ObjectId, Option<gix::ObjectId>)>,
    /// Commit IDs that are only reachable from remote-tracking branches (not in workspace).
    remote_commit_ids: Vec<gix::ObjectId>,
}

/// Extracts branch names and commit IDs from the given `stacks`.
fn get_stacks_info(stacks: &[Stack]) -> anyhow::Result<StacksInfo> {
    let mut branch_names: Vec<BString> = Vec::new();
    let mut workspace_commit_and_first_parent_ids: Vec<(gix::ObjectId, Option<gix::ObjectId>)> =
        Vec::new();
    let mut remote_commit_ids: Vec<gix::ObjectId> = Vec::new();
    for stack in stacks {
        for segment in &stack.segments {
            if let Some(ref_info) = &segment.ref_info {
                branch_names.push(ref_info.ref_name.shorten().to_owned());
            }
            for commit in &segment.commits {
                workspace_commit_and_first_parent_ids
                    .push((commit.id, commit.parent_ids.first().cloned()));
            }
            for commit in &segment.commits_on_remote {
                remote_commit_ids.push(commit.id);
            }
        }
    }

    Ok(StacksInfo {
        branch_names,
        workspace_commit_and_first_parent_ids,
        remote_commit_ids,
    })
}

/// Information about files needed for CLI ID generation.
/// It's really just a named return value.
struct FileInfo {
    /// Uncommitted files paired with their stack assignments, ordered by assignment then filename.
    uncommitted_files: Vec<(Option<StackId>, BString)>,
    /// Committed files paired with their commit IDs, ordered by commit ID then filename.
    committed_files: Vec<(gix::ObjectId, BString)>,
}

/// Extracts file information from workspace commits and worktree status.
///
/// This function processes workspace commits to find all changed files in each commit,
/// and combines this with hunk assignment information to identify uncommitted (and
/// possibly assigned) files in the worktree.
fn get_file_info_from_workspace_commits_and_status<F>(
    workspace_commit_and_first_parent_ids: &[(gix::ObjectId, Option<gix::ObjectId>)],
    mut changed_paths_fn: F,
    hunk_assignments: Vec<HunkAssignment>,
) -> anyhow::Result<FileInfo>
where
    F: FnMut(gix::ObjectId, Option<gix::ObjectId>) -> anyhow::Result<Vec<BString>>,
{
    let mut committed_files: Vec<(gix::ObjectId, BString)> = Vec::new();
    for (commit_id, parent_id) in workspace_commit_and_first_parent_ids {
        let changed_paths = changed_paths_fn(*commit_id, *parent_id)?;
        for changed_path in changed_paths {
            committed_files.push((*commit_id, changed_path));
        }
    }

    let mut uncommitted_files: Vec<(Option<StackId>, BString)> = Vec::new();
    for assignment in hunk_assignments {
        uncommitted_files.push((assignment.stack_id, assignment.path_bytes));
    }

    Ok(FileInfo {
        committed_files,
        uncommitted_files,
    })
}

/// Internal representation of an uncommitted file with its CLI ID.
///
/// This structure is used to store uncommitted files in a `BTreeSet` where ordering
/// is determined by the ID field, enabling efficient lookups by both ID and
/// (assignment, path) tuple.
///
/// # Invariant
///
/// For all instances `a` and `b`: `a.cmp(b) == a.id.cmp(&b.id)`
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct UncommittedFile {
    /// The file's stack assignment and path
    assignment_path: (Option<StackId>, BString),
    /// The short CLI ID assigned to this file
    id: ShortId,
}
impl Borrow<(Option<StackId>, BString)> for UncommittedFile {
    fn borrow(&self) -> &(Option<StackId>, BString) {
        &self.assignment_path
    }
}
impl Borrow<str> for UncommittedFile {
    fn borrow(&self) -> &str {
        &self.id
    }
}

/// Internal representation of a committed file with its CLI ID.
///
/// This structure is used to store committed files in a `BTreeSet` where ordering
/// is determined by the ID field, enabling efficient lookups by both ID and
/// (commit_oid, path) tuple.
///
/// # Invariant
///
/// For all instances `a` and `b`: `a.cmp(b) == a.id.cmp(&b.id)`
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CommittedFile {
    /// The file's commit object ID and path
    commit_oid_path: (gix::ObjectId, BString),
    /// The short CLI ID assigned to this file
    id: ShortId,
}

impl Borrow<(gix::ObjectId, BString)> for CommittedFile {
    fn borrow(&self) -> &(gix::ObjectId, BString) {
        &self.commit_oid_path
    }
}
impl Borrow<str> for CommittedFile {
    fn borrow(&self) -> &str {
        &self.id
    }
}
