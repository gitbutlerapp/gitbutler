use std::{
    borrow::Borrow,
    collections::{BTreeSet, HashMap, HashSet},
    fmt::Display,
};

use bstr::{BStr, BString, ByteSlice};
use but_core::ref_metadata::StackId;
use but_ctx::Context;

#[cfg(test)]
mod tests;

/// All information from [Context] needed for ID creation.
struct ContextInfo {
    /// Branch names in unspecified order.
    branch_names: Vec<BString>,
    /// Commit IDs in unspecified order.
    commit_ids: Vec<gix::ObjectId>,
    /// Uncommitted files ordered by assignment, then filename.
    uncommitted_files: Vec<(Option<StackId>, BString)>,
    /// Committed files ordered by commit ID, then filename.
    committed_files: Vec<(gix::ObjectId, BString)>,
}

fn context_info(ctx: &mut Context) -> anyhow::Result<ContextInfo> {
    let guard = ctx.shared_worktree_access();
    let meta = ctx.meta(guard.read_permission())?;
    let (branch_names, commit_ids, worktree_dir, committed_files) = {
        let repo = &*ctx.repo.get()?;
        let head_info = but_workspace::head_info(
            repo,
            &meta,
            but_workspace::ref_info::Options {
                expensive_commit_info: false,
                ..Default::default()
            },
        )?;
        let mut branch_names: Vec<BString> = Vec::new();
        let mut commit_ids: Vec<gix::ObjectId> = Vec::new();
        let mut committed_files: Vec<(gix::ObjectId, BString)> = Vec::new();
        for stack in head_info.stacks {
            for segment in stack.segments {
                if let Some(ref_info) = segment.ref_info {
                    branch_names.push(ref_info.ref_name.shorten().to_owned());
                }
                for commit in segment.commits {
                    let inner = commit.inner;
                    commit_ids.push(inner.id);

                    let tree_changes = but_core::diff::tree_changes(
                        repo,
                        inner.parent_ids.first().copied(),
                        inner.id,
                    )?;
                    for tree_change in tree_changes {
                        committed_files.push((inner.id, tree_change.path));
                    }
                }
                for commit in segment.commits_on_remote {
                    commit_ids.push(commit.id);
                }
            }
        }
        committed_files.sort();

        (
            branch_names,
            commit_ids,
            repo.worktree().map(|worktree| worktree.base().to_owned()),
            committed_files,
        )
    };
    let mut uncommitted_files: Vec<(Option<StackId>, BString)> = Vec::new();
    if let Some(worktree_dir) = worktree_dir {
        let changes = but_core::diff::ui::worktree_changes_by_worktree_dir(worktree_dir)?.changes;
        let (assignments, _assignments_error) =
            but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes), None)?;
        for assignment in assignments {
            uncommitted_files.push((assignment.stack_id, assignment.path_bytes.clone()));
        }
    }

    Ok(ContextInfo {
        branch_names,
        commit_ids,
        uncommitted_files,
        committed_files,
    })
}

/// a.cmp(b) == a.id.cmp(&b.id) for all a and b
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct UncommittedFile {
    assignment_path: (Option<StackId>, BString),
    id: String,
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

/// a.cmp(b) == a.id.cmp(&b.id) for all a and b
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct CommittedFile {
    commit_oid_path: (gix::ObjectId, BString),
    id: String,
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

pub struct IdMap {
    branch_name_to_cli_id: HashMap<BString, CliId>,
    commit_ids: Vec<gix::ObjectId>,
    uncommitted_files: BTreeSet<UncommittedFile>,
    committed_files: BTreeSet<CommittedFile>,
    unassigned: CliId,
}

/// Lifecycle
impl IdMap {
    /// Initialise CLI IDs for all information in the `RefInfo` structure for `HEAD` via `ctx`.
    // TODO: create an API that enforces re-use of `RefInfo` by its users.
    pub fn new(ctx: &mut Context) -> anyhow::Result<Self> {
        let mut max_zero_count = 1; // Ensure at least two "0" in ID.
        let context_info = context_info(ctx)?;
        let mut pairs_to_count: HashMap<u16, u8> = HashMap::new();
        fn u8_pair_to_u16(two: [u8; 2]) -> u16 {
            two[0] as u16 * 256 + two[1] as u16
        }
        for branch_name in &context_info.branch_names {
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
        'branch_name: for branch_name in context_info.branch_names {
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

        let mut int_hash = 0u64;
        let mut get_next_id = || -> String {
            loop {
                let tentative_id = string_hash(int_hash);
                int_hash += 1;
                if !ids_used.contains(&tentative_id) {
                    return tentative_id;
                }
            }
        };

        let mut uncommitted_files: BTreeSet<UncommittedFile> = BTreeSet::new();
        for assignment_path in context_info.uncommitted_files {
            uncommitted_files.insert(UncommittedFile {
                assignment_path,
                id: get_next_id(),
            });
        }

        let mut committed_files: BTreeSet<CommittedFile> = BTreeSet::new();
        for commit_oid_path in context_info.committed_files {
            committed_files.insert(CommittedFile {
                commit_oid_path,
                id: get_next_id(),
            });
        }

        Ok(Self {
            branch_name_to_cli_id,
            commit_ids: context_info.commit_ids,
            uncommitted_files,
            committed_files,
            unassigned: CliId::Unassigned {
                id: str::repeat("0", max_zero_count + 1),
            },
        })
    }
}

/// Cli ID generation
impl IdMap {
    pub fn parse_str(&self, ctx: &mut Context, s: &str) -> anyhow::Result<Vec<CliId>> {
        if s.len() < 2 {
            return Err(anyhow::anyhow!(
                "Id needs to be at least 2 characters long: {}",
                s
            ));
        }

        let mut matches = Vec::new();

        // First, try exact branch name match
        if let Ok(branch_matches) = self.find_branches_by_name(s.into()) {
            matches.extend(branch_matches);
        }

        // Only try SHA matching if the input looks like a hex string
        if s.chars().all(|c| c.is_ascii_hexdigit()) && s.len() >= 2 {
            for oid in self
                .commit_ids
                .iter()
                .filter(|oid| oid.to_string().starts_with(s))
            {
                matches.push(CliId::Commit { oid: *oid });
            }
        }

        // Then try CliId matching (both prefix and exact)
        if s.len() > 2 {
            // For longer strings, try prefix matching on CliIds
            let mut cli_matches = Vec::new();
            crate::command::legacy::status::all_branches(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| cli_matches.push(id));
            matches.extend(cli_matches);
        } else {
            // For 2-character strings, try exact CliId matching
            let mut cli_matches = Vec::new();
            if let Some(UncommittedFile {
                assignment_path: (assignment, path),
                ..
            }) = self.uncommitted_files.get(s)
            {
                cli_matches.push(CliId::UncommittedFile {
                    assignment: *assignment,
                    path: path.to_owned(),
                    id: s.to_string(),
                });
            }
            if let Some(CommittedFile {
                commit_oid_path: (commit_oid, path),
                ..
            }) = self.committed_files.get(s)
            {
                cli_matches.push(CliId::CommittedFile {
                    commit_oid: *commit_oid,
                    path: path.to_owned(),
                    id: s.to_string(),
                });
            }
            crate::command::legacy::status::all_branches(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            matches.extend(cli_matches);
        }
        if self.unassigned().matches(s) {
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

    pub fn uncommitted_file(&self, assignment: Option<StackId>, path: &BStr) -> CliId {
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

    pub fn committed_file(&self, commit_oid: gix::ObjectId, path: &BStr) -> CliId {
        let sought = (commit_oid, path.to_owned());
        if let Some(CommittedFile { id, .. }) = self.committed_files.get(&sought) {
            CliId::CommittedFile {
                commit_oid: sought.0,
                path: sought.1,
                id: id.to_string(),
            }
        } else {
            CliId::CommittedFile {
                commit_oid: sought.0,
                path: sought.1,
                id: "00".to_string(),
            }
        }
    }

    /// Returns the ID for a branch of the given name. If no such ID exists,
    /// generate one.
    pub fn branch(&mut self, name: &BStr) -> &CliId {
        self.branch_name_to_cli_id
            .entry(name.to_owned())
            .or_insert_with(|| {
                let name = name.to_string();
                let id = hash(&name);
                CliId::Branch { name, id }
            })
    }

    /// Represents the unassigned area. Its ID is a repeated string of '0', long
    /// enough to disambiguate against any existing branch name.
    pub fn unassigned(&self) -> &CliId {
        &self.unassigned
    }
}

impl IdMap {
    fn find_branches_by_name(&self, name: &BStr) -> anyhow::Result<Vec<CliId>> {
        let mut matches = Vec::new();

        for (branch_name, cli_id) in self.branch_name_to_cli_id.iter() {
            // Partial match is fine
            if branch_name.contains_str(name) {
                matches.push(cli_id.clone());
            }
        }

        Ok(matches)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum CliId {
    UncommittedFile {
        assignment: Option<StackId>,
        path: BString,
        id: String,
    },
    CommittedFile {
        commit_oid: gix::ObjectId,
        path: BString,
        id: String,
    },
    Branch {
        name: String,
        id: String,
    },
    Commit {
        oid: gix::ObjectId,
    },
    Unassigned {
        id: String,
    },
}

impl CliId {
    pub fn kind(&self) -> &'static str {
        match self {
            CliId::UncommittedFile { .. } => "an uncommitted file",
            CliId::CommittedFile { .. } => "a committed file",
            CliId::Branch { .. } => "a branch",
            CliId::Commit { .. } => "a commit",
            CliId::Unassigned { .. } => "the unassigned area",
        }
    }
    pub fn commit(oid: gix::ObjectId) -> Self {
        CliId::Commit { oid }
    }

    pub fn matches(&self, s: &str) -> bool {
        match self {
            CliId::Unassigned { .. } => s.find(|c: char| c != '0').is_none(),
            _ => s == self.to_string(),
        }
    }

    pub fn matches_prefix(&self, s: &str) -> bool {
        match self {
            CliId::Commit { oid } => {
                let oid_hash = hash(&oid.to_string());
                oid_hash.starts_with(s)
            }
            CliId::Unassigned { .. } => s.find(|c: char| c != '0').is_none(),
            _ => self.to_string().starts_with(s),
        }
    }
}

impl Display for CliId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliId::UncommittedFile { id, .. } => {
                write!(f, "{}", id)
            }
            CliId::CommittedFile { id, .. } => {
                write!(f, "{}", id)
            }
            CliId::Branch { id, .. } => {
                write!(f, "{}", id)
            }
            CliId::Unassigned { id } => {
                write!(f, "{}", id)
            }
            CliId::Commit { oid } => {
                // let oid_str = oid.to_string();
                // write!(f, "{}", hash(&oid_str))
                let oid = oid.to_string();
                write!(f, "{}", &oid[..2])
            }
        }
    }
}

pub(crate) fn hash(input: &str) -> String {
    string_hash(int_hash(input))
}

fn int_hash(input: &str) -> u64 {
    let mut hash = 0u64;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    hash
}

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
