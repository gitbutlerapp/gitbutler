use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

use bstr::{BStr, BString, ByteSlice};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;

/// All information from [Context] needed for ID creation.
struct ContextInfo {
    /// Branch names in unspecified order.
    branch_names: Vec<BString>,
    /// Committed files ordered by commit ID, then filename.
    committed_files: Vec<(gix::ObjectId, BString)>,
}

fn context_info(ctx: &Context) -> anyhow::Result<ContextInfo> {
    let guard = ctx.shared_worktree_access();
    let meta = ctx.meta(guard.read_permission())?;
    let repo = &*ctx.repo.get()?;
    let head_info = but_workspace::head_info(repo, &meta, Default::default())?;
    let mut branch_names: Vec<BString> = Vec::new();
    let mut committed_files: Vec<(gix::ObjectId, BString)> = Vec::new();
    for stack in head_info.stacks {
        for segment in stack.segments {
            if let Some(ref_info) = segment.ref_info {
                branch_names.push(ref_info.ref_name.shorten().to_owned());
            }
            for commit in segment.commits {
                let inner = commit.inner;
                let tree_changes = but_core::diff::tree_changes(
                    repo,
                    inner.parent_ids.first().copied(),
                    inner.id,
                )?;
                for tree_change in tree_changes {
                    committed_files.push((inner.id, tree_change.path));
                }
            }
        }
    }
    committed_files.sort();
    Ok(ContextInfo {
        branch_names,
        committed_files,
    })
}

pub struct IdDb {
    branch_name_to_cli_id: HashMap<BString, CliId>,
    /// Tuple of `commit_oid`, `path`, and `id`. Ordered by `(commit_oid, path)`
    /// and at the same time, `id`. This means that a binary search can be
    /// performed either on `(commit_oid, path)` or `id`.
    committed_files: Vec<(gix::ObjectId, BString, String)>,
    unassigned: CliId,
}

impl IdDb {
    pub fn new(ctx: &Context) -> anyhow::Result<Self> {
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

        let mut committed_files: Vec<(gix::ObjectId, BString, String)> = Vec::new();
        let mut int_hash = 0u64;
        for (commit_oid, path) in context_info.committed_files {
            let id = loop {
                let tentative_id = string_hash(int_hash);
                int_hash += 1;
                if !ids_used.contains(&tentative_id) {
                    break tentative_id;
                }
            };
            committed_files.push((commit_oid, path, id));
        }

        Ok(Self {
            branch_name_to_cli_id,
            committed_files,
            unassigned: CliId::Unassigned {
                id: str::repeat("0", max_zero_count + 1),
            },
        })
    }

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

        // Then try partial SHA matches (for commits)
        if let Ok(commit_matches) = CliId::find_commits_by_sha(ctx, s) {
            matches.extend(commit_matches);
        }

        // Then try CliId matching (both prefix and exact)
        if s.len() > 2 {
            // For longer strings, try prefix matching on CliIds
            let mut cli_matches = Vec::new();
            crate::command::legacy::status::all_files(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| cli_matches.push(id));
            crate::command::legacy::status::all_branches(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| cli_matches.push(id));
            crate::legacy::commits::all_commits(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| cli_matches.push(id));
            if self.unassigned().matches_prefix(s) {
                cli_matches.push(self.unassigned().clone());
            }
            matches.extend(cli_matches);
        } else {
            // For 2-character strings, try exact CliId matching
            let mut cli_matches = Vec::new();
            crate::command::legacy::status::all_files(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            if let Ok(index) = self
                .committed_files
                .binary_search_by(|(_, _, id)| id.as_str().cmp(s))
            {
                let (commit_oid, path, id) = &self.committed_files[index];
                cli_matches.push(CliId::CommittedFile {
                    commit_oid: *commit_oid,
                    path: path.to_owned(),
                    id: id.to_string(),
                });
            }
            crate::command::legacy::status::all_branches(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            crate::legacy::commits::all_commits(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            if self.unassigned().matches(s) {
                cli_matches.push(self.unassigned().clone());
            }
            matches.extend(cli_matches);
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

    pub fn committed_file(&self, commit_oid: gix::ObjectId, path: &BStr) -> CliId {
        let sought_commit_oid = &commit_oid;
        let sought_path = path;
        match self
            .committed_files
            .binary_search_by(|(commit_oid, path, _)| {
                commit_oid
                    .cmp(sought_commit_oid)
                    .then(AsRef::<BStr>::as_ref(path).cmp(sought_path))
            }) {
            Ok(index) => {
                let (commit_oid, path, id) = &self.committed_files[index];
                CliId::CommittedFile {
                    commit_oid: *commit_oid,
                    path: path.to_owned(),
                    id: id.to_string(),
                }
            }
            Err(_) => CliId::CommittedFile {
                commit_oid,
                path: path.to_owned(),
                id: "00".to_string(),
            },
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

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum CliId {
    UncommittedFile {
        path: String,
        assignment: Option<StackId>,
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

    pub fn file_from_assignment(assignment: &HunkAssignment) -> Self {
        CliId::UncommittedFile {
            path: assignment.path.clone(),
            assignment: assignment.stack_id,
        }
    }

    fn find_commits_by_sha(ctx: &Context, sha_prefix: &str) -> anyhow::Result<Vec<Self>> {
        let mut matches = Vec::new();

        // Only try SHA matching if the input looks like a hex string
        if sha_prefix.chars().all(|c| c.is_ascii_hexdigit()) && sha_prefix.len() >= 4 {
            let all_commits = crate::legacy::commits::all_commits(ctx)?;
            for commit_id in all_commits {
                if let CliId::Commit { oid } = &commit_id {
                    let sha_string = oid.to_string();
                    if sha_string.starts_with(sha_prefix) {
                        matches.push(commit_id);
                    }
                }
            }
        }

        Ok(matches)
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
            CliId::UncommittedFile { path, assignment } => {
                if let Some(assignment) = assignment {
                    let value = hash(&format!("{assignment}{path}"));
                    write!(f, "{value}")
                } else {
                    write!(f, "{}", hash(path))
                }
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
