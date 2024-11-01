use std::path::PathBuf;

use anyhow::{anyhow, Context};
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
#[derive(Debug, Clone)]
pub struct InputDiff {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
}

impl InputDiff {
    pub fn net_lines(&self) -> anyhow::Result<i32> {
        self.new_lines
            .checked_signed_diff(self.old_lines)
            .ok_or(anyhow!("u32 -> i32 conversion overflow"))
    }
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

impl TryFrom<String> for InputDiff {
    fn try_from(value: String) -> Result<Self, anyhow::Error> {
        parse_diff(value)
    }

    type Error = anyhow::Error;
}

impl TryFrom<&str> for InputDiff {
    fn try_from(value: &str) -> Result<Self, anyhow::Error> {
        parse_diff(value)
    }

    type Error = anyhow::Error;
}

fn parse_diff(value: impl AsRef<str>) -> Result<InputDiff, anyhow::Error> {
    let value = value.as_ref();
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
        old_start: old_start + head_context_lines,
        old_lines: old_lines - context_lines,
        new_start: new_start + head_context_lines,
        new_lines: new_lines - context_lines,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_simple() -> anyhow::Result<()> {
        let header = InputDiff::try_from(
            "@@ -1,6 +1,7 @@
1
2
3
+4
5
6
7
",
        )?;
        assert_eq!(header.old_start, 4);
        assert_eq!(header.old_lines, 0);
        assert_eq!(header.new_start, 4);
        assert_eq!(header.new_lines, 1);
        Ok(())
    }

    #[test]
    fn diff_complex() -> anyhow::Result<()> {
        let header = InputDiff::try_from(
            "@@ -5,7 +5,6 @@
5
6
7
-8
-9
+a
10
11
",
        )?;
        assert_eq!(header.old_start, 8);
        assert_eq!(header.old_lines, 2);
        assert_eq!(header.new_start, 8);
        assert_eq!(header.new_lines, 1);
        Ok(())
    }
}
