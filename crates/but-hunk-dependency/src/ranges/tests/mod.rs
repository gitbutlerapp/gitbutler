use anyhow::{Context as _, anyhow};

use crate::InputDiffHunk;

mod path;
mod path_utilities;
mod workspace;

/// Create a new object id by repeating the given `hex_char`.
fn id_from_hex_char(hex_char: char) -> gix::ObjectId {
    gix::ObjectId::from_hex(String::from_iter(std::iter::repeat_n(hex_char, 40)).as_bytes())
        .expect("input char is hex-only")
}

fn input_hunk_from_unified_diff(diff: &str) -> Result<InputDiffHunk, anyhow::Error> {
    let header = diff.lines().next().context("No header found")?;
    if !header.starts_with("@@") {
        return Err(anyhow!("Malformed undiff"));
    }
    let parts: Vec<&str> = header.split_whitespace().collect();
    let (old_start, old_lines) = parse_header(parts[1]);
    let (new_start, new_lines) = parse_header(parts[2]);
    let head_context_lines = count_context_lines(diff.lines().skip(1).take(3));
    let tail_context_lines = count_context_lines(diff.rsplit_terminator('\n').take(3));
    let context_lines = head_context_lines + tail_context_lines;

    Ok(InputDiffHunk {
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
