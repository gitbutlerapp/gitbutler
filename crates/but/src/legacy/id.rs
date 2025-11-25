use std::{collections::HashMap, fmt::Display};

use bstr::{BStr, BString, ByteSlice};
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;

fn branch_names(ctx: &Context) -> anyhow::Result<Vec<BString>> {
    let guard = ctx.shared_worktree_access();
    let meta = ctx.meta(guard.read_permission())?;
    let head_info = but_workspace::head_info(&*ctx.repo.get()?, &meta, Default::default())?;
    let mut branch_names: Vec<BString> = Vec::new();
    for stack in head_info.stacks {
        for segment in stack.segments {
            if let Some(ref_info) = segment.ref_info {
                branch_names.push(ref_info.ref_name.shorten().to_owned());
            }
        }
    }
    Ok(branch_names)
}

pub struct IdDb {
    branch_name_to_cli_id: HashMap<BString, CliId>,
    unassigned: CliId,
}

impl IdDb {
    pub fn new(ctx: &Context) -> anyhow::Result<Self> {
        let mut max_zero_count = 1; // Ensure at least two "0" in ID.
        let branch_names = branch_names(ctx)?;
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
                    branch_name_to_cli_id.insert(branch_name, CliId::Branch { name, id });
                    continue 'branch_name;
                }
            }
        }
        Ok(Self {
            branch_name_to_cli_id,
            unassigned: CliId::Unassigned {
                id: str::repeat("0", max_zero_count + 1),
            },
        })
    }

    fn find_branches_by_name(&mut self, ctx: &Context, name: &BStr) -> anyhow::Result<Vec<CliId>> {
        let branch_names = branch_names(ctx)?;
        let mut matches = Vec::new();

        for branch_name in branch_names {
            // Partial match is fine
            if branch_name.contains_str(name) {
                matches.push(self.branch(branch_name.as_ref()).clone())
            }
        }

        Ok(matches)
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
        path: String,
        commit_oid: gix::ObjectId,
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

    pub fn committed_file(path: &str, commit_oid: gix::ObjectId) -> Self {
        CliId::CommittedFile {
            path: path.to_string(),
            commit_oid,
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

    pub fn from_str(ctx: &mut Context, s: &str) -> anyhow::Result<Vec<Self>> {
        if s.len() < 2 {
            return Err(anyhow::anyhow!(
                "Id needs to be at least 2 characters long: {}",
                s
            ));
        }

        // TODO: make callers of this function pass IdDb instead
        let mut id_db = IdDb::new(ctx)?;

        let mut matches = Vec::new();

        // First, try exact branch name match
        if let Ok(branch_matches) = id_db.find_branches_by_name(ctx, s.into()) {
            matches.extend(branch_matches);
        }

        // Then try partial SHA matches (for commits)
        if let Ok(commit_matches) = Self::find_commits_by_sha(ctx, s) {
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
            crate::command::legacy::status::all_committed_files(ctx)?
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
            if id_db.unassigned().matches_prefix(s) {
                cli_matches.push(id_db.unassigned().clone());
            }
            matches.extend(cli_matches);
        } else {
            // For 2-character strings, try exact CliId matching
            let mut cli_matches = Vec::new();
            crate::command::legacy::status::all_files(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            crate::command::legacy::status::all_committed_files(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            crate::command::legacy::status::all_branches(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            crate::legacy::commits::all_commits(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            if id_db.unassigned().matches(s) {
                cli_matches.push(id_db.unassigned().clone());
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
            CliId::CommittedFile { path, commit_oid } => {
                let value = hash(&format!("{commit_oid}{path}"));
                write!(f, "{value}")
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
    let mut hash = 0u64;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    // First character: g-z (20 options)
    let first_chars = "ghijklmnopqrstuvwxyz";
    let first_char = first_chars.chars().nth((hash % 20) as usize).unwrap();
    hash /= 20;

    // Second character: 0-9,a-z (36 options)
    let second_chars = "0123456789abcdefghijklmnopqrstuvwxyz";
    let second_char = second_chars.chars().nth((hash % 36) as usize).unwrap();

    format!("{first_char}{second_char}")
}
