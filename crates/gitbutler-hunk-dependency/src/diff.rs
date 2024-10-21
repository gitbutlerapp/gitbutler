use anyhow::{anyhow, Context};

#[derive(Debug, PartialEq, Clone)]
pub struct InputDiff {
    pub old_start: i32,
    pub old_lines: i32,
    pub new_start: i32,
    pub new_lines: i32,
}

impl InputDiff {
    pub fn net_lines(&self) -> i32 {
        self.new_lines - self.old_lines
    }
}

fn count_context_lines<I, S>(iter: I) -> i32
where
    I: Iterator<Item = S>,
    S: AsRef<str>,
{
    iter.take_while(|line| {
        let line_ref = line.as_ref(); // Convert to &str
        !line_ref.starts_with('-') && !line_ref.starts_with('+')
    })
    .fold(0i32, |acc, _| acc + 1)
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

fn parse_header(hunk_info: &str) -> (i32, i32) {
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
