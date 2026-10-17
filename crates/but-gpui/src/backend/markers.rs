//! The conflict markers a merge leaves in a file, as git writes them: `<<<<<<<`, an optional
//! `|||||||` before the lines both sides started from, `=======`, and `>>>>>>>`, each at the
//! start of a line.

use std::collections::HashMap;
use std::hash::{Hash as _, Hasher as _};
use std::ops::Range;

/// One conflict, as indexes into the file's lines.
#[derive(Debug, PartialEq)]
pub struct Block {
    /// From the `<<<<<<<` line to just past the `>>>>>>>` one.
    pub lines: Range<usize>,
    /// What the opening and closing markers call the two sides.
    pub labels: [String; 2],
    pub ours: Range<usize>,
    pub ancestor: Option<Range<usize>>,
    pub theirs: Range<usize>,
}

/// The file's lines, each with its line ending.
pub fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

/// Whether `line` is a marker of seven `mark`s, alone or followed by a space and a label.
fn is_marker(line: &str, mark: u8) -> bool {
    let bytes = line.as_bytes();
    bytes.len() >= 7
        && bytes[..7].iter().all(|b| *b == mark)
        && matches!(bytes.get(7), None | Some(b' ' | b'\n' | b'\r'))
}

fn label(line: &str) -> String {
    line[7..].trim().to_owned()
}

/// The conflicts among `lines`, or none at all when the markers don't follow each other as
/// git writes them, as then the file is safer taken as it is.
pub fn blocks(lines: &[&str]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        if !is_marker(lines[at], b'<') {
            at += 1;
            continue;
        }
        // The next marker of `mark` from `from`, before any other block opens.
        let next = |from: usize, mark: u8| {
            (from..lines.len())
                .take_while(|ix| !is_marker(lines[*ix], b'<'))
                .find(|ix| is_marker(lines[*ix], mark))
        };
        let Some(split) = next(at + 1, b'=') else {
            return Vec::new();
        };
        let Some(end) = next(split + 1, b'>') else {
            return Vec::new();
        };
        let ancestor = next(at + 1, b'|').filter(|bar| *bar < split);
        blocks.push(Block {
            lines: at..end + 1,
            labels: [label(lines[at]), label(lines[end])],
            ours: at + 1..ancestor.unwrap_or(split),
            ancestor: ancestor.map(|bar| bar + 1..split),
            theirs: split + 1..end,
        });
        at = end + 1;
    }
    blocks
}

/// An id for each block that stays the same while the file around it changes: its sides'
/// text, and how many blocks with the same text come before it.
pub fn ids(lines: &[&str], blocks: &[Block]) -> Vec<String> {
    let mut seen: HashMap<u64, usize> = HashMap::new();
    (blocks.iter())
        .map(|block| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            for range in [
                Some(&block.ours),
                block.ancestor.as_ref(),
                Some(&block.theirs),
            ] {
                range.map(|range| &lines[range.clone()]).hash(&mut hasher);
            }
            let hash = hasher.finish();
            let nth = seen.entry(hash).or_default();
            *nth += 1;
            format!("{hash:x}-{nth}")
        })
        .collect()
}

/// The text of `range` of `lines`, line endings and all.
pub fn text(lines: &[&str], range: Range<usize>) -> String {
    lines[range].concat()
}

/// `lines` with each block `kept` has a text for replaced by that text, in the block's line
/// endings. The other blocks keep their markers.
pub fn replace(lines: &[&str], blocks: &[Block], kept: &HashMap<usize, String>) -> String {
    let mut out = String::new();
    let mut next = 0;
    for (ix, block) in blocks.iter().enumerate() {
        let Some(text) = kept.get(&ix) else {
            continue;
        };
        out.extend(lines[next..block.lines.start].iter().copied());
        match lines[block.lines.start].ends_with("\r\n") {
            true => out.push_str(&text.replace("\r\n", "\n").replace('\n', "\r\n")),
            false => out.push_str(text),
        }
        next = block.lines.end;
    }
    out.extend(lines[next..].iter().copied());
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{blocks, ids, lines, replace, text};

    const FILE: &str = "a\n<<<<<<< ours\nmine\n||||||| base\nold\n=======\ntheirs\n>>>>>>> theirs\nb\n<<<<<<< ours\nx\n=======\ny\n>>>>>>> theirs\n";

    #[test]
    fn finds_each_conflict_with_its_sides() {
        let lines = lines(FILE);
        let found = blocks(&lines);
        assert_eq!(found.len(), 2, "two conflicts");
        assert_eq!(
            text(&lines, found[0].ours.clone()),
            "mine\n",
            "the first side"
        );
        assert_eq!(
            found[0].ancestor.clone().map(|range| text(&lines, range)),
            Some("old\n".into()),
            "the lines both started from, in diff3 style"
        );
        assert_eq!(
            text(&lines, found[0].theirs.clone()),
            "theirs\n",
            "the second side"
        );
        assert_eq!(
            found[0].labels,
            ["ours".to_owned(), "theirs".to_owned()],
            "the labels"
        );
        assert_eq!(
            found[1].ancestor, None,
            "a conflict without the ancestor's lines"
        );
        assert_ne!(
            ids(&lines, &found)[0],
            ids(&lines, &found)[1],
            "each conflict its own id"
        );
    }

    #[test]
    fn markers_out_of_order_leave_the_file_alone() {
        let unclosed = "<<<<<<< ours\nx\n=======\ny\n";
        assert!(
            blocks(&lines(unclosed)).is_empty(),
            "a conflict that never closes"
        );
        let nested = "<<<<<<< ours\n<<<<<<< ours\nx\n=======\ny\n>>>>>>> theirs\n";
        assert!(
            blocks(&lines(nested)).is_empty(),
            "a conflict opening inside another"
        );
        assert!(
            blocks(&lines("<<<<<<<< not a marker\n")).is_empty(),
            "eight marks aren't a marker"
        );
    }

    #[test]
    fn replaces_the_kept_conflicts_and_leaves_the_rest() {
        let lines = lines(FILE);
        let found = blocks(&lines);
        let kept = HashMap::from([(1, "kept\n".to_owned())]);
        let replaced = replace(&lines, &found, &kept);
        assert!(
            replaced.ends_with("b\nkept\n"),
            "the second conflict is replaced"
        );
        assert!(
            replaced.contains("<<<<<<< ours\nmine\n"),
            "the first keeps its markers"
        );
        let crlf = "<<<<<<< ours\r\nx\r\n=======\r\ny\r\n>>>>>>> theirs\r\nz\r\n";
        let crlf_lines = super::lines(crlf);
        let both = HashMap::from([(0, "x\ny\n".to_owned())]);
        assert_eq!(
            replace(&crlf_lines, &blocks(&crlf_lines), &both),
            "x\r\ny\r\nz\r\n",
            "kept lines take the file's line endings"
        );
    }
}
