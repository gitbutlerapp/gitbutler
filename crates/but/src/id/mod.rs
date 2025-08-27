use std::fmt::Display;

use but_hunk_assignment::HunkAssignment;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;

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
    },
    Commit {
        oid: gix::ObjectId,
    },
    Unassigned,
}

impl CliId {
    pub fn kind(&self) -> &'static str {
        match self {
            CliId::UncommittedFile { .. } => "an uncommitted file",
            CliId::CommittedFile { .. } => "a committed file",
            CliId::Branch { .. } => "a branch",
            CliId::Commit { .. } => "a commit",
            CliId::Unassigned => "the unassigned area",
        }
    }
    pub fn commit(oid: gix::ObjectId) -> Self {
        CliId::Commit { oid }
    }

    pub fn branch(name: &str) -> Self {
        CliId::Branch {
            name: name.to_owned(),
        }
    }

    pub fn unassigned() -> Self {
        CliId::Unassigned
    }

    pub fn file_from_assignment(assignment: &HunkAssignment) -> Self {
        CliId::UncommittedFile {
            path: assignment.path.clone(),
            assignment: assignment.stack_id,
        }
    }

    pub fn file(path: &str) -> Self {
        CliId::UncommittedFile {
            path: path.to_string(),
            assignment: None,
        }
    }

    pub fn committed_file(path: &str, commit_oid: gix::ObjectId) -> Self {
        CliId::CommittedFile {
            path: path.to_string(),
            commit_oid,
        }
    }

    fn find_branches_by_name(ctx: &CommandContext, name: &str) -> anyhow::Result<Vec<Self>> {
        let stacks = crate::log::stacks(ctx)?;
        let mut matches = Vec::new();

        for stack in stacks {
            for head in &stack.heads {
                let branch_name = head.name.to_string();
                // Exact match or partial match
                if branch_name == name || branch_name.contains(name) {
                    matches.push(CliId::branch(&branch_name));
                }
            }
        }

        Ok(matches)
    }

    fn find_commits_by_sha(ctx: &CommandContext, sha_prefix: &str) -> anyhow::Result<Vec<Self>> {
        let mut matches = Vec::new();

        // Only try SHA matching if the input looks like a hex string
        if sha_prefix.chars().all(|c| c.is_ascii_hexdigit()) && sha_prefix.len() >= 4 {
            let all_commits = crate::log::all_commits(ctx)?;
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
        s == self.to_string()
    }

    pub fn matches_prefix(&self, s: &str) -> bool {
        match self {
            CliId::Commit { oid } => {
                let oid_hash = hash(&oid.to_string());
                oid_hash.starts_with(s)
            }
            _ => self.to_string().starts_with(s),
        }
    }

    pub fn from_str(ctx: &mut CommandContext, s: &str) -> anyhow::Result<Vec<Self>> {
        if s.len() < 2 {
            return Err(anyhow::anyhow!(
                "Id needs to be at least 2 characters long: {}",
                s
            ));
        }

        let mut matches = Vec::new();

        // First, try exact branch name match
        if let Ok(branch_matches) = Self::find_branches_by_name(ctx, s) {
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
            crate::status::all_files(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| cli_matches.push(id));
            crate::status::all_committed_files(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| cli_matches.push(id));
            crate::status::all_branches(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| cli_matches.push(id));
            crate::log::all_commits(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| cli_matches.push(id));
            if CliId::unassigned().matches_prefix(s) {
                cli_matches.push(CliId::unassigned());
            }
            matches.extend(cli_matches);
        } else {
            // For 2-character strings, try exact CliId matching
            let mut cli_matches = Vec::new();
            crate::status::all_files(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            crate::status::all_committed_files(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            crate::status::all_branches(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            crate::log::all_commits(ctx)?
                .into_iter()
                .filter(|id| id.matches(s))
                .for_each(|id| cli_matches.push(id));
            if CliId::unassigned().matches(s) {
                cli_matches.push(CliId::unassigned());
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
                    let value = hash(&format!("{}{}", assignment, path));
                    write!(f, "{}", value)
                } else {
                    write!(f, "{}", hash(path))
                }
            }
            CliId::CommittedFile { path, commit_oid } => {
                let value = hash(&format!("{}{}", commit_oid, path));
                write!(f, "{}", value)
            }
            CliId::Branch { name } => {
                write!(f, "{}", hash(name))
            }
            CliId::Unassigned => {
                write!(f, "00")
            }
            CliId::Commit { oid } => {
                let oid_str = oid.to_string();
                write!(f, "{}", hash(&oid_str))
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

    format!("{}{}", first_char, second_char)
}
