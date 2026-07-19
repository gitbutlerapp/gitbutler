//! CLI ID generation and mapping for GitButler entities.
//!
//! This module provides a system for generating short, human-friendly IDs for various GitButler
//! entities including branches, commits, and files. These IDs are used in the CLI to make commands
//! more convenient and readable than using full SHA-1 hashes or long branch names.

#![forbid(missing_docs)]

use std::collections::{BTreeMap, HashMap};
use std::str::{self, FromStr as _};

use bstr::{BStr, BString, ByteSlice};
use but_core::sync::RepoShared;
use but_core::{ChangeId, ref_metadata::StackId};
use but_ctx::Context;
use but_graph::workspace::{Stack, StackCommit, StackSegment};
use but_hunk_assignment::HunkAssignment;
use gix::hash::hasher;
use nonempty::NonEmpty;
use self_cell::self_cell;

use crate::id::{
    file_info::FileInfo, id_usage::UintId, stacks_info::StacksInfo,
    uncommitted_info::UncommittedInfo,
};

mod file_info;
mod id_usage;
pub mod parser;
mod stacks_info;
mod uncommitted_info;

#[cfg(test)]
mod tests;

/// Which namespace an element resolves in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceScope {
    /// Match commits, branches, stacks, and uncommitted files alike.
    Any,
    /// Match only uncommitted files and hunks (plus their explicit containers
    /// `zz`, `dir/`, and `@{stack}`), so commit and branch IDs minted after a
    /// file ID was printed cannot shadow it.
    UncommittedOnly,
}

/// A helper to indicate that this is a short-id as a user would see.
pub(crate) type ShortId = String;

pub(crate) const UNCOMMITTED: &str = "zz";

const INDEX_SEPARATOR: char = '#';

/// The ID of a hunk, without its namespace (file).
#[derive(Debug, Clone, Default)]
struct UnqualifiedHunkId {
    /// The ID of the hunk.
    id: String,
    /// The smallest amount of prefix characters necessary to form a distinct ID.
    min_short_id_chars: usize,
    /// The collision index if there are other hunks with the exact same ID. Otherwise this is
    /// empty.
    collision_index: Option<String>,
}

impl UnqualifiedHunkId {
    fn short_id(&self) -> String {
        let prefix = &self.id[..self.min_short_id_chars];

        match &self.collision_index {
            Some(collision_index) => format!("{prefix}{INDEX_SEPARATOR}{collision_index}"),
            None => prefix.to_string(),
        }
    }

    /// Check if `prefix` matches this ID by non-strict prefix match.
    ///
    /// Note that collision indices are matched separately and must match exactly if provided. For
    /// example, given the full ID `f1#0-2`, all of `f`, `f1`, `f#0-2` and `f1#0-2` are valid
    /// prefixes, but e.g. `f#0` is not as the collision index is only a partial match.
    ///
    /// Similarly, if the full id is `f1` then the prefix `f#0-2` does not match due to the
    /// collision index mismatch.
    ///
    /// It's also not allowed to specify _only_ a collision index (e.g. `#0-2`), that matches
    /// nothing simply because it doesn't seem useful at all, and also clashes a bit semantically
    /// with the file wide hunk indexing.
    ///
    /// These prefixing rules aren't here because they're particularly useful to know about and
    /// utilize. The purpose is to not cause a shortening of an ID to invalidate a longer form of
    /// that ID. As a practical example of where that would be inconvenient, consider the case where
    /// two hunks have short IDs `f1` and `f2`, the second character necessary to disambiguate from
    /// the other hunk. If one were to run a `but status` to show these IDs and then commit hunk
    /// `f1`, the ID for the other hunk would be shortened `f2 -> f`. If you then try to reference
    /// it with ID `f2`, we still want that to work so you don't have to run repeated `but status`
    /// commands.
    ///
    /// The reverse problem still exists, where the addition of new hunks can cause existing short
    /// IDs to become ambiguous. That's not a solvable problem, we'll simply get more hits on the
    /// same prefix.
    fn matches_prefix(&self, prefix: &str) -> bool {
        let (prefix_id, prefix_collision_index) = match prefix.split_once(INDEX_SEPARATOR) {
            Some((prefix_id, prefix_collision_index)) => (prefix_id, Some(prefix_collision_index)),
            None => (prefix, None),
        };

        // We only care about matching against the collision index if the user provided it
        let collision_index_mismatch = prefix_collision_index.is_some()
            && self.collision_index.as_deref() != prefix_collision_index;

        !prefix_id.is_empty() && self.id.starts_with(prefix_id) && !collision_index_mismatch
    }
}

/// Create a CLI ID for the given staged file (if `stack_id` is `Some`) or the
/// given unstaged file or committed file (if `stack_id` is `None`).
fn create_reverse_hex_id(
    path_bytes: &[u8],
    stack_id: Option<&StackId>,
) -> anyhow::Result<ChangeId> {
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
    hasher.update(path_bytes);
    if let Some(stack_id) = stack_id {
        hasher.update(stack_id.0.as_bytes());
    }
    let object_id = hasher.try_finalize()?;
    let mut change_id = ChangeId::from_bytes(object_id.as_bytes());
    if stack_id.is_none() && path_bytes.iter().all(|c| b'k' <= *c && *c <= b'z') {
        change_id.prefix_with(path_bytes.iter().copied());
    }
    Ok(change_id)
}

/// Assign short IDs to each `Some` entry such that they are unambiguous with respect to every other
/// entry.
///
/// `None` entries represent reserved IDs that cannot be changed, such as filenames. They are only
/// there to cause disambiguation with other IDs that we can change. There is currently no mechanism
/// to deal with multiple colliding `None` entries. These will simply slip through.
///
/// Short IDs are disambiguated in two ways:
///
/// 1. By lengthening if the IDs match by prefix but are not identical.
/// 2. By appending an index for collisions.
///
/// These two mechanisms interact. For example, given three IDs 123, 123 and 132, the output short
/// IDs will be 12#0, 12#1 and 13. 123 and 132 are disambiguated against each other by lengthening
/// both, and the duplicate 123 entries are then internally disambiguated by index.
///
/// Note that the algorithm only guarantees that the exact short IDs are distinct. Prefix matching
/// with short IDs is therefore only advisable if the underlying IDs are known to not have cases
/// where one ID is a prefix of another ID.
///
/// TODO Index disambiguation should be structured data. Right now we just append it to the short ID
/// as a string, but that is rather inconvenient when it comes to rendering and matching. The
/// [`ShortId`] type needs to carry collision information in the same way that [`UnqualifiedHunkId`]
/// does.
fn assign_short_ids(
    reverse_hex_short_ids: BTreeMap<ChangeId, Vec<Option<&mut ShortId>>>,
) -> anyhow::Result<()> {
    let mut common_with_previous_len = 0;
    let mut reverse_hex_short_ids: Vec<_> = reverse_hex_short_ids.into_iter().collect();
    let mut remaining = reverse_hex_short_ids.as_mut_slice();

    while let Some(((reverse_hex, short_ids), rest)) = remaining.split_first_mut() {
        // TODO should compare UTF8 chars instead of bytes once we start putting full branch names
        // in here. Otherwise we risk splitting in the middle of a UTF8 character.
        let common_with_next_len = rest
            .first()
            .map_or(0, |(next_reverse_hex, _next_short_id)| {
                common_prefix_len(reverse_hex, next_reverse_hex)
            });

        let min_disambiguation_len = 1 + common_with_previous_len.max(common_with_next_len);

        let num_conflicting_ids = short_ids.len();
        for (i, short_id) in short_ids.iter_mut().flatten().enumerate() {
            short_id.clear();

            let reverse_hex_utf8 = str::from_utf8(reverse_hex)?;
            if min_disambiguation_len > reverse_hex.len() {
                short_id.push_str(reverse_hex_utf8);
            } else {
                short_id.push_str(str::from_utf8(&reverse_hex[..min_disambiguation_len])?);
            }

            if num_conflicting_ids > 1 {
                short_id.push(INDEX_SEPARATOR);
                short_id.push_str(&i.to_string());
            }
        }
        common_with_previous_len = common_with_next_len;
        remaining = rest;
    }
    Ok(())
}

fn short_ids_from_tree_changes(
    tree_changes: Vec<but_core::TreeChange>,
) -> anyhow::Result<Vec<(NonEmpty<but_core::TreeChange>, ChangeId, ShortId)>> {
    let FileInfo { changes } = FileInfo::from_tree_changes(tree_changes)?;
    let mut short_ids: Vec<(NonEmpty<but_core::TreeChange>, ChangeId, ShortId)> = Vec::new();
    for (path, changes) in changes {
        short_ids.push((
            changes,
            create_reverse_hex_id(&path, None)?,
            ShortId::default(),
        ));
    }
    let mut reverse_hex_short_ids = BTreeMap::<ChangeId, Vec<_>>::new();

    for (_, reverse_hex, short_id) in short_ids.iter_mut() {
        reverse_hex_short_ids
            .entry(reverse_hex.clone())
            .or_default()
            .push(Some(short_id));
    }

    assign_short_ids(reverse_hex_short_ids)?;
    Ok(short_ids)
}

type ChangesInCommitFn<'a> = Box<
    dyn FnMut(gix::ObjectId, Option<gix::ObjectId>) -> anyhow::Result<Vec<but_core::TreeChange>>
        + 'a,
>;
trait Node<'a>: std::fmt::Debug {
    fn parse(
        self: Box<Self>,
        element: &str,
        id_map: &'a IdMap,
        changes_in_commit_fn: &mut ChangesInCommitFn<'a>,
    ) -> anyhow::Result<Vec<Box<dyn Node<'a> + 'a>>>;

    fn to_cli_id(self: Box<Self>, short_id: &str, id_map: &IdMap) -> anyhow::Result<Option<CliId>>;
}

#[derive(Debug)]
struct Leaf {
    cli_id: CliId,
}
impl<'a> Node<'a> for Leaf {
    fn parse(
        self: Box<Self>,
        _element: &str,
        _id_map: &'a IdMap,
        _changes_in_commit_fn: &mut ChangesInCommitFn<'a>,
    ) -> anyhow::Result<Vec<Box<dyn Node<'a> + 'a>>> {
        Ok(Vec::new())
    }

    fn to_cli_id(
        self: Box<Self>,
        _short_id: &str,
        _id_map: &IdMap,
    ) -> anyhow::Result<Option<CliId>> {
        Ok(Some(self.cli_id.clone()))
    }
}

/// A change in a workspace commit.
#[derive(Debug, Clone)]
pub struct TreeChangeWithId {
    /// The short ID.
    pub short_id: ShortId,
    /// The tree change.
    pub inner: but_core::TreeChange,
}

/// A change ID with an accompanying distinct short ID
#[derive(Debug, Clone)]
pub struct ChangeIdWithShortId {
    /// The full change ID
    pub change_id: ChangeId,
    /// The shortened version of [`Self::change_id`]
    pub short_id: ShortId,
}

/// The minimum number of change ID characters displayed for a commit, so that
/// short IDs remain visually distinctive.
pub(crate) const MIN_DISPLAYED_CHANGE_ID_CHARS: usize = 3;

impl ChangeIdWithShortId {
    /// The short ID padded with further change ID characters to
    /// [`MIN_DISPLAYED_CHANGE_ID_CHARS`], matching how the change ID is
    /// displayed on commit lines.
    pub fn padded_short_id(&self) -> String {
        let mut id = self.short_id.clone();
        let full = self.change_id.to_string();
        if let Some(padding) = full.get(id.len()..MIN_DISPLAYED_CHANGE_ID_CHARS.min(full.len())) {
            id.push_str(padding);
        }
        id
    }
}

impl From<ChangeId> for ChangeIdWithShortId {
    fn from(value: ChangeId) -> Self {
        Self {
            short_id: ShortId::default(),
            change_id: value,
        }
    }
}

/// A workspace commit with its short ID.
#[derive(Debug, Clone)]
pub struct WorkspaceCommitWithId {
    /// The short ID.
    pub short_id: ShortId,
    /// The change ID
    pub change_id: Option<ChangeIdWithShortId>,
    /// The original workspace commit.
    pub inner: StackCommit,
}

impl WorkspaceCommitWithId {
    /// The object ID of the commit.
    pub fn commit_id(&self) -> gix::ObjectId {
        self.inner.id
    }
    /// The ID of the first parent if the commit has parents.
    pub fn first_parent_id(&self) -> Option<gix::ObjectId> {
        self.inner.parent_ids.first().cloned()
    }
}
/// Methods to calculate the short IDs of committed files.
impl WorkspaceCommitWithId {
    /// Calculate the short IDs of all changes in this commit.
    pub fn tree_changes<F>(
        &self,
        mut changes_in_commit_fn: F,
    ) -> anyhow::Result<Vec<TreeChangeWithId>>
    where
        F: FnMut(gix::ObjectId, Option<gix::ObjectId>) -> anyhow::Result<Vec<but_core::TreeChange>>,
    {
        let rhs_indexes = short_ids_from_tree_changes(changes_in_commit_fn(
            self.commit_id(),
            self.first_parent_id(),
        )?)?;
        Ok(rhs_indexes
            .into_iter()
            .flat_map(|(changes, _change_id, short_id)| {
                changes.into_iter().map(move |change| TreeChangeWithId {
                    short_id: format!(
                        "{}:{}",
                        self.change_id
                            .as_ref()
                            .map(|cid| &cid.short_id)
                            .unwrap_or(&self.short_id),
                        short_id.clone()
                    ),
                    inner: change,
                })
            })
            .collect())
    }
    /// Convenience for [WorkspaceCommitWithId::tree_changes] if a
    /// [gix::Repository] is available.
    pub fn tree_changes_using_repo(
        &self,
        repo: &gix::Repository,
    ) -> anyhow::Result<Vec<TreeChangeWithId>> {
        self.tree_changes(|commit_id, parent_id| {
            but_core::diff::tree_changes(repo, parent_id, commit_id)
        })
    }
}
impl<'a> Node<'a> for &'a WorkspaceCommitWithId {
    fn parse(
        self: Box<Self>,
        element: &str,
        _id_map: &'a IdMap,
        changes_in_commit_fn: &mut ChangesInCommitFn<'a>,
    ) -> anyhow::Result<Vec<Box<dyn Node<'a> + 'a>>> {
        let mut matches = Vec::<Box<dyn Node<'a> + 'a>>::new();
        let rhs_indexes = short_ids_from_tree_changes(changes_in_commit_fn(
            self.commit_id(),
            self.first_parent_id(),
        )?)?;
        for (tree_changes, change_id, short_id) in rhs_indexes {
            let is_match = change_id.starts_with(element.as_bytes())
                || tree_changes.first().path == BStr::new(element);
            if is_match {
                matches.push(Box::new(Leaf {
                    cli_id: CliId::CommittedFile {
                        commit_id: self.commit_id(),
                        path: tree_changes.first().path.clone(),
                        id: format!(
                            "{}:{}",
                            self.change_id
                                .as_ref()
                                .map(|cid| &cid.short_id)
                                .unwrap_or(&self.short_id),
                            short_id
                        ),
                    },
                }));
            }
        }
        Ok(matches)
    }

    fn to_cli_id(
        self: Box<Self>,
        _short_id: &str,
        _id_map: &IdMap,
    ) -> anyhow::Result<Option<CliId>> {
        Ok(Some(CliId::Commit {
            commit_id: self.commit_id(),
            id: self.short_id.clone(),
            change_id: self.change_id.as_ref().map(|id| id.change_id.clone()),
        }))
    }
}

/// A remote commit with its short ID.
#[derive(Debug, Clone)]
pub struct RemoteCommitWithId {
    /// The short ID.
    pub short_id: ShortId,
    /// The original remote commit.
    pub inner: StackCommit,
}
impl RemoteCommitWithId {
    /// The object ID of the commit.
    pub fn commit_id(&self) -> gix::ObjectId {
        self.inner.id
    }
}
impl<'a> Node<'a> for &'a RemoteCommitWithId {
    fn parse(
        self: Box<Self>,
        _element: &str,
        _id_map: &'a IdMap,
        _changes_in_commit_fn: &mut ChangesInCommitFn<'a>,
    ) -> anyhow::Result<Vec<Box<dyn Node<'a> + 'a>>> {
        Ok(Vec::new())
    }

    fn to_cli_id(
        self: Box<Self>,
        _short_id: &str,
        _id_map: &IdMap,
    ) -> anyhow::Result<Option<CliId>> {
        Ok(Some(CliId::Commit {
            commit_id: self.commit_id(),
            id: self.short_id.clone(),
            change_id: None,
        }))
    }
}

/// A segment with its short ID and commit IDs.
#[derive(Debug, Clone)]
pub struct SegmentWithId {
    /// The short ID.
    pub short_id: ShortId,
    /// The original segment except that `commits` and `commits_on_remote` are
    /// blank to save memory.
    pub inner: StackSegment,
    /// The original `inner.commits` with additional information.
    pub workspace_commits: Vec<WorkspaceCommitWithId>,
    /// The original `inner.commits_on_remote` with additional information.
    pub remote_commits: Vec<RemoteCommitWithId>,
    /// Backreference to the ID of the stack that this segment belongs to, for
    /// workflows that refer to a stack by the name of one of its constituent
    /// segments.
    pub stack_id: Option<StackId>,
}
impl SegmentWithId {
    /// Returns the branch name.
    pub fn branch_name(&self) -> Option<&BStr> {
        self.inner
            .ref_info
            .as_ref()
            .map(|ref_info| ref_info.ref_name.shorten())
    }
    /// Returns the linked worktree ID.
    pub fn linked_worktree_id(&self) -> Option<&BStr> {
        if let Some(ref_info) = &self.inner.ref_info
            && let Some(worktree) = &ref_info.worktree
            && let but_graph::WorktreeKind::LinkedId(id) = &worktree.kind
        {
            Some(id.as_bstr())
        } else {
            None
        }
    }
}
impl<'a> Node<'a> for &'a SegmentWithId {
    fn parse(
        self: Box<Self>,
        _element: &str,
        _id_map: &'a IdMap,
        _changes_in_commit_fn: &mut ChangesInCommitFn<'a>,
    ) -> anyhow::Result<Vec<Box<dyn Node<'a> + 'a>>> {
        // TODO: it may be confusing for the user if `branch_id:something`
        // silently does not match instead of an error message being printed.
        Ok(Vec::new())
    }

    fn to_cli_id(
        self: Box<Self>,
        _short_id: &str,
        _id_map: &IdMap,
    ) -> anyhow::Result<Option<CliId>> {
        Ok(Some(CliId::Branch {
            name: self.branch_name().unwrap_or_default().to_string(),
            id: self.short_id.clone(),
            stack_id: self.stack_id,
        }))
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
impl<'a> Node<'a> for &'a StackWithId {
    fn parse(
        self: Box<Self>,
        element: &str,
        id_map: &'a IdMap,
        _changes_in_commit_fn: &mut ChangesInCommitFn<'a>,
    ) -> anyhow::Result<Vec<Box<dyn Node<'a> + 'a>>> {
        // Parse known suffixes.
        if element.ends_with('/') {
            return Ok(id_map.parse_uncommitted_path_prefix(self.id, element));
        }
        for uncommitted_file in id_map.uncommitted_files.values() {
            let hunk_assignments = uncommitted_file.hunk_assignments();
            let hunk_assignment = hunk_assignments.first();
            // TODO once the set of allowed CLI IDs is determined and the
            // access patterns of `uncommitted_files` are known, change its data
            // structure to be more efficient than the current linear search.
            if hunk_assignment.stack_id == self.id
                && hunk_assignment.path_bytes == element.as_bytes()
            {
                return Ok(vec![Box::new(uncommitted_file)]);
            }
        }
        Ok(Vec::new())
    }

    fn to_cli_id(
        self: Box<Self>,
        short_id: &str,
        _id_map: &IdMap,
    ) -> anyhow::Result<Option<CliId>> {
        let Some(stack_id) = self.id else {
            return Ok(None);
        };
        Ok(Some(CliId::Stack {
            id: short_id.to_owned(),
            stack_id,
        }))
    }
}

struct StacksIndexes<'a> {
    // This is left here in case we need indexes in the future. (If we don't, we
    // can delete this.)
    _dummy: &'a Vec<StackWithId>,
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
    /// Mapping from stack IDs to their corresponding stack CLI IDs.
    stack_ids: BTreeMap<StackId, CliId>,
    /// The ID representing the uncommitted area, i.e. uncommitted files that aren't assigned to a stack.
    uncommitted: CliId,

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
    pub fn new(
        stacks: Vec<Stack>,
        hunk_assignments: Vec<HunkAssignment>,
        commit_id_to_change_id: gix::hashtable::HashMap<gix::ObjectId, ChangeId>,
    ) -> anyhow::Result<Self> {
        let UncommittedInfo {
            partitioned_hunks,
            uncommitted_short_filenames,
        } = UncommittedInfo::from_hunk_assignments(hunk_assignments)?;
        let StacksInfo {
            mut stacks,
            mut id_usage,
            non_hex_used_short_ids,
        } = StacksInfo::new(
            stacks,
            &uncommitted_short_filenames,
            &commit_id_to_change_id,
        )?;

        let mut uncommitted_files: BTreeMap<ChangeId, UncommittedFile> = BTreeMap::new();
        for hunk_assignments in partitioned_hunks {
            let HunkAssignment {
                path_bytes,
                stack_id,
                ..
            } = hunk_assignments.first();
            let reverse_hex = create_reverse_hex_id(path_bytes, stack_id.as_ref())?;
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
                    short_id_hunk_assignments: hunk_assignments
                        .map(|hunk_assignment| (UnqualifiedHunkId::default(), hunk_assignment)),
                },
            );
            // Skip an ID for stability of other IDs below with respect to older
            // versions of the GitButler CLI.
            id_usage.next_available()?;
        }
        let mut reverse_hex_short_ids: Vec<(ChangeId, Option<&mut ShortId>)> = uncommitted_files
            .iter_mut()
            .map(|(reverse_hex, uncommitted_file)| {
                (reverse_hex.clone(), Some(&mut uncommitted_file.short_id))
            })
            .collect();
        // Ensure that uncommitted file revers hexes do not collide short IDs that have already been allocated
        //
        // TODO The raw filenames of the uncommitted files are in these non_hex_used_short_ids, which means
        // that a file that is its own reverse hex ID collides with itself and forces an unnecessary
        // extension. E.g. the file "out" gets the short ID "outk", which seems pretty redundant as
        // there is no ambiguity if both IDs point to the same thing.
        for short_id in non_hex_used_short_ids {
            reverse_hex_short_ids.push((ChangeId::from(BString::from(short_id.as_str())), None));
        }

        for change_id in stacks
            .iter_mut()
            .flat_map(|stack| stack.segments.iter_mut())
            .flat_map(|segment| {
                segment
                    .workspace_commits
                    .iter_mut()
                    .filter_map(|c| c.change_id.as_mut())
            })
        {
            reverse_hex_short_ids.push((change_id.change_id.clone(), Some(&mut change_id.short_id)))
        }

        let mut mapped_reverse_hex_short_ids =
            BTreeMap::<ChangeId, Vec<Option<&mut ShortId>>>::new();
        for (id, short_id) in reverse_hex_short_ids {
            mapped_reverse_hex_short_ids
                .entry(id)
                .or_default()
                .push(short_id);
        }
        assign_short_ids(mapped_reverse_hex_short_ids)?;

        let mut uncommitted_hunks = HashMap::new();
        for uncommitted_file in uncommitted_files.values_mut() {
            {
                Self::assign_content_based_hunk_ids(
                    uncommitted_file.short_id_hunk_assignments.iter_mut(),
                )?;
            }

            for (hunk_id, hunk_assignment) in &uncommitted_file.short_id_hunk_assignments {
                uncommitted_hunks.insert(
                    format!("{}:{}", uncommitted_file.short_id, hunk_id.short_id()),
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

        let indexed_stacks = IndexedStacks::new(stacks, |stacks| StacksIndexes { _dummy: stacks });

        Ok(Self {
            indexed_stacks,
            stack_ids,
            uncommitted: CliId::Uncommitted {
                id: UNCOMMITTED.to_string(),
            },
            uncommitted_files,
            uncommitted_hunks,
        })
    }

    const HUNK_EMPTY_CONTENT_PREFIX: &str = "q";

    /// Assign unique hunk IDs based on content hash into the input iterator's first element.
    ///
    /// On hash collisions, the IDs are disambiguated based on the amount of collisions for that
    /// particular hash.
    ///
    /// Hunks that lack a diff get a content hash of "q" and then rely on the disambiguation
    /// mechanism if there are multiple such hunks.
    ///
    /// In summary, the formats are:
    ///
    /// * No collision: `<prefix>`
    ///     - Example: `0`
    /// * Collision: `<prefix>#<collision_index>-<num_collisions>`
    ///     - `<collision_index>` is the 0-based index of the hunk relative to the other colliding
    ///       hunks, in input order.
    ///     - Example: `0#0-2` and `0#1-2`
    ///
    /// We include the number of collisions in the collision disambiguation to prevent
    /// commit/discard of a leading colliding hunk from causing trailing colliding hunks to get new
    /// IDs that previously pointed to other colliding hunks.
    ///
    /// Note that hunks that lack a diff follow the same rules, only that `<prefix>="q"` always.
    fn assign_content_based_hunk_ids<'a>(
        short_ids_and_hunks: impl Iterator<Item = &'a mut (UnqualifiedHunkId, HunkAssignment)>,
    ) -> anyhow::Result<()> {
        let mut content_hash_to_short_ids: BTreeMap<String, Vec<&'a mut UnqualifiedHunkId>> =
            BTreeMap::new();
        for (hunk_id, hunk_assignment) in short_ids_and_hunks {
            let content_hash = match hunk_assignment.diff.as_ref() {
                Some(diff) => {
                    let mut content_hasher = hasher(gix::hash::Kind::Sha1);
                    for line in diff
                        .lines_with_terminator()
                        .filter(|line| !line.starts_with_str(b"@@"))
                    {
                        content_hasher.update(line);
                    }
                    content_hasher.try_finalize()?.to_string()
                }
                None => Self::HUNK_EMPTY_CONTENT_PREFIX.to_string(),
            };

            content_hash_to_short_ids
                .entry(content_hash)
                .or_default()
                .push(hunk_id);
        }

        let mut all_hashes = content_hash_to_short_ids.into_iter();

        let mut current = all_hashes.next();
        let mut len_in_common_with_last: usize = 0;
        while let Some((content_hash, mut ids)) = current {
            let next = all_hashes.next();

            let len_in_common_with_next = common_prefix_len(
                content_hash.as_bytes(),
                next.as_ref()
                    .map(|(content_hash, _)| content_hash.as_bytes())
                    .unwrap_or_default(),
            );
            let min_short_id_chars = 1
                .max(len_in_common_with_next + 1)
                .max(len_in_common_with_last + 1);

            let num_colliding_ids = ids.len();
            for (i, hunk_id) in ids.iter_mut().enumerate() {
                hunk_id.id.push_str(&content_hash);
                hunk_id.min_short_id_chars = min_short_id_chars;

                if num_colliding_ids > 1 {
                    hunk_id.collision_index = Some(format!("{i}-{num_colliding_ids}"))
                }
            }

            len_in_common_with_last = len_in_common_with_next;
            current = next;
        }

        Ok(())
    }

    /// Creates a new instance from `ctx` for more convenience over calling [IdMap::new].
    ///
    /// # NOTE: claims a read-only workspace lock!
    // TODO(ctx|ai): make it use perm so the caller keeps the state exclusive/shared over greater periods.
    // Use `new_from_context` instead - it takes `perm`, and forces you to think about repository locks
    // in the light of mutations.
    pub fn legacy_new_from_context(
        ctx: &Context,
        assignments: Option<Vec<HunkAssignment>>,
    ) -> anyhow::Result<Self> {
        let guard = ctx.shared_worktree_access();
        Self::new_from_context(ctx, assignments, guard.read_permission())
    }

    ///
    /// Creates a new instance from `ctx` for more convenience over calling [IdMap::new].
    /// `perm` is needed to obtain a read-only workspace.
    ///
    /// # NOTE: claims a read-only workspace lock!
    /// TODO(ctx|ai): Use a `ws` directly instead of creating a whole new RefInfo uncached.
    pub fn new_from_context(
        ctx: &Context,
        assignments: Option<Vec<HunkAssignment>>,
        perm: &RepoShared,
    ) -> anyhow::Result<Self> {
        let context_lines = ctx.settings.context_lines;
        let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(perm)?;

        let hunk_assignments = match assignments {
            Some(assignments) => assignments,
            None => {
                let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
                let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
                    db.hunk_assignments_mut()?,
                    &repo,
                    &ws,
                    Some(changes),
                    context_lines,
                )?;
                assignments
            }
        };

        let commit_ids = ws
            .stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .flat_map(|segment| segment.commits.iter())
            .map(|c| c.id);

        let commit_id_to_change_id = commit_ids
            .filter_map(|commit_id| {
                let result = (|| {
                    let commit = repo.find_commit(commit_id)?;
                    let commit = commit.decode()?;
                    let change_id = but_core::commit::Headers::try_from_commit_headers(|| {
                        commit.extra_headers()
                    })
                    .and_then(|headers| headers.change_id)
                    .unwrap_or_else(|| {
                        but_core::commit::Headers::synthetic_change_id_from_commit_id(commit_id)
                    });
                    Ok::<(gix::ObjectId, ChangeId), anyhow::Error>((commit_id, change_id))
                })();

                match result {
                    Ok((commit_id, change_id)) => Some((commit_id, change_id)),
                    Err(err) => {
                        tracing::error!(
                            ?commit_id,
                            ?err,
                            "Failed to resolve commit when mapping change IDs"
                        );
                        None
                    }
                }
            })
            .collect();

        Self::new(ws.stacks.clone(), hunk_assignments, commit_id_to_change_id)
    }
}

/// Private methods to individually parse what can appear on both side of a
/// colon. (Some of them can also appear alone.)
impl IdMap {
    fn parse_uncommitted_filename<'a>(
        &'a self,
        stack_id: Option<StackId>,
        element: &str,
    ) -> Vec<Box<dyn Node<'a> + 'a>> {
        let mut matches = Vec::<Box<dyn Node<'a> + 'a>>::new();
        for uncommitted_file in self.uncommitted_files.values() {
            let hunk_assignments = uncommitted_file.hunk_assignments();
            let hunk_assignment = hunk_assignments.first();
            // TODO once the set of allowed CLI IDs is determined and the
            // access patterns of `uncommitted_files` are known, change its data
            // structure to be more efficient than the current linear search.
            if hunk_assignment.stack_id == stack_id
                && hunk_assignment.path_bytes == element.as_bytes()
            {
                matches.push(Box::new(uncommitted_file));
            }
        }
        matches
    }

    fn parse_uncommitted_path_prefix<'a>(
        &'a self,
        stack_id: Option<StackId>,
        element: &str,
    ) -> Vec<Box<dyn Node<'a> + 'a>> {
        let mut hunk_assignments = Vec::new();
        for (short_id, uncommitted_hunk) in self.uncommitted_hunks.iter() {
            let hunk_assignment = &uncommitted_hunk.hunk_assignment;
            if hunk_assignment.stack_id == stack_id
                && hunk_assignment.path_bytes.starts_with(element.as_bytes())
            {
                hunk_assignments.push((short_id.to_owned(), hunk_assignment.to_owned()));
            }
        }
        hunk_assignments.sort_by(|a, b| a.1.path_bytes.cmp(&b.1.path_bytes));
        let Some(hunk_assignments) = NonEmpty::from_vec(hunk_assignments) else {
            return vec![];
        };
        vec![Box::new(Leaf {
            cli_id: CliId::PathPrefix {
                id: element.to_string(),
                hunk_assignments,
            },
        })]
    }

    /// Whether `element` is exactly a branch segment or stack short ID
    /// currently in this map. Unlike commit change IDs (random, minted after
    /// the fact), these are displayed to the user, so a selector equal to one
    /// refers to that entity — never to a file that happens to share the
    /// prefix.
    fn is_displayed_branch_or_stack_id(&self, element: &str) -> bool {
        self.indexed_stacks
            .borrow_owner()
            .iter()
            .flat_map(|stack| stack.segments.iter())
            .any(|segment| segment.short_id == element)
            || self
                .stack_ids
                .values()
                .any(|id| matches!(id, CliId::Stack { id, .. } if id == element))
    }

    /// All uncommitted files whose full reverse-hex ID starts with `element`.
    fn uncommitted_file_id_prefix_matches<'a>(
        &'a self,
        element: &str,
    ) -> Vec<Box<dyn Node<'a> + 'a>> {
        let mut matches = Vec::<Box<dyn Node<'a> + 'a>>::new();
        let element_bstring = BString::from(element);
        for (reverse_hex, uncommitted_file) in self
            .uncommitted_files
            .range(ChangeId::from(element_bstring.clone())..)
        {
            if !reverse_hex.starts_with(&element_bstring) {
                break;
            }
            matches.push(Box::new(uncommitted_file));
        }
        matches
    }

    fn parse_element_scoped<'a>(
        &'a self,
        element: &str,
        scope: SourceScope,
    ) -> anyhow::Result<Vec<Box<dyn Node<'a> + 'a>>> {
        if element.is_empty() {
            return Ok(vec![]);
        }

        // Parse known suffixes.
        if let Some(prefix) = element.strip_suffix("@{stack}") {
            let mut matches = Vec::<Box<dyn Node<'a> + 'a>>::new();
            for stack_with_id in self.indexed_stacks.borrow_owner().iter() {
                for segment_with_id in stack_with_id.segments.iter() {
                    if segment_with_id
                        .branch_name()
                        .is_some_and(|branch_name| branch_name.contains_str(prefix))
                    {
                        matches.push(Box::new(stack_with_id));
                        break;
                    }
                }
            }
            return Ok(matches);
        }
        if element.ends_with('/') {
            return Ok(self.parse_uncommitted_path_prefix(None, element));
        }

        let mut matches = Vec::<Box<dyn Node<'a> + 'a>>::new();

        // Branches match if they match exactly. Likewise for uncommitted, uncommitted files.
        if scope == SourceScope::Any {
            for stack_with_id in self.indexed_stacks.borrow_owner().iter() {
                for segment_with_id in stack_with_id.segments.iter() {
                    if segment_with_id
                        .branch_name()
                        .is_some_and(|branch_name| branch_name == element)
                    {
                        matches.push(Box::new(segment_with_id));
                    }
                }
            }
        }
        matches.extend(self.parse_uncommitted_filename(None, element));

        // The following match only if there have been no matches so far.
        if !matches.is_empty() {
            return Ok(matches);
        }

        if scope == SourceScope::Any {
            self.push_generated_id_matches(element, &mut matches);
        }
        if element == UNCOMMITTED {
            matches.push(Box::new(Unstaged {}));
        }

        // We only match against uncommitted files if there are no other matches. The reason for
        // this is that we want prefix matching for uncommitted files to make the IDs "stable under
        // shortening". However, as the reverse hex IDs of uncommitted files may collide with branch
        // names and/or branch short IDs, we only match against uncommitted files if there are no
        // other matches. This allows branch name short IDs to be prefixes of uncommitted file short
        // IDs — and in the uncommitted scope, where container IDs never match, an element that
        // exactly equals a displayed branch or stack ID deliberately matches nothing rather than
        // resolving to a file it was never shown for.
        if matches.is_empty() {
            if scope == SourceScope::UncommittedOnly
                && self.is_displayed_branch_or_stack_id(element)
            {
                return Ok(vec![]);
            }
            matches = self.uncommitted_file_id_prefix_matches(element);
        }

        Ok(matches)
    }

    /// Commit, stack-ID, and branch-short-ID matches for `element`, appended
    /// to `matches`. Only meaningful in the full namespace.
    fn push_generated_id_matches<'a>(
        &'a self,
        element: &str,
        matches: &mut Vec<Box<dyn Node<'a> + 'a>>,
    ) {
        // Match against commits
        let maybe_element_hex_prefix = if element
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            gix::hash::Prefix::from_hex_nonempty(element).ok()
        } else {
            None
        };

        let element_matches_commit =
            |id: gix::ObjectId, maybe_change_id: Option<&ChangeIdWithShortId>| {
                if let Some(element_hex_prefix) = maybe_element_hex_prefix
                    && element_hex_prefix.cmp_oid(&id).is_eq()
                {
                    return true;
                }

                if let Some(change_id) = maybe_change_id
                    && (change_id.change_id.starts_with_str(element)
                        || element == change_id.short_id)
                {
                    return true;
                }

                false
            };

        for stack_with_id in self.indexed_stacks.borrow_owner().iter() {
            for segment_with_id in stack_with_id.segments.iter() {
                for workspace_commit_with_id in segment_with_id.workspace_commits.iter() {
                    if element_matches_commit(
                        workspace_commit_with_id.commit_id(),
                        workspace_commit_with_id.change_id.as_ref(),
                    ) {
                        matches.push(Box::new(workspace_commit_with_id))
                    }
                }

                for remote_commit_with_id in segment_with_id.remote_commits.iter() {
                    if element_matches_commit(
                        remote_commit_with_id.commit_id(),
                        // We currently do not allow change ID matching against remote commits to
                        // prevent unnecessary ambiguity with local commits. If we do want this
                        // feature in the future, we should probably put remote commit change IDs in
                        // a separate namespace by prefixing something to them.
                        None,
                    ) {
                        matches.push(Box::new(remote_commit_with_id))
                    }
                }
            }
        }

        // handle stack_ids as well
        // TODO: add a ShortId field to StackWithId so that we don't have to do
        // a double lookup
        for cli_id in self.stack_ids.values() {
            if let CliId::Stack { id, stack_id } = cli_id
                && id == element
                && let Some(stack_with_id) = self
                    .indexed_stacks
                    .borrow_owner()
                    .iter()
                    .find(|stack_with_id| stack_with_id.id == Some(*stack_id))
            {
                matches.push(Box::new(stack_with_id));
                break;
            }
        }

        // Then try CliId matching
        for stack_with_id in self.indexed_stacks.borrow_owner().iter() {
            for segment_with_id in stack_with_id.segments.iter() {
                if segment_with_id.short_id == element {
                    matches.push(Box::new(segment_with_id));
                }
            }
        }
    }
}

/// The `zz` uncommitted-area sentinel as a parse node: children are unstaged
/// filenames, and by itself it resolves to [`CliId::Uncommitted`]. Shared by
/// the full and the uncommitted-scoped element parsers so the sentinel cannot
/// drift between them.
#[derive(Debug)]
struct Unstaged {}

impl<'a> Node<'a> for Unstaged {
    fn parse(
        self: Box<Self>,
        element: &str,
        id_map: &'a IdMap,
        _changes_in_commit_fn: &mut ChangesInCommitFn<'a>,
    ) -> anyhow::Result<Vec<Box<dyn Node<'a> + 'a>>> {
        Ok(id_map.parse_uncommitted_filename(None, element))
    }

    fn to_cli_id(
        self: Box<Self>,
        _short_id: &str,
        id_map: &IdMap,
    ) -> anyhow::Result<Option<CliId>> {
        Ok(Some(id_map.uncommitted.clone()))
    }
}

/// Methods for parsing and generating CLI IDs.
impl IdMap {
    /// Parses a user-provided `entity` name into matching CLI IDs, with each ID matching a single entity.
    /// Multiple IDs may be returned if the entity matches multiple items.
    ///
    /// Besides generated IDs, this method also accepts filenames, which are
    /// interpreted as uncommitted, uncommitted files.
    pub fn parse<'a>(
        &'a self,
        entity: &str,
        changes_in_commit_fn: ChangesInCommitFn<'a>,
    ) -> anyhow::Result<Vec<CliId>> {
        self.parse_with(entity, changes_in_commit_fn, SourceScope::Any)
    }

    /// Like [IdMap::parse], but the leading element resolves in the
    /// uncommitted namespace: commit change IDs and branch/stack short IDs
    /// never shadow a file ID. Use this for selectors that must name
    /// uncommitted changes (e.g. `--changes`), so a file ID handed out by an
    /// earlier command cannot be invalidated by a commit minted in between.
    ///
    /// Container selectors still surface their containers: bare `zz` yields
    /// [`CliId::Uncommitted`], `dir/` yields [`CliId::PathPrefix`], and
    /// `X@{stack}` resolves its stack (and can yield
    /// [`CliId::Stack`]); callers validate the kinds they accept.
    pub fn parse_uncommitted<'a>(
        &'a self,
        entity: &str,
        changes_in_commit_fn: ChangesInCommitFn<'a>,
    ) -> anyhow::Result<Vec<CliId>> {
        self.parse_with(entity, changes_in_commit_fn, SourceScope::UncommittedOnly)
    }

    fn parse_with<'a>(
        &'a self,
        entity: &str,
        mut changes_in_commit_fn: ChangesInCommitFn<'a>,
        scope: SourceScope,
    ) -> anyhow::Result<Vec<CliId>> {
        let mut cli_ids = Vec::new();
        if let Some((lhs, rhs)) = entity.split_once(':') {
            if let Some((mhs, rhs)) = rhs.rsplit_once(':') {
                // 2 colons is the limit. This allows filenames with
                // colons to be specified in the middle part (e.g.
                // `a:filename:with:colon:b` will parse to `a`,
                // `filename:with:colon`, `b`).
                for node in self.parse_element_scoped(lhs, scope)? {
                    for node in node.parse(mhs, self, &mut changes_in_commit_fn)? {
                        for node in node.parse(rhs, self, &mut changes_in_commit_fn)? {
                            if let Some(cli_id) = node.to_cli_id(entity, self)? {
                                cli_ids.push(cli_id);
                            }
                        }
                    }
                }
            } else {
                for node in self.parse_element_scoped(lhs, scope)? {
                    for node in node.parse(rhs, self, &mut changes_in_commit_fn)? {
                        if let Some(cli_id) = node.to_cli_id(entity, self)? {
                            cli_ids.push(cli_id);
                        }
                    }
                }
            }
        } else {
            for node in self.parse_element_scoped(entity, scope)? {
                if let Some(cli_id) = node.to_cli_id(entity, self)? {
                    cli_ids.push(cli_id);
                }
            }
        }

        let mut deduped = Vec::new();
        'next: for cli_id in cli_ids {
            for existing in &deduped {
                if cli_ids_refer_to_same_entity(existing, &cli_id) {
                    continue 'next;
                }
            }
            deduped.push(cli_id);
        }

        Ok(deduped)
    }
    /// Convenience for [IdMap::parse] if a [gix::Repository] is available.
    pub fn parse_using_repo<'a>(
        &'a self,
        entity: &str,
        repo: &'a gix::Repository,
    ) -> anyhow::Result<Vec<CliId>> {
        self.parse(
            entity,
            Box::new(move |commit_id, parent_id| {
                but_core::diff::tree_changes(repo, parent_id, commit_id)
            }),
        )
    }

    /// Convenience for [IdMap::parse] if a [Context] is available.
    pub fn parse_using_context(&self, entity: &str, ctx: &Context) -> anyhow::Result<Vec<CliId>> {
        let repo = &*ctx.repo.get()?;
        self.parse_using_repo(entity, repo)
    }

    /// Convenience for [IdMap::parse_uncommitted] if a [gix::Repository] is available.
    pub fn parse_uncommitted_using_repo<'a>(
        &'a self,
        entity: &str,
        repo: &'a gix::Repository,
    ) -> anyhow::Result<Vec<CliId>> {
        self.parse_uncommitted(
            entity,
            Box::new(move |commit_id, parent_id| {
                but_core::diff::tree_changes(repo, parent_id, commit_id)
            }),
        )
    }

    /// Convenience for [IdMap::parse_uncommitted] if a [Context] is available.
    pub fn parse_uncommitted_using_context(
        &self,
        entity: &str,
        ctx: &Context,
    ) -> anyhow::Result<Vec<CliId>> {
        let repo = &*ctx.repo.get()?;
        self.parse_uncommitted_using_repo(entity, repo)
    }

    /// Returns the [`CliId::Stack`] for a given `stack_id`, if it exists.
    pub fn resolve_stack(&self, stack_id: StackId) -> Option<&CliId> {
        self.stack_ids.get(&stack_id)
    }

    /// Returns the [`CliId::Uncommitted`] for the uncommitted area, which is useful as an
    /// ID for a destination of operations.
    ///
    /// The uncommitted area represents files and changes that are not assigned to any branch.
    pub fn uncommitted(&self) -> &CliId {
        &self.uncommitted
    }

    /// Returns all known stacks.
    pub fn stacks(&self) -> &Vec<StackWithId> {
        self.indexed_stacks.borrow_owner()
    }

    /// The change ID behind the primary identifier `but status` displays for
    /// `commit_id`, with its disambiguated short form. Returns `None` when the
    /// sha is the identifier instead, i.e. for commits without a change ID or
    /// commits this map does not contain.
    ///
    /// A commit is only found in a map built while it existed: use the map the
    /// user's arguments were resolved against for commits they referenced, and
    /// a freshly built map for commits a mutation just created. Do not
    /// substitute a stale map for the latter: while plain change-ID prefixes
    /// stay valid as the workspace changes (the parser matches them by
    /// prefix), the `#N` collision suffixes are positional and only match
    /// exactly, so an ID taken from the wrong map can silently identify a
    /// different commit.
    pub fn change_id_ref(&self, commit_id: gix::ObjectId) -> Option<&ChangeIdWithShortId> {
        self.stacks()
            .iter()
            .flat_map(|stack| &stack.segments)
            .flat_map(|segment| &segment.workspace_commits)
            .find(|commit| commit.commit_id() == commit_id)?
            .change_id
            .as_ref()
    }
}

fn cli_ids_refer_to_same_entity(lhs: &CliId, rhs: &CliId) -> bool {
    match (lhs, rhs) {
        (CliId::UncommittedHunkOrFile(lhs), CliId::UncommittedHunkOrFile(rhs)) => lhs == rhs,
        (
            CliId::Commit {
                commit_id: lhs_commit_id,
                ..
            },
            CliId::Commit {
                commit_id: rhs_commit_id,
                ..
            },
        ) => lhs_commit_id == rhs_commit_id,
        (
            CliId::CommittedFile {
                commit_id: lhs_commit_id,
                path: lhs_path,
                ..
            },
            CliId::CommittedFile {
                commit_id: rhs_commit_id,
                path: rhs_path,
                ..
            },
        ) => lhs_commit_id == rhs_commit_id && lhs_path == rhs_path,
        (
            CliId::Branch {
                name: lhs_name,
                id: lhs_id,
                stack_id: lhs_stack_id,
                ..
            },
            CliId::Branch {
                name: rhs_name,
                id: rhs_id,
                stack_id: rhs_stack_id,
                ..
            },
        ) => match (lhs_stack_id, rhs_stack_id) {
            // Managed stacks have stable stack IDs, so this is true entity identity.
            (Some(lhs_stack_id), Some(rhs_stack_id)) => {
                lhs_name == rhs_name && lhs_stack_id == rhs_stack_id
            }
            // Unmanaged stacks can have `None` IDs; keep branch matches distinct by their own CLI IDs.
            _ => lhs_id == rhs_id,
        },
        (
            CliId::Stack {
                stack_id: lhs_stack_id,
                ..
            },
            CliId::Stack {
                stack_id: rhs_stack_id,
                ..
            },
        ) => lhs_stack_id == rhs_stack_id,
        (CliId::Uncommitted { .. }, CliId::Uncommitted { .. }) => true,
        _ => false,
    }
}

/// An uncommitted file or hunk in the worktree.
#[derive(Debug, Clone)]
pub struct UncommittedHunkOrFile {
    /// The short CLI ID for this file (typically 2 characters)
    pub id: ShortId,
    /// The hunk assignments
    pub hunk_assignments: NonEmpty<HunkAssignment>,
    /// `true` if self represents all hunks in a stack-assignment or file pair.
    /// Note that this file may have hunks with other stack assignments.
    pub is_entire_file: bool,
}

impl PartialEq for UncommittedHunkOrFile {
    fn eq(&self, other: &Self) -> bool {
        let Self {
            hunk_assignments,
            is_entire_file,
            // Intentionally dont compare the short id since it depends on what other hunks exist
            // and thus doesn't change the identity of this hunk specifically.
            //
            // `fn synthetic_hunk` relies on this.
            id: _,
        } = self;
        hunk_assignments == &other.hunk_assignments && *is_entire_file == other.is_entire_file
    }
}

impl UncommittedHunkOrFile {
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
            "the uncommitted area"
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
    UncommittedHunkOrFile(UncommittedHunkOrFile),
    /// A path prefix, representing several uncommitted hunks.
    PathPrefix {
        /// The ID as given by the user
        id: ShortId,
        /// The hunk assignments with their associated short IDs
        hunk_assignments: NonEmpty<(ShortId, HunkAssignment)>,
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
        /// The stable change ID from the commit headers, if present.
        change_id: Option<but_core::ChangeId>,
    },
    /// The uncommitted area, as a designated area that files can be put in.
    Uncommitted {
        /// The CLI ID for the uncommitted area.
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
            (Self::UncommittedHunkOrFile(l), Self::UncommittedHunkOrFile(r)) => l == r,
            (
                Self::CommittedFile {
                    id: l_id,
                    path: l_path,
                    ..
                },
                Self::CommittedFile {
                    id: r_id,
                    path: r_path,
                    ..
                },
            ) => l_id == r_id && l_path == r_path,
            (Self::Branch { id: l_id, .. }, Self::Branch { id: r_id, .. }) => l_id == r_id,
            (Self::Commit { id: l_id, .. }, Self::Commit { id: r_id, .. }) => l_id == r_id,
            (Self::Stack { id: l_id, .. }, Self::Stack { id: r_id, .. }) => l_id == r_id,
            (Self::Uncommitted { .. }, Self::Uncommitted { .. }) => true,
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
            CliId::UncommittedHunkOrFile { .. } => "an uncommitted file or hunk",
            CliId::PathPrefix { .. } => "a path prefix",
            CliId::CommittedFile { .. } => "a committed file",
            CliId::Branch { .. } => "a branch",
            CliId::Commit { .. } => "a commit",
            CliId::Uncommitted { .. } => "the uncommitted area",
            CliId::Stack { .. } => "a stack",
        }
    }

    /// Returns the short ID string for display to users.
    pub fn to_short_string(&self) -> ShortId {
        match self {
            CliId::UncommittedHunkOrFile(UncommittedHunkOrFile { id, .. })
            | CliId::PathPrefix { id, .. }
            | CliId::CommittedFile { id, .. }
            | CliId::Branch { id, .. }
            | CliId::Commit { id, .. }
            | CliId::Stack { id, .. }
            | CliId::Uncommitted { id, .. } => id.clone(),
        }
    }

    /// Get the stack id, if any.
    pub fn stack_id(&self) -> Option<StackId> {
        match self {
            CliId::Branch { stack_id, .. } => *stack_id,
            CliId::Stack { stack_id, .. } => Some(*stack_id),
            CliId::UncommittedHunkOrFile(uncommitted_cli_id) => {
                uncommitted_cli_id.hunk_assignments.first().stack_id
            }
            CliId::PathPrefix { .. }
            | CliId::CommittedFile { .. }
            | CliId::Commit { .. }
            | CliId::Uncommitted { .. } => None,
        }
    }

    /// Try to convert the id into a hunk.
    pub fn as_uncommitted_hunk_or_file(&self) -> Option<&UncommittedHunkOrFile> {
        if let Self::UncommittedHunkOrFile(hunk) = self {
            Some(hunk)
        } else {
            None
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
    short_id_hunk_assignments: NonEmpty<(UnqualifiedHunkId, HunkAssignment)>,
}

impl UncommittedFile {
    /// Return the file's stack if it is associated to one, or `None` if the Stack is unknown/has no ID.
    pub fn stack_id(&self) -> Option<StackId> {
        self.hunk_assignments().first().stack_id
    }
    /// The path of the uncommitted file.
    pub fn path(&self) -> &BStr {
        self.hunk_assignments().first().path_bytes.as_ref()
    }
    /// Turn this instance into a [CliId].
    pub fn to_id(&self) -> CliId {
        CliId::UncommittedHunkOrFile(UncommittedHunkOrFile {
            hunk_assignments: self
                .hunk_assignments()
                .map(|hunk_assignment| hunk_assignment.to_owned()),
            id: self.short_id.clone(),
            is_entire_file: true,
        })
    }
    /// Hunk assignments.
    pub fn hunk_assignments(&self) -> NonEmpty<&HunkAssignment> {
        self.short_id_hunk_assignments
            .as_ref()
            .map(|(_, hunk_assignment)| hunk_assignment)
    }
}

impl<'a> Node<'a> for &'a UncommittedFile {
    fn parse(
        self: Box<Self>,
        element: &str,
        _id_map: &'a IdMap,
        _changes_in_commit_fn: &mut ChangesInCommitFn<'a>,
    ) -> anyhow::Result<Vec<Box<dyn Node<'a> + 'a>>> {
        match element.strip_prefix(INDEX_SEPARATOR) {
            Some(maybe_index) if let Ok(index) = usize::from_str(maybe_index) => {
                if let Some((hunk_id, hunk_assignment)) = self.short_id_hunk_assignments.get(index)
                {
                    let cli_id = CliId::UncommittedHunkOrFile(UncommittedHunkOrFile {
                        id: format!("{}:{}", self.short_id, hunk_id.short_id()),
                        hunk_assignments: NonEmpty::new(hunk_assignment.to_owned()),
                        is_entire_file: false,
                    });
                    Ok(vec![Box::new(Leaf { cli_id })])
                } else {
                    Ok(vec![])
                }
            }
            _ => {
                let matches = self
                    .short_id_hunk_assignments
                    .iter()
                    .filter(|(hunk_id, _)| hunk_id.matches_prefix(element))
                    .map(|(hunk_id, hunk_assignment)| {
                        let cli_id = CliId::UncommittedHunkOrFile(UncommittedHunkOrFile {
                            id: format!("{}:{}", self.short_id, hunk_id.short_id()),
                            hunk_assignments: NonEmpty::new(hunk_assignment.to_owned()),
                            is_entire_file: false,
                        });
                        Box::new(Leaf { cli_id }) as Box<dyn Node<'a> + 'a>
                    });

                Ok(matches.collect())
            }
        }
    }

    fn to_cli_id(
        self: Box<Self>,
        _short_id: &str,
        _id_map: &IdMap,
    ) -> anyhow::Result<Option<CliId>> {
        Ok(Some(CliId::UncommittedHunkOrFile(UncommittedHunkOrFile {
            id: self.short_id.clone(),
            hunk_assignments: self
                .hunk_assignments()
                .map(|hunk_assignment| hunk_assignment.to_owned()),
            is_entire_file: true,
        })))
    }
}

/// An uncommitted hunk.
#[derive(Debug)]
pub struct UncommittedHunk {
    /// The hunk assignment.
    pub hunk_assignment: HunkAssignment,
}

impl<'a> Node<'a> for &'a UncommittedHunk {
    fn parse(
        self: Box<Self>,
        _element: &str,
        _id_map: &'a IdMap,
        _changes_in_commit_fn: &mut ChangesInCommitFn<'a>,
    ) -> anyhow::Result<Vec<Box<dyn Node<'a> + 'a>>> {
        Ok(Vec::new())
    }

    fn to_cli_id(
        self: Box<Self>,
        short_id: &str,
        _id_map: &IdMap,
    ) -> anyhow::Result<Option<CliId>> {
        Ok(Some(CliId::UncommittedHunkOrFile(UncommittedHunkOrFile {
            id: short_id.to_owned(),
            hunk_assignments: NonEmpty::new(self.hunk_assignment.clone()),
            is_entire_file: false,
        })))
    }
}
