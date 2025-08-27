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

    pub fn matches(&self, s: &str) -> bool {
        s == self.to_string()
    }

    pub fn matches_prefix(&self, s: &str) -> bool {
        match self {
            CliId::Commit { oid } => {
                let full_sha = oid.to_string();
                full_sha.starts_with(s)
            }
            _ => s == self.to_string()
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
            CliId::Branch { name } => {
                write!(f, "{}", hash(name))
            }
            CliId::Unassigned => {
                write!(f, "00")
            }
            CliId::Commit { oid } => {
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
    // Convert to base 36 (0-9, a-z)
    let chars = "0123456789abcdefghijklmnopqrstuvwxyz";
    let mut result = String::new();
    for _ in 0..2 {
        result.push(chars.chars().nth((hash % 36) as usize).unwrap());
        hash /= 36;
    }
    result
}
