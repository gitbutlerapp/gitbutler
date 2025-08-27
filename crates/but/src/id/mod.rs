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

    pub fn committed_file(path: &str, commit_oid: gix::ObjectId) -> Self {
        CliId::CommittedFile {
            path: path.to_string(),
            commit_oid,
        }
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
            _ => self.to_string().starts_with(s)
        }
    }

    pub fn from_str(ctx: &mut CommandContext, s: &str) -> anyhow::Result<Vec<Self>> {
        if s.len() < 2 {
            return Err(anyhow::anyhow!("Id needs to be 3 characters long: {}", s));
        }
        
        // First try with the full input string for prefix matching
        if s.len() > 2 {
            let mut everything = Vec::new();
            crate::status::all_files(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| everything.push(id));
            crate::status::all_committed_files(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| everything.push(id));
            crate::status::all_branches(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| everything.push(id));
            crate::log::all_commits(ctx)?
                .into_iter()
                .filter(|id| id.matches_prefix(s))
                .for_each(|id| everything.push(id));
            if CliId::unassigned().matches_prefix(s) {
                everything.push(CliId::unassigned());
            }
            
            // If we found exactly one match with the full prefix, return it
            if everything.len() == 1 {
                return Ok(everything);
            }
            // If we found multiple matches with the full prefix, return them all (ambiguous)
            if everything.len() > 1 {
                return Ok(everything);
            }
            // If no matches with full prefix, fall through to 2-char matching
        }
        
        // Fall back to original 2-character matching behavior
        let s = &s[..2];
        let mut everything = Vec::new();
        crate::status::all_files(ctx)?
            .into_iter()
            .filter(|id| id.matches(s))
            .for_each(|id| everything.push(id));
        crate::status::all_committed_files(ctx)?
            .into_iter()
            .filter(|id| id.matches(s))
            .for_each(|id| everything.push(id));
        crate::status::all_branches(ctx)?
            .into_iter()
            .filter(|id| id.matches(s))
            .for_each(|id| everything.push(id));
        crate::log::all_commits(ctx)?
            .into_iter()
            .filter(|id| id.matches(s))
            .for_each(|id| everything.push(id));
        everything.push(CliId::unassigned());

        let mut matches = Vec::new();
        for id in everything {
            if id.matches(s) {
                matches.push(id);
            }
        }
        Ok(matches)
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
