//! The conflict markers a merge leaves in a file, as git writes them: `<<<<<<<`, an optional
//! `|||||||` before the lines both sides started from, `=======`, and `>>>>>>>`, each at the
//! start of a line.

use std::collections::HashMap;
use std::hash::{Hash as _, Hasher as _};
use std::ops::Range;

use bstr::{BStr, BString, ByteSlice as _};

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
pub fn lines(text: &[u8]) -> Vec<&BStr> {
    text.split_inclusive(|b| *b == b'\n')
        .map(BStr::new)
        .collect()
}

/// Whether `line` is a marker of seven `mark`s, alone or followed by a space and a label.
fn is_marker(line: &BStr, mark: u8) -> bool {
    (line.get(..7)).is_some_and(|head| head.iter().all(|b| *b == mark))
        && matches!(line.get(7), None | Some(b' ' | b'\n' | b'\r'))
}

fn label(line: &BStr) -> String {
    line.get(7..)
        .unwrap_or_default()
        .trim()
        .to_str_lossy()
        .into_owned()
}

/// The conflicts among `lines`, or none at all when the markers don't follow each other as
/// git writes them, as then the file is safer taken as it is.
pub fn blocks(lines: &[&BStr]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut at = 0;
    let marker_at = |ix: usize, mark: u8| lines.get(ix).is_some_and(|line| is_marker(line, mark));
    while let Some(line) = lines.get(at) {
        if !is_marker(line, b'<') {
            at += 1;
            continue;
        }
        // The next marker of `mark` from `from`, before any other block opens.
        let next = |from: usize, mark: u8| {
            (from..lines.len())
                .take_while(|ix| !marker_at(*ix, b'<'))
                .find(|ix| marker_at(*ix, mark))
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
            labels: [
                label(line),
                lines.get(end).map(|line| label(line)).unwrap_or_default(),
            ],
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
pub fn ids(lines: &[&BStr], blocks: &[Block]) -> Vec<String> {
    let mut seen: HashMap<u64, usize> = HashMap::new();
    (blocks.iter())
        .map(|block| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            for range in [
                Some(&block.ours),
                block.ancestor.as_ref(),
                Some(&block.theirs),
            ] {
                range
                    .and_then(|range| lines.get(range.clone()))
                    .hash(&mut hasher);
            }
            let hash = hasher.finish();
            let nth = seen.entry(hash).or_default();
            *nth += 1;
            format!("{hash:x}-{nth}")
        })
        .collect()
}

/// The text of `range` of `lines`, line endings and all.
pub fn text(lines: &[&BStr], range: Range<usize>) -> BString {
    lines
        .get(range)
        .unwrap_or_default()
        .iter()
        .copied()
        .collect()
}

/// `lines` with each block `kept` has a text for replaced by that text, in the block's line
/// endings. The other blocks keep their markers.
pub fn replace(lines: &[&BStr], blocks: &[Block], kept: &HashMap<usize, BString>) -> BString {
    let mut out = BString::default();
    let mut next = 0;
    for (ix, block) in blocks.iter().enumerate() {
        let Some(text) = kept.get(&ix) else {
            continue;
        };
        out.extend(
            (lines.get(next..block.lines.start).unwrap_or_default())
                .iter()
                .flat_map(|line| line.iter()),
        );
        match (lines.get(block.lines.start)).is_some_and(|line| line.ends_with(b"\r\n")) {
            true => out.extend(text.replace(b"\r\n", b"\n").replace(b"\n", b"\r\n")),
            false => out.extend_from_slice(text),
        }
        next = block.lines.end;
    }
    out.extend(
        (lines.get(next..).unwrap_or_default())
            .iter()
            .flat_map(|line| line.iter()),
    );
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use bstr::ByteSlice as _;

    use super::{blocks, ids, lines, replace, text};

    const FILE: &[u8] = b"a\n<<<<<<< ours\nmine\n||||||| base\nold\n=======\ntheirs\n>>>>>>> theirs\nb\n<<<<<<< ours\nx\n=======\ny\n>>>>>>> theirs\n";

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
        let unclosed = b"<<<<<<< ours\nx\n=======\ny\n";
        assert!(
            blocks(&lines(unclosed)).is_empty(),
            "a conflict that never closes"
        );
        let nested = b"<<<<<<< ours\n<<<<<<< ours\nx\n=======\ny\n>>>>>>> theirs\n";
        assert!(
            blocks(&lines(nested)).is_empty(),
            "a conflict opening inside another"
        );
        assert!(
            blocks(&lines(b"<<<<<<<< not a marker\n")).is_empty(),
            "eight marks aren't a marker"
        );
    }

    #[test]
    fn replaces_the_kept_conflicts_and_leaves_the_rest() {
        let lines = lines(FILE);
        let found = blocks(&lines);
        let kept = HashMap::from([(1, "kept\n".into())]);
        let replaced = replace(&lines, &found, &kept);
        assert!(
            replaced.ends_with_str("b\nkept\n"),
            "the second conflict is replaced"
        );
        assert!(
            replaced.contains_str("<<<<<<< ours\nmine\n"),
            "the first keeps its markers"
        );
        let crlf = b"<<<<<<< ours\r\nx\r\n=======\r\ny\r\n>>>>>>> theirs\r\nz\r\n";
        let crlf_lines = super::lines(crlf);
        let both = HashMap::from([(0, "x\ny\n".into())]);
        assert_eq!(
            replace(&crlf_lines, &blocks(&crlf_lines), &both),
            "x\r\ny\r\nz\r\n",
            "kept lines take the file's line endings"
        );
    }
}
