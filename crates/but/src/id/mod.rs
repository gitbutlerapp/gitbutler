//! CLI ID generation and mapping for GitButler entities.
//!
//! This module provides a system for generating short, human-friendly IDs for various GitButler
//! entities including branches, commits, and files. These IDs are used in the CLI to make commands
//! more convenient and readable than using full SHA-1 hashes or long branch names.

#![forbid(missing_docs)]

use std::collections::{BTreeMap, HashMap, HashSet};

use bstr::{BStr, BString, ByteSlice};
use but_core::{ChangeId, ref_metadata::StackId};
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;
use but_workspace::{branch::Stack, ref_info::LocalCommitRelation};
use nonempty::NonEmpty;
use self_cell::self_cell;

use crate::id::{file_info::FileInfo, id_usage::UintId, stacks_info::StacksInfo, uncommitted_info::UncommittedInfo};

mod file_info;
mod id_usage;
mod stacks_info;
mod uncommitted_info;

#[cfg(test)]
mod tests;

/// A helper to indicate that this is a short-id as a user would see.
type ShortId = String;

const UNASSIGNED: &str = "zz";

fn rhs_indexes_from_tree_changes(
    tree_changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<Vec<(u64, but_core::TreeChange)>> {
    let FileInfo { changes } = FileInfo::from_tree_changes(tree_changes)?;
    let mut used_indexes: HashSet<u64> = HashSet::new();

    // First pass: assign indexes for all filenames that look like nonnegative
    // integers
    let changes: Vec<(Option<u64>, Vec<but_core::TreeChange>)> = changes
        .into_iter()
        .map(|(path, changes)| {
            (
                if let Ok(utf8) = str::from_utf8(&path)
                    && let Ok(index) = utf8.parse::<u64>()
                {
                    used_indexes.insert(index);
                    Some(index)
                } else {
                    None
                },
                changes,
            )
        })
        .collect();

    // Second pass: assign indexes for all other filenames
    let mut candidate_index = 0u64;
    let mut tree_changes: Vec<(u64, but_core::TreeChange)> = Vec::new();
    for (assigned_index, changes) in changes {
        let index_to_use = if let Some(index) = assigned_index {
            index
        } else {
            while used_indexes.contains(&candidate_index) {
                candidate_index = candidate_index
                    .checked_add(1)
                    .ok_or_else(|| anyhow::format_err!("commit has more than 2**64 changes"))?;
            }
            let index_to_use = candidate_index;
            candidate_index = candidate_index
                .checked_add(1)
                .ok_or_else(|| anyhow::format_err!("commit has more than 2**64 changes"))?;
            index_to_use
        };
        tree_changes.extend(changes.into_iter().map(move |change| (index_to_use, change)));
    }

    Ok(tree_changes)
}

/// A change in a workspace commit.
#[derive(Debug, Clone)]
pub struct TreeChangeWithId {
    /// The short ID.
    pub short_id: ShortId,
    /// The tree change.
    pub inner: but_core::TreeChange,
}

/// A workspace commit with its short ID.
#[derive(Debug, Clone)]
pub struct WorkspaceCommitWithId {
    /// The short ID.
    pub short_id: ShortId,
    /// The original workspace commit.
    pub inner: but_workspace::ref_info::LocalCommit,
}
impl WorkspaceCommitWithId {
    /// The object ID of the commit.
    pub fn commit_id(&self) -> gix::ObjectId {
        self.inner.inner.id
    }
    /// The ID of the first parent if the commit has parents.
    pub fn first_parent_id(&self) -> Option<gix::ObjectId> {
        self.inner.inner.parent_ids.first().cloned()
    }
    /// State in relation to its remote tracking branch.
    pub fn relation(&self) -> LocalCommitRelation {
        self.inner.relation
    }
}
/// Methods to calculate the short IDs of committed files.
impl WorkspaceCommitWithId {
    /// Calculate the short IDs of all changes in this commit.
    pub fn tree_changes<F>(&self, mut changes_in_commit_fn: F) -> anyhow::Result<Vec<TreeChangeWithId>>
    where
        F: FnMut(gix::ObjectId, Option<gix::ObjectId>) -> anyhow::Result<Vec<but_core::TreeChange>>,
    {
        let rhs_indexes =
            rhs_indexes_from_tree_changes(changes_in_commit_fn(self.commit_id(), self.first_parent_id())?)?;
        Ok(rhs_indexes
            .into_iter()
            .map(|(index, tree_change)| TreeChangeWithId {
                short_id: format!("{}:{}", self.short_id, index),
                inner: tree_change,
            })
            .collect())
    }
    /// Convenience for [WorkspaceCommitWithId::tree_changes] if a
    /// [gix::Repository] is available.
    pub fn tree_changes_using_repo(&self, repo: &gix::Repository) -> anyhow::Result<Vec<TreeChangeWithId>> {
        self.tree_changes(|commit_id, parent_id| but_core::diff::tree_changes(repo, parent_id, commit_id))
    }
}

/// A remote commit with its short ID.
#[derive(Debug, Clone)]
pub struct RemoteCommitWithId {
    /// The short ID.
    pub short_id: ShortId,
    /// The original remote commit.
    pub inner: but_workspace::ref_info::Commit,
}
impl RemoteCommitWithId {
    /// The object ID of the commit.
    pub fn commit_id(&self) -> gix::ObjectId {
        self.inner.id
    }
}

/// A segment with its short ID and commit IDs.
#[derive(Debug, Clone)]
pub struct SegmentWithId {
    /// The short ID.
    pub short_id: ShortId,
    /// True iff `short_id` was generated from scratch (and not from a substring
    /// of the branch name).
    pub is_auto_id: bool,
    /// The original segment except that `commits` and `commits_on_remote` are
    /// blank to save memory.
    pub inner: but_workspace::ref_info::Segment,
    /// The original `inner.commits` with additional information.
    pub workspace_commits: Vec<WorkspaceCommitWithId>,
    /// The original `inner.commits_on_remote` with additional information.
    pub remote_commits: Vec<RemoteCommitWithId>,
}
impl SegmentWithId {
    /// Returns the branch name.
    pub fn branch_name(&self) -> Option<&BStr> {
        self.inner.ref_info.as_ref().map(|ref_info| ref_info.ref_name.shorten())
    }
    /// Returns the linked worktree ID.
    pub fn linked_worktree_id(&self) -> Option<&BStr> {
        if let Some(ref_info) = &self.inner.ref_info
            && let Some(but_graph::Worktree::LinkedId(id)) = &ref_info.worktree
        {
            Some(id.as_bstr())
        } else {
            None
        }
    }
    /// Returns the PR number.
    pub fn pr_number(&self) -> Option<usize> {
        if let Some(metadata) = &self.inner.metadata {
            metadata.review.pull_request
        } else {
            None
        }
    }
}

/// A stack with segment and commit IDs.
#[derive(Debug, Clone)]
pub struct StackWithId {
    /// Same as [Stack::id].
    pub id: Option<StackId>,
    /// Parallel to the original [Stack::segments].
    pub segments: Vec<SegmentWithId>,
}

struct StacksIndexes<'a> {
    workspace_commits: BTreeMap<gix::ObjectId, &'a WorkspaceCommitWithId>,
    remote_commits: BTreeMap<gix::ObjectId, &'a RemoteCommitWithId>,
}
self_cell!(
    struct IndexedStacks {
        owner: Vec<StackWithId>,
        #[covariant]
        dependent: StacksIndexes,
    }
);

/// A mapping from user-friendly CLI IDs to GitButler entities.
pub struct IdMap {
    /// Stacks with indexes into various fields.
    indexed_stacks: IndexedStacks,
    /// Maps shortened branch names to their assigned CLI IDs
    branch_name_to_cli_id: BTreeMap<BString, CliId>,
    /// Maps [ShortId]s of branches if they are autogenerated to CLI IDs.
    /// This will be duplicate at least in parts with `branch_name_to_cli_id`.
    branch_auto_id_to_cli_id: HashMap<ShortId, CliId>,
    /// Mapping from stack IDs to their corresponding stack CLI IDs.
    stack_ids: BTreeMap<StackId, CliId>,
    /// The ID representing the unassigned area, i.e. uncommitted files that aren't assigned to a stack.
    unassigned: CliId,

    /// Maps full reverse hex IDs to uncommitted files.
    /// It's public for convenience in `but rub` currently.
    pub uncommitted_files: BTreeMap<ChangeId, UncommittedFile>,
    /// Uncommitted hunks.
    pub uncommitted_hunks: HashMap<ShortId, UncommittedHunk>,
}

fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b.iter()).take_while(|(a, b)| a == b).count()
}

/// Lifecycle methods for creating and initializing `IdMap` instances.
impl IdMap {
    /// Initializes CLI IDs for branches, commits, and uncommitted
    /// files/hunks.
    pub fn new(stacks: Vec<Stack>, hunk_assignments: Vec<HunkAssignment>) -> anyhow::Result<Self> {
        let UncommittedInfo {
            partitioned_hunks,
            uncommitted_short_filenames,
        } = UncommittedInfo::from_hunk_assignments(hunk_assignments)?;
        let StacksInfo {
            stacks,
            mut id_usage,
            short_ids_to_count,
        } = StacksInfo::new(stacks, &uncommitted_short_filenames)?;

        let mut branch_name_to_cli_id = BTreeMap::new();
        let mut branch_auto_id_to_cli_id = HashMap::new();
        for stack in stacks.iter() {
            for segment in stack.segments.iter() {
                if let Some(branch_name) = segment.branch_name() {
                    let cli_id = CliId::Branch {
                        name: branch_name.to_string(),
                        id: segment.short_id.clone(),
                        stack_id: stack.id,
                    };
                    if segment.is_auto_id {
                        branch_auto_id_to_cli_id.insert(segment.short_id.clone(), cli_id.clone());
                    }
                    branch_name_to_cli_id.insert(branch_name.to_owned(), cli_id);
                }
            }
        }

        let mut uncommitted_files: BTreeMap<ChangeId, UncommittedFile> = BTreeMap::new();
        for hunk_assignments in partitioned_hunks {
            let HunkAssignment {
                path_bytes, stack_id, ..
            } = hunk_assignments.first();
            let reverse_hex = if stack_id.is_none() && path_bytes.iter().all(|c| b'k' <= *c && *c <= b'z') {
                ChangeId::from(path_bytes.to_owned())
            } else {
                let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
                hasher.update(path_bytes);
                if let Some(stack_id) = stack_id {
                    hasher.update(stack_id.0.as_bytes());
                }
                let object_id = hasher.try_finalize()?;
                ChangeId::from_bytes(object_id.as_bytes())
            };
            // Ensure that uncommitted files do not collide with CLI IDs generated after
            if let Some(uint_id) = UintId::from_name(&reverse_hex[..2]) {
                id_usage.mark_used(uint_id);
            }
            if let Some(uint_id) = UintId::from_name(&reverse_hex[..3]) {
                id_usage.mark_used(uint_id);
            }
            uncommitted_files.insert(
                reverse_hex,
                UncommittedFile {
                    short_id: ShortId::default(),
                    hunk_assignments,
                },
            );
            // Skip an ID for stability of other IDs below with respect to older
            // versions of the GitButler CLI.
            id_usage.next_available()?;
        }
        let mut reverse_hex_short_ids: Vec<(ChangeId, Option<&mut ShortId>)> = uncommitted_files
            .iter_mut()
            .map(|(reverse_hex, uncommitted_file)| (reverse_hex.clone(), Some(&mut uncommitted_file.short_id)))
            .collect();
        // Ensure that uncommitted files do not collide with branch substrings
        for short_id in short_ids_to_count.keys() {
            reverse_hex_short_ids.push((ChangeId::from(BString::from(short_id.as_str())), None));
        }
        reverse_hex_short_ids.sort();
        let mut common_with_previous_len = 0;
        let mut remaining = reverse_hex_short_ids.as_mut_slice();
        while let Some(((reverse_hex, short_id), rest)) = remaining.split_first_mut() {
            let common_with_next_len = rest.first().map_or(0, |(next_reverse_hex, _next_short_id)| {
                common_prefix_len(reverse_hex, next_reverse_hex)
            });
            if let Some(short_id) = short_id {
                short_id.push_str(str::from_utf8(
                    &reverse_hex[..(1 + 1.max(common_with_previous_len).max(common_with_next_len))],
                )?);
            }
            common_with_previous_len = common_with_next_len;
            remaining = rest;
        }

        let mut uncommitted_hunks = HashMap::new();
        for uncommitted_file in uncommitted_files.values() {
            for hunk_assignment in uncommitted_file.hunk_assignments.iter() {
                uncommitted_hunks.insert(
                    id_usage.next_available()?.to_short_id(),
                    UncommittedHunk {
                        hunk_assignment: hunk_assignment.clone(),
                    },
                );
            }
        }
        let mut stack_ids = BTreeMap::new();
        for stack in &stacks {
            if let Some(id) = stack.id {
                stack_ids.insert(
                    id,
                    CliId::Stack {
                        id: id_usage.next_available()?.to_short_id(),
                        stack_id: id,
                    },
                );
            }
        }

        let indexed_stacks = IndexedStacks::new(stacks, |stacks| {
            let mut workspace_commits = BTreeMap::new();
            let mut remote_commits = BTreeMap::new();
            for stack in stacks.iter() {
                for segment in stack.segments.iter() {
                    for workspace_commit in segment.workspace_commits.iter() {
                        workspace_commits.insert(workspace_commit.commit_id(), workspace_commit);
                    }
                    for remote_commit in segment.remote_commits.iter() {
                        remote_commits.insert(remote_commit.commit_id(), remote_commit);
                    }
                }
            }

            StacksIndexes {
                workspace_commits,
                remote_commits,
            }
        });

        Ok(Self {
            indexed_stacks,
            branch_name_to_cli_id,
            branch_auto_id_to_cli_id,
            stack_ids,
            unassigned: CliId::Unassigned {
                id: UNASSIGNED.to_string(),
            },
            uncommitted_files,
            uncommitted_hunks,
        })
    }

    /// Creates a new instance from `ctx` for more convenience over calling [IdMap::new].
    ///
    /// # NOTE: claims a read-only workspace lock!
    /// TODO(ctx|ai): make it use perm so the caller keeps the state exclusive/shared over greater periods.
    pub fn new_from_context(ctx: &mut Context, assignments: Option<Vec<HunkAssignment>>) -> anyhow::Result<Self> {
        let meta = ctx.meta()?;
        let context_lines = ctx.settings.context_lines;
        let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;

        let hunk_assignments = match assignments {
            Some(assignments) => assignments,
            None => {
                let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
                let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
                    db.hunk_assignments_mut()?,
                    &repo,
                    &ws,
                    false,
                    Some(changes),
                    None,
                    context_lines,
                )?;
                assignments
            }
        };

        let head_info = but_workspace::head_info(
            &repo,
            &meta,
            but_workspace::ref_info::Options {
                expensive_commit_info: false,
                ..Default::default()
            },
        )?;
        Self::new(head_info.stacks, hunk_assignments)
    }
}

/// Private methods to individually parse what can appear on both side of a
/// colon. (Some of them can also appear alone.)
impl IdMap {
    /// Parses "long" IDs, which take precedence over any "regular" ID.
    fn parse_long_lhs(&self, entity: &str) -> Vec<CliId> {
        // Parse known suffixes.
        if let Some(prefix) = entity.strip_suffix("@{stack}")
            && let Some((_, CliId::Branch { stack_id, .. })) = self
                .branch_name_to_cli_id
                .iter()
                .find(|(branch_name, _)| *branch_name == prefix)
            && let Some(stack_id) = stack_id
        {
            return vec![CliId::Stack {
                id: entity.to_string(),
                stack_id: *stack_id,
            }];
        }

        let mut matches = Vec::<CliId>::new();
        // Branches match if they match exactly.
        if let Some((_, cli_id)) = self
            .branch_name_to_cli_id
            .iter()
            .find(|(branch_name, _)| *branch_name == entity)
        {
            matches.push(cli_id.clone());
        }
        matches
    }
    /// Parses "regular" IDs.
    fn parse_regular_lhs(&self, entity: &str) -> Vec<CliId> {
        let mut matches = Vec::<CliId>::new();

        // Parse known suffixes.
        if let Some(prefix) = entity.strip_suffix("@{stack}") {
            for cli_id in self.find_branches_by_substring_match(prefix.into()) {
                if let CliId::Branch { stack_id, .. } = cli_id
                    && let Some(stack_id) = stack_id
                {
                    matches.push(CliId::Stack {
                        id: entity.to_string(),
                        stack_id: *stack_id,
                    });
                }
            }
        }
        if !matches.is_empty() {
            return matches;
        }

        // First, try partial branch name match
        matches.extend(self.find_branches_by_substring_match(entity.into()).map(Clone::clone));

        // Only try SHA matching if the input looks like a hex string
        if entity.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            && let Ok(prefix) = gix::hash::Prefix::from_hex_nonempty(entity)
        {
            for oid in self
                .workspace_and_remote_commit_ids()
                .filter(|oid| prefix.cmp_oid(oid).is_eq())
            {
                matches.push(CliId::Commit {
                    commit_id: *oid,
                    id: entity.to_owned(),
                });
            }
        }

        // handle stack_ids as well
        if let Some(cli_id) = self
            .stack_ids
            .values()
            .find(|cli_id| matches!(cli_id, CliId::Stack { id, .. } if id == entity))
        {
            matches.push(cli_id.clone());
        }

        // Then try CliId matching
        if let Some(cli_id) = self.branch_auto_id_to_cli_id.get(entity) {
            matches.push(cli_id.clone());
        }
        if entity == UNASSIGNED {
            matches.push(self.unassigned.clone());
        }
        matches
    }
    /// Parses filenames of uncommitted files.
    fn parse_uncommitted_filename_rhs(&self, stack_id: Option<StackId>, entity: &str) -> Vec<CliId> {
        let mut matches = Vec::<CliId>::new();
        for uncommitted_file in self.uncommitted_files.values() {
            let hunk_assignment = uncommitted_file.hunk_assignments.first();
            // TODO once the set of allowed CLI IDs is determined and the
            // access patterns of `uncommitted_files` are known, change its data
            // structure to be more efficient than the current linear search.
            if hunk_assignment.stack_id == stack_id && hunk_assignment.path_bytes == entity.as_bytes() {
                matches.push(uncommitted_file.to_cli_id());
            }
        }
        matches
    }
    /// Parses indexes or filenames of committed files.
    fn parse_committed_filename_rhs<F>(
        &self,
        commit_id: gix::ObjectId,
        first_parent_id: Option<gix::ObjectId>,
        mut changes_in_commit_fn: F,
        lhs: &str,
        rhs: &str,
    ) -> anyhow::Result<Option<CliId>>
    where
        F: FnMut(gix::ObjectId, Option<gix::ObjectId>) -> anyhow::Result<Vec<but_core::TreeChange>>,
    {
        let rhs_indexes = rhs_indexes_from_tree_changes(changes_in_commit_fn(commit_id, first_parent_id)?)?;
        let rhs_u64 = rhs.parse::<u64>().ok();
        for (index, tree_change) in rhs_indexes {
            let is_match = if let Some(rhs_u64) = rhs_u64 {
                index == rhs_u64
            } else {
                tree_change.path == BStr::new(rhs)
            };
            if is_match {
                return Ok(Some(CliId::CommittedFile {
                    commit_id,
                    path: tree_change.path,
                    id: format!("{}:{}", lhs, rhs),
                }));
            }
        }
        Ok(None)
    }
}

/// Methods for parsing and generating CLI IDs.
impl IdMap {
    /// Parses a user-provided `entity` name into matching CLI IDs, with each ID matching a single entity.
    /// Multiple IDs may be returned if the entity matches multiple items.
    ///
    /// Besides generated IDs, this method also accepts filenames, which are
    /// interpreted as uncommitted, unassigned files.
    pub fn parse<F>(&self, entity: &str, mut changes_in_commit_fn: F) -> anyhow::Result<Vec<CliId>>
    where
        F: FnMut(gix::ObjectId, Option<gix::ObjectId>) -> anyhow::Result<Vec<but_core::TreeChange>>,
    {
        if let Some((lhs, rhs)) = entity.split_once(':') {
            let mut lhs_matches = self.parse_long_lhs(lhs);
            if lhs_matches.is_empty() {
                lhs_matches = self.parse_regular_lhs(lhs);
            }
            if lhs_matches.is_empty() {
                return Ok(lhs_matches);
            }
            let mut matches = Vec::new();
            for cli_id in lhs_matches {
                match cli_id {
                    CliId::Unassigned { .. } => {
                        matches.append(&mut self.parse_uncommitted_filename_rhs(None, rhs));
                    }
                    CliId::Stack { stack_id, .. } => {
                        matches.append(&mut self.parse_uncommitted_filename_rhs(Some(stack_id), rhs));
                    }
                    CliId::Commit { commit_id, .. } => {
                        if let Some(workspace_commit) =
                            self.indexed_stacks.borrow_dependent().workspace_commits.get(&commit_id)
                        {
                            matches.extend(self.parse_committed_filename_rhs(
                                workspace_commit.commit_id(),
                                workspace_commit.first_parent_id(),
                                &mut changes_in_commit_fn,
                                lhs,
                                rhs,
                            )?);
                        }
                    }
                    _ => {
                        // TODO: it may be confusing for the user if some IDs
                        // (e.g. branch) silently do not match instead
                        // of an error message being printed.
                        continue;
                    }
                };
            }
            return Ok(matches);
        }

        // Branches match if they match exactly. Likewise for uncommitted, unassigned files.
        let mut matches = self.parse_long_lhs(entity);
        matches.append(&mut self.parse_uncommitted_filename_rhs(None, entity));
        if !matches.is_empty() {
            return Ok(matches);
        }

        if entity.len() < 2 {
            return Err(anyhow::anyhow!("Id needs to be at least 2 characters long: '{entity}'"));
        }

        matches = self.parse_regular_lhs(entity);
        if let Some(uncommitted_hunk) = self.uncommitted_hunks.get(entity) {
            matches.push(CliId::Uncommitted(UncommittedCliId {
                id: entity.to_string(),
                hunk_assignments: NonEmpty::new(uncommitted_hunk.hunk_assignment.clone()),
                is_entire_file: false,
            }));
        }

        // To avoid false positives, only check uncommitted files if nothing
        // else matches. See the uncommitted_files_disambiguate_with_branch()
        // test for an example of the desired behavior (an uncommitted file
        // is assigned the ID "kpr" to avoid ambiguity with a branch with the
        // substring "kp"), so it should not match with "kp".
        if matches.is_empty() {
            let entity_bstring = BString::from(entity);
            for (reverse_hex, uncommitted_file) in
                self.uncommitted_files.range(ChangeId::from(entity_bstring.clone())..)
            {
                if !reverse_hex.starts_with(&entity_bstring) {
                    break;
                }
                matches.push(uncommitted_file.to_cli_id());
            }
        }

        Ok(matches)
    }

    /// Convenience for [IdMap::parse] if a [gix::Repository] is available.
    pub fn parse_using_repo(&self, entity: &str, repo: &gix::Repository) -> anyhow::Result<Vec<CliId>> {
        self.parse(entity, |commit_id, parent_id| {
            but_core::diff::tree_changes(repo, parent_id, commit_id)
        })
    }

    /// Convenience for [IdMap::parse] if a [Context] is available.
    pub fn parse_using_context(&self, entity: &str, ctx: &mut Context) -> anyhow::Result<Vec<CliId>> {
        let repo = &*ctx.repo.get()?;
        self.parse_using_repo(entity, repo)
    }

    /// Returns the [`CliId::Stack`] for a given `stack_id`, if it exists.
    pub fn resolve_stack(&self, stack_id: StackId) -> Option<&CliId> {
        self.stack_ids.get(&stack_id)
    }

    /// Returns the [`CliId::Unassigned`] for the unassigned area, which is useful as an
    /// ID for a destination of operations.
    ///
    /// The unassigned area represents files and changes that are not assigned to any branch.
    pub fn unassigned(&self) -> &CliId {
        &self.unassigned
    }

    /// Returns all known stacks.
    pub fn stacks(&self) -> &Vec<StackWithId> {
        self.indexed_stacks.borrow_owner()
    }
}

/// Private helper methods for `IdMap`.
impl IdMap {
    /// Finds all branches whose names contain the given `substring`.
    ///
    /// A vector of [`CliId::Branch`] instances for all matching branches.
    fn find_branches_by_substring_match<'a, 's: 'a>(&'s self, substring: &'a BStr) -> impl Iterator<Item = &'s CliId> {
        self.branch_name_to_cli_id
            .iter()
            .filter_map(move |(branch_name, cli_id)| branch_name.contains_str(substring).then_some(cli_id))
    }

    /// Returns an iterator over all commit IDs (workspace and remote) known to
    /// this ID map.
    fn workspace_and_remote_commit_ids(&self) -> impl Iterator<Item = &gix::ObjectId> {
        let StacksIndexes {
            workspace_commits,
            remote_commits,
        } = self.indexed_stacks.borrow_dependent();
        workspace_commits.keys().chain(remote_commits.keys())
    }
}

/// An uncommitted file or hunk in the worktree.
#[derive(Debug, Clone)]
pub struct UncommittedCliId {
    /// The short CLI ID for this file (typically 2 characters)
    pub id: ShortId,
    /// The hunk assignments
    pub hunk_assignments: NonEmpty<HunkAssignment>,
    /// `true` if self represents all hunks in a stack-assignment or file pair.
    /// Note that this file may have hunks with other stack assignments.
    pub is_entire_file: bool,
}

impl UncommittedCliId {
    /// Describes self.
    pub fn describe(&self) -> String {
        let hunk_cardinality = if self.is_entire_file {
            if self.hunk_assignments.len() == 1 {
                "the only hunk"
            } else {
                "all hunks"
            }
        } else {
            "a hunk"
        };
        let assignment = if self.hunk_assignments.first().stack_id.is_some() {
            "a stack"
        } else {
            "the unassigned area"
        };
        format!(
            "{hunk_cardinality} in {} in {assignment}",
            self.hunk_assignments.first().path_bytes,
        )
    }
}

/// A user-friendly CLI ID that identifies a GitButler entity,
/// with each identified by a variant.
///
/// This enum represents the various types of entities that can be identified
/// by short CLI IDs. Each variant contains the necessary information to
/// uniquely identify the entity along with its short ID that one could use
/// to find it.
#[derive(Debug, Clone)]
pub enum CliId {
    /// An uncommitted file or hunk in the worktree.
    Uncommitted(UncommittedCliId),
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
        /// The stack ID.
        stack_id: Option<StackId>,
    },
    /// A commit in the workspace identified by its SHA.
    Commit {
        /// The object ID of the commit.
        commit_id: gix::ObjectId,
        /// The short CLI ID, a prefix of the object ID. This prefix is unique
        /// among all commits in all stacks (but not necessarily among all
        /// commits in the repo).
        id: ShortId,
    },
    /// The unassigned area, as a designated area that files can be put in.
    Unassigned {
        /// The CLI ID for the unassigned area.
        id: ShortId,
    },
    /// A stack in the workspace.
    Stack {
        /// The short CLI ID for this stack (typically 2 characters)
        id: ShortId,
        /// The stack ID.
        stack_id: StackId,
    },
}
impl PartialEq for CliId {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Uncommitted(UncommittedCliId { id: l_id, .. }),
                Self::Uncommitted(UncommittedCliId { id: r_id, .. }),
            ) => l_id == r_id,
            (Self::CommittedFile { id: l_id, .. }, Self::CommittedFile { id: r_id, .. }) => l_id == r_id,
            (Self::Branch { id: l_id, .. }, Self::Branch { id: r_id, .. }) => l_id == r_id,
            (Self::Commit { id: l_id, .. }, Self::Commit { id: r_id, .. }) => l_id == r_id,
            (Self::Unassigned { .. }, Self::Unassigned { .. }) => true,
            _ => false,
        }
    }
}
impl Eq for CliId {}

/// Methods for accessing `CliId` information.
impl CliId {
    /// Returns a human-readable description of the entity type.
    pub fn kind_for_humans(&self) -> &'static str {
        match self {
            CliId::Uncommitted { .. } => "an uncommitted file or hunk",
            CliId::CommittedFile { .. } => "a committed file",
            CliId::Branch { .. } => "a branch",
            CliId::Commit { .. } => "a commit",
            CliId::Unassigned { .. } => "the unassigned area",
            CliId::Stack { .. } => "a stack",
        }
    }

    /// Returns the short ID string for display to users.
    pub fn to_short_string(&self) -> ShortId {
        match self {
            CliId::Uncommitted(UncommittedCliId { id, .. })
            | CliId::CommittedFile { id, .. }
            | CliId::Branch { id, .. }
            | CliId::Commit { id, .. }
            | CliId::Stack { id, .. }
            | CliId::Unassigned { id, .. } => id.clone(),
        }
    }
}

/// Internal representation of an uncommitted file.
#[derive(Debug, Clone)]
pub struct UncommittedFile {
    /// The shortest ID that can be used to unambiguously refer to this file.
    pub short_id: ShortId,
    /// Every element has the same [HunkAssignment::stack_id] and [HunkAssignment::path_bytes],
    /// so the first assignment can be used to obtain both.
    pub hunk_assignments: NonEmpty<HunkAssignment>,
}

impl UncommittedFile {
    /// Return the file's stack if it is associated to one, or `None` if the Stack is unknown/has no ID.
    pub fn stack_id(&self) -> Option<StackId> {
        self.hunk_assignments.first().stack_id
    }
    /// The path of the uncommitted file.
    pub fn path(&self) -> &BStr {
        self.hunk_assignments.first().path_bytes.as_ref()
    }
    /// Turn this instance into a [CliId].
    pub fn to_cli_id(&self) -> CliId {
        CliId::Uncommitted(UncommittedCliId {
            hunk_assignments: self.hunk_assignments.clone(),
            id: self.short_id.clone(),
            is_entire_file: true,
        })
    }
}

/// An uncommitted hunk.
#[derive(Debug)]
pub struct UncommittedHunk {
    /// The hunk assignment.
    pub hunk_assignment: HunkAssignment,
}
