use std::path::PathBuf;

use anyhow::{Context, anyhow};
use gitbutler_stack::StackId;

#[derive(Debug, Clone)]
pub struct InputStack {
    pub stack_id: StackId,
    /// The commits in the stack.
    ///
    /// The commits are ordered from the base to the top of the stack (application order).
    pub commits: Vec<InputCommit>,
}

#[derive(Debug, Clone)]
pub struct InputCommit {
    pub commit_id: git2::Oid,
    pub files: Vec<InputFile>,
}

#[derive(Debug, Clone)]
pub struct InputFile {
    pub path: PathBuf,
    pub diffs: Vec<InputDiff>,
}

/// Please note that the From conversions and parsing of diffs exists to facilitate testing, in
/// the client code we get the line numbers from elsewhere.
#[derive(Debug, Clone, Copy)]
pub struct InputDiff {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub change_type: gitbutler_diff::ChangeType,
}

impl InputDiff {
    pub fn net_lines(&self) -> anyhow::Result<i32> {
        // TODO: use `checked_signed_diff` instead when stable.
        (self.new_lines as i64)
            .checked_sub(self.old_lines as i64)
            .and_then(|n| i32::try_from(n).ok())
            .ok_or(anyhow!("u32 -> i32 conversion overflow"))
    }
}

pub fn parse_diff_from_string(
    value: &str,
    change_type: gitbutler_diff::ChangeType,
) -> Result<InputDiff, anyhow::Error> {
    let header = value.lines().next().context("No header found")?;
    if !header.starts_with("@@") {
        return Err(anyhow!("Malformed undiff"));
    }
    let parts: Vec<&str> = header.split_whitespace().collect();
    let (old_start, old_lines) = parse_header(parts[1]);
    let (new_start, new_lines) = parse_header(parts[2]);
    let head_context_lines = count_context_lines(value.lines().skip(1).take(3));
    let tail_context_lines = count_context_lines(value.rsplit_terminator('\n').take(3));
    let context_lines = head_context_lines + tail_context_lines;

    Ok(InputDiff {
        change_type,
        old_start: old_start + head_context_lines,
        old_lines: old_lines - context_lines,
        new_start: new_start + head_context_lines,
        new_lines: new_lines - context_lines,
    })
}

fn count_context_lines<I, S>(iter: I) -> u32
where
    I: Iterator<Item = S>,
    S: AsRef<str>,
{
    iter.take_while(|line| {
        let line_ref = line.as_ref(); // Convert to &str
        !line_ref.starts_with('-') && !line_ref.starts_with('+')
    })
    .fold(0u32, |acc, _| acc + 1)
}

fn parse_header(hunk_info: &str) -> (u32, u32) {
    let hunk_info = hunk_info.trim_start_matches(&['-', '+'][..]); // Remove the leading '-' or '+'
    let parts: Vec<&str> = hunk_info.split(',').collect();
    let start = parts[0].parse().unwrap();
    let lines = if parts.len() > 1 {
        parts[1].parse().unwrap()
    } else {
        1
    };
    (start, lines)
}
