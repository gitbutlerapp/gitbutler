//! Code an agent names in what it wrote, joined to that code on the branch: its name shows the
//! lines on hover and goes to them on a click. The lines come from the repository, so a reader
//! can check a sentence without taking the agent's word for it.
//!
//! A name the diff doesn't have looks like any other code, and only a click on it says so. Of the
//! names in real briefs that aren't in the code at all, every one was a fair thing to write, as a
//! term from another project's docs, so marking them would be crying wolf.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::backend::briefs::{BriefBody, BriefPage};
use crate::backend::{DiffLine, FileDiff, LineKind};
use crate::ui::markdown::{ANCHOR, Block, CODE_ANCHOR, Peek};

/// How many lines a hover shows above the line that has the name.
const PEEK_ABOVE: usize = 2;

/// How many lines a hover shows in all.
const PEEK_LINES: usize = 9;

/// The words before a name where a line declares it, which says more than where it's used.
const DECLARES: [&str; 13] = [
    "fn",
    "function",
    "const",
    "let",
    "def",
    "class",
    "struct",
    "enum",
    "type",
    "interface",
    "trait",
    "table",
    "TABLE",
];

/// What a span of code in a brief's words can be found by.
#[derive(Debug, PartialEq)]
enum Name<'a> {
    File(&'a str),
    Word(&'a str),
}

/// What `span` names, if it looks like code. A plain lowercase word is left out, as `title` or
/// `null` is in any diff and showing where says nothing.
fn name_of(span: &str) -> Option<Name<'_>> {
    let (bare, called) = match span.strip_suffix("()") {
        Some(bare) => (bare, true),
        None => (span, false),
    };
    let file_like = bare.contains('/')
        || (bare.rsplit_once('.')).is_some_and(|(_, extension)| {
            (1..=5).contains(&extension.len())
                && extension.chars().all(|c| c.is_ascii_alphanumeric())
        });
    if !called && file_like {
        let fits = |c: char| c.is_alphanumeric() || "._-/@".contains(c);
        return bare.chars().all(fits).then_some(Name::File(bare));
    }
    let word = bare.rsplit(['.', ':', '#']).next()?;
    let wordy = word.chars().all(|c| c.is_alphanumeric() || c == '_');
    let code_like = called || word.contains('_') || word.chars().any(char::is_uppercase);
    (wordy && code_like && word.len() >= 3).then_some(Name::Word(word))
}

/// The best place found for a word so far, the lower its rank the better.
struct Place {
    rank: u8,
    file: usize,
    hunk: usize,
    line: usize,
}

/// How much it says that `word` is at `at` in a line of `kind`: where it's added or stands over
/// where it's taken away, and where it's declared over where it's used.
fn rank(kind: LineKind, text: &str, at: usize) -> u8 {
    let before = text.get(..at).unwrap_or_default().trim_end();
    let declared = before
        .rsplit(|c: char| !c.is_alphanumeric())
        .next()
        .is_some_and(|word| DECLARES.contains(&word));
    let kept = match kind {
        LineKind::Added => 0,
        LineKind::Context => 1,
        LineKind::Removed => 2,
    };
    kept * 2 + u8::from(!declared)
}

/// The words of `text` and where each starts.
fn words(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|word| !word.is_empty())
        .map(move |word| (word.as_ptr() as usize - text.as_ptr() as usize, word))
}

/// Where in `files` each of `wanted` is, reading every line once.
fn places<'a>(wanted: &HashSet<&'a str>, files: &[FileDiff]) -> HashMap<&'a str, Place> {
    let mut found: HashMap<&str, Place> = HashMap::new();
    for (file_ix, file) in files.iter().enumerate() {
        for (hunk_ix, hunk) in file.hunks().iter().enumerate() {
            for (line_ix, line) in hunk.lines.iter().enumerate() {
                for (at, word) in words(&line.text) {
                    let Some(name) = wanted.get(word) else {
                        continue;
                    };
                    let rank = rank(line.kind, &line.text, at);
                    if found.get(name).is_none_or(|place| rank < place.rank) {
                        found.insert(
                            name,
                            Place {
                                rank,
                                file: file_ix,
                                hunk: hunk_ix,
                                line: line_ix,
                            },
                        );
                    }
                }
            }
        }
    }
    found
}

/// The lines around line `at` of `lines`.
fn around(lines: &[DiffLine], at: usize) -> Arc<[DiffLine]> {
    let start = at.saturating_sub(PEEK_ABOVE);
    let end = (start + PEEK_LINES).min(lines.len());
    lines.get(start..end).unwrap_or_default().into()
}

/// The stretches of code in `block`, their text, and whether a link is on them already. A link
/// that leaves the page says where on hover, so what that's on is left out.
fn code_spans(block: &Block) -> Vec<(std::ops::Range<usize>, &str, bool)> {
    let link_on = |range: &std::ops::Range<usize>| {
        (block.links.iter())
            .find(|(link, _)| link.start < range.end && range.start < link.end)
            .map(|(_, url)| url.as_str())
    };
    (block.styles.iter())
        .filter(|(_, style)| style.code)
        .filter(|(range, _)| link_on(range).is_none_or(|url| url.starts_with(ANCHOR)))
        .filter_map(|(range, _)| {
            let linked = link_on(range).is_some();
            Some((range.clone(), block.text.get(range.clone())?, linked))
        })
        .collect()
}

/// Every text block of `page`, with its words for who read the version before.
fn text_blocks(page: &mut BriefPage) -> impl Iterator<Item = &mut Block> {
    page.blocks.iter_mut().flat_map(|block| {
        let body = match &mut block.body {
            BriefBody::Text(blocks) => blocks.as_mut_slice(),
            _ => &mut [],
        };
        body.iter_mut().chain(
            block
                .only_new
                .iter_mut()
                .flat_map(|blocks| blocks.iter_mut()),
        )
    })
}

/// Which of `files` a name in words is: the one it names whole, else the only one it ends.
fn file_named(files: &[FileDiff], name: &str) -> Option<usize> {
    if let Some(exact) = files.iter().position(|file| file.file.path == name) {
        return Some(exact);
    }
    let mut ending = (files.iter().enumerate()).filter(|(_, file)| {
        (file.file.path.strip_suffix(name)).is_some_and(|before| before.ends_with('/'))
    });
    let (first, _) = ending.next()?;
    ending.next().is_none().then_some(first)
}

/// How file `path` of `files` starts to change, to show on hover.
pub(super) fn peek_of_file(files: &[FileDiff], path: &str) -> Option<Peek> {
    let file = files.iter().find(|file| file.file.path == path)?;
    let hunk = file.hunks().first()?;
    Some(Peek {
        path: path.to_owned(),
        lines: around(&hunk.lines, 0),
    })
}

/// Make the names of code in `page`'s words links to where `files` have them.
pub(super) fn link_names(page: &mut BriefPage, files: &[FileDiff]) {
    link_blocks(text_blocks(page).collect(), files);
}

/// Make the names of code in `blocks` links to where `files` have them, in place of any they had.
fn link_blocks(mut blocks: Vec<&mut Block>, files: &[FileDiff]) {
    let mut wanted: HashSet<String> = HashSet::new();
    for block in blocks.iter_mut() {
        block.links.retain(|(_, url)| !url.starts_with(CODE_ANCHOR));
        block.peeks.clear();
        for (_, span, _) in code_spans(block) {
            if let Some(Name::Word(word)) = name_of(span) {
                wanted.insert(word.to_owned());
            }
        }
    }
    let wanted: HashSet<&str> = wanted.iter().map(String::as_str).collect();
    let found = places(&wanted, files);
    for block in blocks.iter_mut() {
        let links: Vec<_> = code_spans(block)
            .into_iter()
            .filter_map(|(range, span, linked)| {
                let name = name_of(span)?;
                let place = match name {
                    Name::Word(word) => found.get(word).and_then(|place| {
                        let hunk = files.get(place.file)?.hunks().get(place.hunk)?;
                        Some((
                            place.file,
                            place.hunk,
                            place.line,
                            around(&hunk.lines, place.line),
                        ))
                    }),
                    Name::File(name) => {
                        let name = name.strip_prefix("./").unwrap_or(name);
                        file_named(files, name).and_then(|file| {
                            let hunk = files.get(file)?.hunks().first()?;
                            Some((file, 0, 0, around(&hunk.lines, 0)))
                        })
                    }
                };
                let (file, hunk, line, lines) = place?;
                let path = files.get(file)?.file.path.clone();
                // By path, as the order of the diff's files changes with how they're sorted.
                let url = format!("{CODE_ANCHOR}{hunk}:{line}:{path}");
                let url = (!linked).then_some(url);
                Some((range, url, Some(Peek { path, lines })))
            })
            .collect();
        for (range, url, peek) in links {
            block.links.extend(url.map(|url| (range.clone(), url)));
            block.peeks.extend(peek.map(|peek| (range, peek)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file with one hunk of `lines`, each a kind of line and its text.
    fn file_of(path: &str, lines: &[(LineKind, &str)]) -> FileDiff {
        use crate::backend::{DiffBody, FileChange, Hunk, HunkKey};
        let hunk = Hunk {
            key: HunkKey(but_core::HunkHeader {
                old_start: 1,
                old_lines: 1,
                new_start: 1,
                new_lines: 1,
            }),
            header: String::new(),
            patch: "".into(),
            lines: (lines.iter())
                .map(|(kind, text)| DiffLine::new(*kind, Some(1), Some(1), text))
                .collect(),
            scope: Vec::new(),
            lead: Vec::new(),
            tail: Vec::new(),
        };
        FileDiff {
            file: FileChange::bare(path),
            body: DiffBody::Hunks {
                hunks: vec![hunk],
                added: 0,
                removed: 0,
                widest: [1, 1],
                conflicts: Vec::new(),
            },
            fingerprint: 0,
            annotated: true,
            scope_lines: None,
            drawing: None,
            commit: None,
        }
    }

    #[test]
    fn what_looks_like_code_is_looked_for() {
        assert_eq!(name_of("previewable()"), Some(Name::Word("previewable")));
        assert_eq!(
            name_of("Kind::takes_group()"),
            Some(Name::Word("takes_group"))
        );
        assert_eq!(name_of("team_id"), Some(Name::Word("team_id")));
        assert_eq!(name_of("CardHead"), Some(Name::Word("CardHead")));
        assert_eq!(
            name_of("share-card.test.tsx"),
            Some(Name::File("share-card.test.tsx"))
        );
        assert_eq!(
            name_of("crates/briefs/src/skill/"),
            Some(Name::File("crates/briefs/src/skill/"))
        );
    }

    #[test]
    fn what_could_be_any_word_or_a_command_is_not() {
        for span in ["title", "null", "pnpm check", "--listed", "...", "a()"] {
            assert_eq!(
                name_of(span),
                None,
                "{span} says nothing about where to look"
            );
        }
    }

    #[test]
    fn a_declaration_says_more_than_a_use_and_added_more_than_removed() {
        let added = |text| rank(LineKind::Added, text, text.find("name").unwrap());
        let removed = |text| rank(LineKind::Removed, text, text.find("name").unwrap());
        assert!(added("fn name() {") < added("    name();"));
        assert!(added("    name();") < removed("fn name() {"));
    }

    #[test]
    fn a_name_goes_to_where_it_is_declared_on_the_branch() {
        let files = [
            file_of("a.ts", &[(LineKind::Added, "use(readableCard);")]),
            file_of(
                "b.ts",
                &[
                    (LineKind::Removed, "function readableCard() {"),
                    (LineKind::Added, "function readableCard(id) {"),
                ],
            ),
        ];
        let wanted = HashSet::from(["readableCard"]);
        let place = places(&wanted, &files);
        let place = &place["readableCard"];
        assert_eq!(
            (place.file, place.line),
            (1, 1),
            "the added declaration beats a use and the line it replaced"
        );
    }

    #[test]
    fn a_name_links_once_however_often_it_is_joined_up() {
        let files = [file_of(
            "src/card.ts",
            &[(LineKind::Added, "function readableCard() {")],
        )];
        let mut blocks = crate::ui::markdown::parse("Asks `readableCard()` and `title`");
        for _ in 0..2 {
            link_blocks(blocks.iter_mut().collect(), &files);
        }
        assert_eq!(
            blocks[0]
                .links
                .iter()
                .map(|(_, url)| url.as_str())
                .collect::<Vec<_>>(),
            ["anchor:code:0:0:src/card.ts"],
            "and a plain word isn't one"
        );
        assert_eq!(blocks[0].peeks.len(), 1, "with the lines it shows on hover");
    }

    #[test]
    fn a_file_is_named_whole_or_by_an_ending_only_it_has() {
        let files = [
            file_of("a/content.tsx", &[]),
            file_of("b/content.tsx", &[]),
            file_of("content.tsx", &[]),
            file_of("c/other.ts", &[]),
        ];
        assert_eq!(file_named(&files, "content.tsx"), Some(2));
        assert_eq!(file_named(&files, "other.ts"), Some(3));
        assert_eq!(
            file_named(&files[..2], "content.tsx"),
            None,
            "either could be meant"
        );
    }

    #[test]
    fn words_say_where_they_start() {
        let found: Vec<_> = words("let row_id = get(id);").collect();
        assert_eq!(found[1], (4, "row_id"));
        assert_eq!(found[3], (17, "id"));
    }
}
