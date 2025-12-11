//! CLI ID generation and mapping for GitButler entities.
//!
//! This module provides a system for generating short, human-friendly IDs for various GitButler
//! entities including branches, commits, and files. These IDs are used in the CLI to make commands
//! more convenient and readable than using full SHA-1 hashes or long branch names.
//!
//! IDs can be mapped back to their entities in a separate step, and are usually used in a second invocation,
//! which is why they should be as stable as possible even when facing intermediate mutations of the set of
//! entities used to create them.

#![forbid(missing_docs)]

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
    /// Tracks all IDs that have been used to avoid collisions
    ids_used: HashSet<ShortId>,
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
    /// To enable parsing of file IDs, call [IdMap::add_file_info] or
    /// [IdMap::add_file_info_from_context] afterward.
    ///
    /// # Algorithm
    ///
    /// For branches, this method:
    /// 1. Analyzes all branch names to find unique 2-character alphanumeric pairs
    /// 2. Avoids pairs that could be confused with commit SHA prefixes (hex digits)
    /// 3. Falls back to hash-based IDs when no unique pair exists
    /// 4. Generates an unassigned area ID with enough zeros to avoid collisions with previously generated IDs.
    pub fn new_for_branches_and_commits(stacks: &[Stack]) -> anyhow::Result<Self> {
        let mut max_zero_count = 1; // Ensure at least two "0" in ID.
        let StacksInfo {
            branch_names,
            workspace_commit_and_first_parent_ids,
            remote_commit_ids,
        } = get_stacks_info(stacks)?;
        let mut pairs_to_count: HashMap<u16, u8> = HashMap::new();
        fn u8_pair_to_u16(two: [u8; 2]) -> u16 {
            two[0] as u16 * 256 + two[1] as u16
        }
        for branch_name in &branch_names {
            for pair in branch_name.windows(2) {
                let pair: [u8; 2] = pair.try_into()?;
                if !pair[0].is_ascii_alphanumeric() || !pair[1].is_ascii_alphanumeric() {
                    continue;
                }
                let could_collide_with_commits =
                    pair[0].is_ascii_hexdigit() && pair[1].is_ascii_hexdigit();
                if could_collide_with_commits {
                    continue;
                }
                let u16pair = u8_pair_to_u16(pair);
                pairs_to_count
                    .entry(u16pair)
                    .and_modify(|count| *count = count.saturating_add(1))
                    .or_insert(1);
            }
            for field in branch_name.fields_with(|c| c != '0') {
                max_zero_count = std::cmp::max(field.len(), max_zero_count);
            }
        }

        let mut branch_name_to_cli_id: HashMap<BString, CliId> = HashMap::new();
        let mut ids_used: HashSet<String> = HashSet::new();
        'branch_name: for branch_name in branch_names {
            // Find first non-conflicting pair and use it as CliId.
            for pair in branch_name.windows(2) {
                let pair: [u8; 2] = pair.try_into()?;
                let u16pair = u8_pair_to_u16(pair);
                if let Some(1) = pairs_to_count.get(&u16pair) {
                    let name = branch_name.to_string();
                    let id = str::from_utf8(&pair)
                        .expect("if we stored it, it's ascii-alphanum")
                        .to_owned();
                    ids_used.insert(id.clone());
                    branch_name_to_cli_id.insert(branch_name, CliId::Branch { name, id });
                    continue 'branch_name;
                }
            }
        }
        Ok(Self {
            branch_name_to_cli_id,
            ids_used,
            workspace_commit_and_first_parent_ids,
            remote_commit_ids,
            unassigned: CliId::Unassigned {
                id: str::repeat("0", max_zero_count + 1),
            },
            uncommitted_files: BTreeSet::new(),
            committed_files: BTreeSet::new(),
        })
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

        let mut int_hash = 0u64;
        let mut get_next_id = || -> String {
            for _ in 0..20 * 36 {
                let tentative_id = string_hash(int_hash);
                int_hash += 1;
                if !self.ids_used.contains(&tentative_id) {
                    return tentative_id;
                }
            }
            panic!(
                "BUG: we really need a way to indicate we want more characters in the string-hash to support more IDs"
            )
        };

        let uncommitted_files: BTreeSet<_> = uncommitted_files
            .into_iter()
            .map(|assignment_path| UncommittedFile {
                assignment_path,
                id: get_next_id(),
            })
            .collect();

        let committed_files: BTreeSet<_> = committed_files
            .into_iter()
            .map(|commit_oid_path| CommittedFile {
                commit_oid_path,
                id: get_next_id(),
            })
            .collect();

        self.uncommitted_files = uncommitted_files;
        self.committed_files = committed_files;
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
    /// If the branch already has an assigned ID, return it.
    /// Otherwise, it generates a new hash-based ID for the branch and
    /// caches it for future use.
    /// TODO: make sure newly created hash is non-conflicting.
    pub fn resolve_branch_or_insert(&mut self, name: &BStr) -> &CliId {
        self.branch_name_to_cli_id
            .entry(name.to_owned())
            .or_insert_with(|| {
                let name = name.to_string();
                let id = hash(&name);
                CliId::Branch { name, id }
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

    /// Returns an iterator over all workspace commit IDs known to this ID map.
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
    /// Commit IDs of commits reachable from workspace tips paired with their first parent IDs.
    /// The parent ID is stored to enable computing diffs on demand.
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

/// Generates a 2-character hash string from the input string.
fn hash(input: &str) -> String {
    string_hash(int_hash(input))
}

/// Computes a simple integer hash of the input string.
///
/// Uses a basic polynomial rolling hash algorithm for simplicity and speed.
/// This is not a cryptographic hash and is only used for generating short IDs.
fn int_hash(input: &str) -> u64 {
    let mut hash = 0u64;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    hash
}

/// Converts an integer `hash` into a 2-character string ID.
///
/// The generated ID uses:
/// - First character: one of 'g'-'z' (20 options) to avoid hex digit collisions
/// - Second character: one of '0'-'9' or 'a'-'z' (36 options)
///
/// This provides 720 unique combinations while avoiding IDs that could be
/// confused with commit SHA prefixes (which use 'a'-'f' and '0'-'9').
fn string_hash(mut hash: u64) -> String {
    // First character: g-z (20 options)
    let first_chars = "ghijklmnopqrstuvwxyz";
    let first_char = first_chars.chars().nth((hash % 20) as usize).unwrap();
    hash /= 20;

    // Second character: 0-9,a-z (36 options)
    let second_chars = "0123456789abcdefghijklmnopqrstuvwxyz";
    let second_char = second_chars.chars().nth((hash % 36) as usize).unwrap();

    format!("{first_char}{second_char}")
}
