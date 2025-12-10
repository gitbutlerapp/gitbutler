use bstr::{BStr, BString, ByteSlice};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;
use but_workspace::branch::Stack;
use std::borrow::Cow;
use std::{
    borrow::Borrow,
    collections::{BTreeSet, HashMap, HashSet},
    fmt::Display,
};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct IdMap {
    branch_name_to_cli_id: HashMap<BString, CliId>,
    ids_used: HashSet<String>,
    workspace_commit_and_first_parent_ids: Vec<(gix::ObjectId, Option<gix::ObjectId>)>,
    remote_commit_ids: Vec<gix::ObjectId>,
    unassigned: CliId,

    uncommitted_files: BTreeSet<UncommittedFile>,
    committed_files: BTreeSet<CommittedFile>,
}

/// Lifecycle
impl IdMap {
    /// Initialise CLI IDs for all information in the `RefInfo` structure for
    /// `HEAD`. Callers that do not need to support files in the non-error
    /// code path can use the return value as-is; when there is an error
    /// (or if the caller needs to support files in the first place),
    /// [Self::add_file_info()] can be called to enable parsing file IDs.
    pub fn new(stacks: &[Stack]) -> anyhow::Result<Self> {
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

    /// Enable parsing uncommitted and committed file IDs.
    fn add_file_info<F>(
        &mut self,
        changed_paths_fn: F,
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
            changed_paths_fn,
            hunk_assignments,
        )?;

        let mut int_hash = 0u64;
        let mut get_next_id = || -> String {
            loop {
                let tentative_id = string_hash(int_hash);
                int_hash += 1;
                if !self.ids_used.contains(&tentative_id) {
                    return tentative_id;
                }
            }
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

/// Thin wrappers around lifecycle methods for use with [Context].
impl IdMap {
    /// Create a new instance from `ctx`, which is used to get [head info](but_workspace::head_info())
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
        Self::new(&head_info.stacks)
    }
}

/// Add context for ID generation
impl IdMap {
    /// Use `ctx` to retrieve information around…
    /// * …changed files in the worktree, taking their assignments into account.
    /// * …all changes of all worktree commits
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
}

/// Cli ID generation
impl IdMap {
    pub fn parse_str(&self, s: &str) -> anyhow::Result<Vec<CliId>> {
        if s.len() < 2 {
            return Err(anyhow::anyhow!(
                "Id needs to be at least 2 characters long: {}",
                s
            ));
        }

        let mut matches = Vec::new();

        // First, try partial branch name match
        if let Ok(branch_matches) = self.find_branches_by_name(s.into()) {
            matches.extend(branch_matches);
        }

        // Only try SHA matching if the input looks like a hex string
        if s.chars().all(|c| c.is_ascii_hexdigit()) && s.len() >= 2 {
            for oid in self
                .workspace_and_remote_commit_ids()
                .filter(|oid| oid.to_string().starts_with(s))
            {
                matches.push(CliId::Commit { oid: *oid });
            }
        }

        // Then try CliId matching
        if s.len() == 2 {
            if let Some(UncommittedFile {
                assignment_path: (assignment, path),
                ..
            }) = self.uncommitted_files.get(s)
            {
                matches.push(CliId::UncommittedFile {
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
                matches.push(CliId::CommittedFile {
                    commit_oid: *commit_oid,
                    path: path.to_owned(),
                    id: s.to_string(),
                });
            }
        }
        if s.find(|c: char| c != '0').is_none() {
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

    fn workspace_and_remote_commit_ids(&self) -> impl Iterator<Item = &gix::ObjectId> {
        self.workspace_commit_and_first_parent_ids
            .iter()
            .map(|(commit_id, _parent_id)| commit_id)
            .chain(&self.remote_commit_ids)
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

/// Lifecycle
impl CliId {
    /// Create a CliID identifying `oid`.
    pub fn commit(oid: gix::ObjectId) -> Self {
        CliId::Commit { oid }
    }
}

/// Access
impl CliId {
    pub fn kind_for_humans(&self) -> &'static str {
        match self {
            CliId::UncommittedFile { .. } => "an uncommitted file",
            CliId::CommittedFile { .. } => "a committed file",
            CliId::Branch { .. } => "a branch",
            CliId::Commit { .. } => "a commit",
            CliId::Unassigned { .. } => "the unassigned area",
        }
    }

    /// Obtain an ID-string from this instance, for human usage as it's meant to be short.
    pub fn to_short_str(&self) -> Cow<'_, str> {
        match self {
            CliId::UncommittedFile { id, .. }
            | CliId::CommittedFile { id, .. }
            | CliId::Branch { id, .. }
            | CliId::Unassigned { id, .. } => Cow::Borrowed(id),
            CliId::Commit { oid, .. } => Cow::Owned(oid.to_hex_with_len(2).to_string()),
        }
    }
}

impl Display for CliId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_short_str())
    }
}

/// All information from HEAD's [Stack] objects needed for branch and commit CLI IDs.
struct StacksInfo {
    /// Branch names in unspecified order.
    branch_names: Vec<BString>,
    /// Commit IDs of commits reachable from the workspace tip with their first parent IDs in unspecified order.
    /// The first parent ID is stored in case a diff needs to be performed on the commit.
    workspace_commit_and_first_parent_ids: Vec<(gix::ObjectId, Option<gix::ObjectId>)>,
    /// Commits that are reachable from the remote-tracking only, i.e. are only on the remote, in unspecified order.
    remote_commit_ids: Vec<gix::ObjectId>,
}

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

/// All file information needed for uncommitted file and committed file CLI IDs.
struct FileInfo {
    /// Uncommitted files ordered by assignment, then filename.
    uncommitted_files: Vec<(Option<StackId>, BString)>,
    /// Committed files ordered by commit ID, then filename.
    committed_files: Vec<(gix::ObjectId, BString)>,
}

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

/// a.cmp(b) == a.id.cmp(&b.id) for all a and b
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
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
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
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

fn hash(input: &str) -> String {
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
