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
    #[allow(dead_code)]
    Commit {
        oid: gix::ObjectId,
    },
}

impl CliId {
    const UNCOMMITED_FILE: &str = "j";
    const BRANCH: &str = "r";
    const COMMIT: &str = "";

    pub fn commit(oid: gix::ObjectId) -> Self {
        CliId::Commit { oid }
    }

    pub fn branch(name: &str) -> Self {
        CliId::Branch {
            name: name.to_owned(),
        }
    }

    pub fn file_from_assignment(assignment: &HunkAssignment) -> Self {
        CliId::UncommittedFile {
            path: assignment.path.clone(),
            assignment: assignment.stack_id,
        }
    }

    fn prefix(&self) -> &str {
        match self {
            CliId::UncommittedFile { .. } => CliId::UNCOMMITED_FILE,
            CliId::Branch { .. } => CliId::BRANCH,
            CliId::Commit { .. } => CliId::COMMIT,
        }
    }

    pub fn matches(&self, s: &str) -> bool {
        s == self.to_string()
    }

    #[allow(dead_code)]
    pub fn from_str(ctx: &mut CommandContext, s: &str) -> anyhow::Result<Vec<Self>> {
        if s.len() < 3 {
            return Err(anyhow::anyhow!("Id needs to be 3 characters long: {}", s));
        }
        let s = &s[..3];
        if s.starts_with(CliId::UNCOMMITED_FILE) {
            crate::status::file_from_hash(ctx, s)
        } else if s.starts_with(CliId::BRANCH) {
            crate::status::branch_from_hash(ctx, s)
        } else {
            crate::log::commit_from_hash(ctx, s)
        }
    }
}

impl Display for CliId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliId::UncommittedFile { path, assignment } => {
                if let Some(assignment) = assignment {
                    let value = hash(&format!("{}{}", assignment, path));
                    write!(f, "{}{}", self.prefix(), value)
                } else {
                    write!(f, "{}{}", self.prefix(), hash(path))
                }
            }
            CliId::Branch { name } => {
                write!(f, "{}{}", self.prefix(), hash(name))
            }
            CliId::Commit { oid } => {
                let oid = oid.to_string();
                write!(f, "{}", &oid[..3])
            }
        }
    }
}

fn hash(input: &str) -> String {
    let mut hash = 0u64;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    // Convert to base 62 (0-9, a-z, A-Z)
    let chars = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut result = String::new();
    for _ in 0..2 {
        result.push(chars.chars().nth((hash % 62) as usize).unwrap());
        hash /= 62;
    }
    result
}
