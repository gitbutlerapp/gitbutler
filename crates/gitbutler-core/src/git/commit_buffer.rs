use anyhow::Result;
use core::str;
use std::ops::Deref;

pub struct CommitBuffer(String);

impl CommitBuffer {
    pub fn new(git2_buffer: &git2::Buf) -> Result<Self> {
        let commit_ends_in_newline = git2_buffer.ends_with(b"\n");
        let git2_buffer = str::from_utf8(git2_buffer)?;
        let lines = git2_buffer.lines();

        let mut new_buffer = String::new();

        for line in lines {
            new_buffer.push_str(line);
            new_buffer.push('\n');
        }

        if !commit_ends_in_newline {
            // strip last \n
            new_buffer.pop();
        }

        Ok(Self(new_buffer))
    }

    pub fn inject_header(&mut self, key: &str, value: &str) {
        if let Some((heading, body)) = self.split_once("\n\n") {
            let mut output = String::new();

            output.push_str(heading);
            output.push_str(format!("{} {}", key, value).as_str());
            output.push_str("\n\n");
            output.push_str(body);

            self.0 = output;
        }
    }

    pub fn inject_change_id(&mut self, change_id: Option<&str>) {
        let change_id = change_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| format!("{}", uuid::Uuid::new_v4()));

        self.inject_header("change-id", change_id.as_str());
    }
}

impl TryFrom<git2::Buf> for CommitBuffer {
    type Error = anyhow::Error;

    fn try_from(git2_buffer: git2::Buf) -> Result<Self> {
        Self::new(&git2_buffer)
    }
}

impl From<String> for CommitBuffer {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl Deref for CommitBuffer {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
