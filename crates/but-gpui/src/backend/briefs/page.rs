//! A version of a brief as the tab draws it: what it says in blocks ready to draw, and what's
//! known about it apart from what it says.

use std::collections::HashSet;
use std::sync::Arc;

use but_briefs::{Block, BlockKind, Brief};
use but_core::unified_diff::DiffHunk;

use super::since::{mark_what_is_new, still_there};
use super::*;
use crate::backend::diff::{highlight_lines, parse_hunk};
use crate::backend::{CommitId, DiffLine, LineKind};
use crate::ui::markdown;

/// What stands in for a diagram, which only the web draws, and goes to it there.
const DIAGRAM: &str = "A diagram goes here. Open it on the web";

/// How many characters say what's gone from a brief.
const GIST: usize = 90;

/// A version of a brief, ready to draw.
#[derive(Debug, Clone)]
pub struct BriefPage {
    pub id: String,
    pub version: u32,
    /// The commit the branch was at as this was read, which [`BriefPage::standing`] is true of.
    pub tip: Option<CommitId>,
    /// The commit it was written at, as far as Briefs knows.
    pub written_at: Option<String>,
    pub title: String,
    /// Seconds since the epoch.
    pub published: Option<i64>,
    pub changed: Option<Changed>,
    pub ask: Option<Ask>,
    pub labels: Vec<Label>,
    /// The agent that wrote it, in its own words.
    pub agent: Option<String>,
    pub standing: Standing,
    /// The code cards, by their blocks, that show being added what the branch doesn't have now.
    pub drifted: HashSet<usize>,
    /// It had uncommitted changes on top of the commit it was written at.
    pub uncommitted: bool,
    /// What's different from the version read before this one, if one was.
    pub since_read: Option<SinceRead>,
    /// Its pictures in order, which a preview pages through.
    pub gallery: Arc<[markdown::Image]>,
    pub blocks: Vec<BriefBlock>,
}

/// What a later version did about something said on an earlier one, by what's there rather than
/// by what anyone replied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadBack {
    /// What it's about is the same in the version shown.
    Untouched,
    /// What it's about is different there, or gone.
    Changed,
    /// It's about nothing that can be checked: the whole brief, or too old to compare.
    CannotTell,
}

impl BriefPage {
    /// What this version did about `thread`, said on the version before it.
    pub fn read_back(&self, thread: &Thread) -> ReadBack {
        let Some(since) =
            (self.since_read.as_ref()).filter(|since| thread.version == Some(since.version))
        else {
            return ReadBack::CannotTell;
        };
        if let Anchor::Text { quote, .. } = &thread.anchor
            && !quote.trim().is_empty()
        {
            // A quote can run across paragraphs, each of which is what's looked for.
            let has = |line: &str| {
                self.blocks.iter().any(|block| match &block.body {
                    BriefBody::Text(blocks) => blocks.iter().any(|block| block.text.contains(line)),
                    _ => false,
                })
            };
            let there = (quote.lines().map(str::trim))
                .filter(|line| !line.is_empty())
                .all(has);
            return if there {
                ReadBack::Untouched
            } else {
                ReadBack::Changed
            };
        }
        match (thread.anchor.block()).and_then(|block| since.kept.get(block.checked_sub(1)?)) {
            Some(true) => ReadBack::Untouched,
            Some(false) => ReadBack::Changed,
            None => ReadBack::CannotTell,
        }
    }
}

/// Whether changing `path` tends to matter beyond the code around it, whatever a brief says about
/// it: what runs against production data, and what decides what's checked and how it ships.
pub fn weighty(path: &str) -> bool {
    let lower = path.to_lowercase();
    [
        "migration",
        ".sql",
        ".github/",
        "dockerfile",
        ".env",
        "secret",
        "credential",
        "/auth",
        "permission",
    ]
    .iter()
    .any(|word| lower.contains(word))
}

/// What's different in a version from the one its reader last read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinceRead {
    pub version: u32,
    /// How many of its blocks that one didn't have as they are.
    pub changed: usize,
    /// What that one had that's gone, in a few words each.
    pub removed: Vec<String>,
    /// For each of that one's blocks, whether this one has it as it was.
    pub kept: Vec<bool>,
}

impl BriefPage {
    /// The files it shows code of, in any way a reader sees lines of them.
    pub fn code_paths(&self) -> HashSet<&str> {
        (self.blocks.iter())
            .filter_map(|block| match &block.body {
                BriefBody::Code(code) => Some(code.path.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Whether it names `path` anywhere: lists it, or says its name in words.
    pub fn mentions(&self, path: &str) -> bool {
        let name = path.rsplit('/').next().unwrap_or(path);
        self.blocks.iter().any(|block| match &block.body {
            BriefBody::Code(code) => code.path == path,
            BriefBody::Files { files, .. } => files.iter().any(|file| file.path == path),
            BriefBody::Text(blocks) => blocks.iter().any(|block| block.text.contains(name)),
            BriefBody::Pictures(_) | BriefBody::OnTheWeb(_) => false,
        })
    }
}

/// How a brief relates to its branch as that is now, worked out from the repository, so nothing
/// the agent wrote can claim otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// It was written at the commit the branch is at.
    Current,
    /// The branch has moved on from the commit it was written at.
    Since {
        /// Those commits, newest first, up to [`MOST_SINCE`].
        commits: Vec<SinceCommit>,
        /// The files they change.
        files: Vec<String>,
    },
    /// The commit it was written at isn't on the branch, as after a rebase or an amend.
    Rewritten,
    /// It names no commit.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinceCommit {
    pub id: CommitId,
    pub title: String,
}

/// Code a brief shows.
#[derive(Debug, Clone)]
pub struct BriefCode {
    pub path: String,
    pub source: CodeSource,
    /// `modified`, `added`, `deleted` or `renamed`. None for lines of a file as it stands.
    pub status: Option<String>,
    pub stats: Option<but_briefs::LineStats>,
    pub parts: Vec<Vec<DiffLine>>,
    /// What isn't shown, in the words that follow "Not shown:".
    pub omitted: Vec<String>,
}

/// Where code a brief shows comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeSource {
    /// A commit.
    Committed,
    /// Uncommitted work as the brief was written.
    Worktree,
    /// Lines the agent proposes, which exist nowhere yet.
    Proposal,
}

#[derive(Debug, Clone)]
pub struct BriefBlock {
    /// Which of the version's blocks this draws, counted from 1 as comments name them. Pictures
    /// of a group are several.
    pub positions: std::ops::RangeInclusive<usize>,
    /// The version last read didn't have it like this.
    pub fresh: bool,
    /// The line it folds away under at first, if it does.
    pub detail: Option<String>,
    pub body: BriefBody,
}

#[derive(Debug, Clone)]
pub enum BriefBody {
    /// Words, or a picture, which draws as Markdown does.
    Text(Vec<markdown::Block>),
    /// Runs of lines of a file, numbered and highlighted, with what git knows of the file.
    Code(BriefCode),
    /// The files of a comparison, as git says.
    Files {
        files: Vec<but_briefs::ChangedFile>,
        omitted: Vec<String>,
    },
    /// Pictures meant to be seen together, or one alone.
    Pictures(Pictures),
    /// What only the web shows, in a few words.
    OnTheWeb(String),
}

/// Pictures of a brief drawn in a row, at one height, so a phone and a desktop screenshot sit at
/// the same scale and the eye can go across.
#[derive(Debug, Clone)]
pub struct Pictures {
    pub pictures: Vec<BriefPicture>,
    /// Too wide together to read side by side, as strips cut from a page.
    pub stacked: bool,
}

#[derive(Debug, Clone)]
pub struct BriefPicture {
    /// Where it is in the brief's gallery.
    pub at: usize,
    /// How wide it is for how tall.
    pub ratio: f32,
    /// A few words over it that say which it is, as "Before".
    pub title: Option<String>,
    pub caption: String,
}

impl Pictures {
    /// How wide the row may be, so that it's no taller than [`Pictures::TALLEST`].
    pub fn widest(&self) -> f32 {
        match self.stacked {
            true => {
                self.pictures
                    .iter()
                    .fold(0., |most, one| one.ratio.max(most))
                    * Self::TALLEST
            }
            false => self.pictures.iter().map(|one| one.ratio).sum::<f32>() * Self::TALLEST,
        }
    }

    /// As tall as a picture gets, in design pixels, so one never takes the whole page.
    const TALLEST: f32 = 460.;
    /// Row widths above this many heights leave too little height to read, at the column's width.
    const MOST_ACROSS: f32 = 5.;
}

pub(super) fn page(
    id: &str,
    version: u32,
    brief: Brief,
    standing: Standing,
    uncommitted: bool,
    tip: Option<CommitId>,
    earlier: Option<(u32, Brief)>,
) -> BriefPage {
    let published = (brief.created_at.as_deref())
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
        .map(|at| at.timestamp());
    let review = brief.review;
    let mut since_read = earlier.as_ref().map(|(version, earlier)| SinceRead {
        version: *version,
        changed: 0,
        removed: what_is_gone(&earlier.review.blocks, &review.blocks),
        kept: (earlier.review.blocks.iter())
            .map(|block| review.blocks.iter().any(|now| now.kind == block.kind))
            .collect(),
    });
    // The words of every paragraph of the version read, to tell what's new from.
    let earlier_words: Option<Vec<String>> = earlier.as_ref().map(|(_, earlier)| {
        (earlier.review.blocks.iter())
            .filter_map(|block| match &block.kind {
                BlockKind::Text { markdown } => Some(text_blocks(markdown)),
                _ => None,
            })
            .flatten()
            .map(|block| block.text)
            .collect()
    });
    let (mut gallery, mut blocks) = (Vec::new(), Vec::<BriefBlock>::new());
    let mut last_group = None;
    for (position, Block { detail, kind }) in (1..).zip(review.blocks) {
        // Neighbours of one group go in one row, as the last one's fold.
        let group = match &kind {
            BlockKind::Media { group, .. } => group.clone(),
            _ => None,
        };
        let fresh = earlier
            .as_ref()
            .is_some_and(|(_, earlier)| !earlier.review.blocks.iter().any(|old| old.kind == kind));
        let mut body = body(id, kind, &mut gallery);
        if let BriefBody::Text(blocks) = &mut body {
            for diagram in blocks.iter_mut().filter(|block| block.text == DIAGRAM) {
                let place = format!("{}#p{position}", web_url(id, version));
                diagram.links.push((0..diagram.text.len(), place));
            }
        }
        let mut fresh = fresh;
        if let (true, BriefBody::Text(blocks), Some(old)) = (fresh, &mut body, &earlier_words) {
            mark_what_is_new(blocks, old);
            // Only its formatting changed, which there's nothing to show of.
            fresh = (blocks.iter()).any(|block| block.has_tint(markdown::Tint::Fresh));
        }
        if group.is_some()
            && group == last_group
            && let (
                BriefBody::Pictures(more),
                Some(BriefBlock {
                    detail: same,
                    body: BriefBody::Pictures(row),
                    ..
                }),
            ) = (&body, blocks.last_mut())
            && *same == detail
        {
            row.pictures.extend(more.pictures.iter().cloned());
            let across: f32 = row.pictures.iter().map(|one| one.ratio).sum();
            row.stacked = across > Pictures::MOST_ACROSS;
            if let Some(last) = blocks.last_mut() {
                last.positions = *last.positions.start()..=position;
                last.fresh |= fresh;
            }
            continue;
        }
        last_group = group;
        blocks.push(BriefBlock {
            positions: position..=position,
            fresh,
            detail,
            body,
        });
    }
    if let Some(since) = &mut since_read {
        since.changed = blocks.iter().filter(|block| block.fresh).count();
    }
    BriefPage {
        id: id.to_owned(),
        version,
        title: review.title,
        published,
        changed: review.changed,
        ask: review.ask,
        labels: review.labels,
        agent: review.generator.and_then(|generator| generator.agent),
        standing,
        drifted: HashSet::new(),
        uncommitted,
        since_read,
        tip,
        written_at: None,
        gallery: gallery.into(),
        blocks,
    }
}

/// What `earlier` had that `now` hasn't, in a few words each. Paragraph by paragraph, as a block
/// with a word changed in it, or a link's number, isn't gone, and code by its file, as the same
/// file shown with other lines isn't either.
fn what_is_gone(earlier: &[Block], now: &[Block]) -> Vec<String> {
    let paragraphs: Vec<String> = (now.iter())
        .filter_map(|block| match &block.kind {
            BlockKind::Text { markdown } => Some(text_blocks(markdown)),
            _ => None,
        })
        .flatten()
        .map(|block| block.text)
        .collect();
    let shows_file = |wanted: &str| {
        now.iter().any(|block| match &block.kind {
            BlockKind::Excerpt { path, .. } | BlockKind::Diff { path, .. } => path == wanted,
            _ => false,
        })
    };
    (earlier.iter())
        .flat_map(|block| match &block.kind {
            BlockKind::Text { markdown } => (text_blocks(markdown).into_iter())
                .map(|block| block.text)
                .filter(|text| !text.trim().is_empty() && !still_there(text, &paragraphs))
                .map(|text| in_a_few_words(&text))
                .collect(),
            BlockKind::Excerpt { path, .. } | BlockKind::Diff { path, .. } => {
                match shows_file(path) {
                    true => Vec::new(),
                    false => vec![format!("The code from {path}")],
                }
            }
            BlockKind::Files { .. } => match now.iter().any(|one| one.kind == block.kind) {
                true => Vec::new(),
                false => vec!["A list of files".to_owned()],
            },
            BlockKind::Media { alt, title, .. } => {
                match now.iter().any(|one| one.kind == block.kind) {
                    true => Vec::new(),
                    false => vec![format!("The picture {}", title.as_deref().unwrap_or(alt))],
                }
            }
            BlockKind::Unknown(kind) => match now.iter().any(|one| one.kind == block.kind) {
                true => Vec::new(),
                false => vec![format!("A {kind} block")],
            },
        })
        .collect()
}

fn in_a_few_words(text: &str) -> String {
    match text.char_indices().nth(GIST) {
        Some((end, _)) => format!("{}…", text.get(..end).unwrap_or(text)),
        None => text.to_owned(),
    }
}

/// A text block as it reads. A link to a block of the same brief goes there rather than out.
fn text_blocks(text: &str) -> Vec<markdown::Block> {
    markdown::parse_with_anchors(&without_diagrams(text))
}

fn body(id: &str, kind: BlockKind, gallery: &mut Vec<markdown::Image>) -> BriefBody {
    match kind {
        BlockKind::Text { markdown } => BriefBody::Text(text_blocks(&markdown)),
        BlockKind::Excerpt {
            at,
            path,
            range,
            lines,
        } => {
            let lines = (lines.iter().zip(range.start..))
                .map(|(text, n)| DiffLine::new(LineKind::Context, Some(n), Some(n), text))
                .collect();
            code(BriefCode {
                path,
                source: match at.as_str() {
                    "worktree" => CodeSource::Worktree,
                    _ => CodeSource::Committed,
                },
                status: None,
                stats: None,
                parts: vec![lines],
                omitted: Vec::new(),
            })
        }
        BlockKind::Diff {
            comparison,
            path,
            status,
            stats,
            hunks,
            omitted,
        } => {
            let parts = (hunks.into_iter())
                .map(|hunk| {
                    parse_hunk(&DiffHunk {
                        old_start: hunk.old_start,
                        old_lines: 0,
                        new_start: hunk.new_start,
                        new_lines: 0,
                        diff: hunk.body.into(),
                    })
                    .lines
                })
                .collect();
            code(BriefCode {
                path,
                source: match comparison.and_then(|it| it.head).as_deref() {
                    Some("worktree") => CodeSource::Worktree,
                    Some("proposal") => CodeSource::Proposal,
                    _ => CodeSource::Committed,
                },
                status,
                stats,
                parts,
                omitted,
            })
        }
        BlockKind::Files { files, omitted } => BriefBody::Files { files, omitted },
        BlockKind::Media {
            media,
            media_type,
            title,
            width,
            height,
            alt,
            group: _,
        } => match media_type.starts_with("image/") {
            true => {
                let at = gallery.len();
                gallery.push(picture(id, &media, width, height, title.as_deref(), &alt));
                BriefBody::Pictures(Pictures {
                    pictures: vec![BriefPicture {
                        at,
                        ratio: width.max(1) as f32 / height.max(1) as f32,
                        title,
                        caption: alt,
                    }],
                    stacked: false,
                })
            }
            false => BriefBody::OnTheWeb(format!("A video: {alt}")),
        },
        BlockKind::Unknown(kind) => BriefBody::OnTheWeb(format!("A {kind} block")),
    }
}

/// `markdown` with diagrams said to be on the web, as nothing here draws them and their source
/// says little.
fn without_diagrams(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut in_diagram = false;
    for line in markdown.lines() {
        match (in_diagram, line.trim_start()) {
            (false, fence) if fence.starts_with("```mermaid") => {
                in_diagram = true;
                out.push_str(&format!("*{DIAGRAM}*\n"));
            }
            (true, fence) if fence.starts_with("```") => in_diagram = false,
            (true, _) => {}
            (false, _) => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

fn code(mut code: BriefCode) -> BriefBody {
    for lines in &mut code.parts {
        highlight_lines(&code.path, lines.iter_mut());
    }
    BriefBody::Code(code)
}

/// A picture of the brief for its gallery, which says which it is as it's previewed.
fn picture(
    id: &str,
    sha: &str,
    width: u32,
    height: u32,
    title: Option<&str>,
    alt: &str,
) -> markdown::Image {
    let src = but_briefs::media_url(id, sha);
    markdown::Image {
        opens: src.clone(),
        src,
        alt: match title {
            Some(title) => format!("{title}: {alt}"),
            None => alt.to_owned(),
        },
        width: Some(width as f32),
        height: Some(height as f32),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn brief(blocks: serde_json::Value) -> Brief {
        serde_json::from_value(json!({ "review": { "title": "t", "blocks": blocks } })).unwrap()
    }

    fn page_of(blocks: serde_json::Value, earlier: Option<(u32, Brief)>) -> BriefPage {
        page(
            "b1",
            2,
            brief(blocks),
            Standing::Unknown,
            false,
            None,
            earlier,
        )
    }

    fn media(sha: &str, group: Option<&str>, (width, height): (u32, u32)) -> serde_json::Value {
        json!({
            "kind": "media", "media": sha, "type": "image/png", "width": width, "height": height,
            "alt": sha, "group": group,
        })
    }

    #[test]
    fn pictures_of_a_group_are_seen_together() {
        let page = page_of(
            json!([
                media("a", Some("compare"), (100, 100)),
                media("b", Some("compare"), (200, 100)),
                { "kind": "text", "markdown": "between" },
                media("c", None, (100, 100)),
                media("d", None, (100, 100)),
            ]),
            None,
        );
        let rows: Vec<_> = (page.blocks.iter())
            .filter_map(|block| match &block.body {
                BriefBody::Pictures(row) => Some((row.pictures.len(), block.positions.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(
            rows,
            [(2, 1..=2), (1, 4..=4), (1, 5..=5)],
            "a group is one row that answers to both blocks' names; others stand alone"
        );
        assert_eq!(page.gallery.len(), 4, "a preview pages through them all");
    }

    #[test]
    fn pictures_too_wide_to_read_across_stack() {
        let page = page_of(
            json!([
                media("a", Some("compare"), (2400, 348)),
                media("b", Some("compare"), (2400, 348)),
            ]),
            None,
        );
        let BriefBody::Pictures(row) = &page.blocks[0].body else {
            panic!("pictures draw as pictures");
        };
        assert!(
            row.stacked,
            "two strips side by side would each be a sliver"
        );
    }

    #[test]
    fn what_changed_since_the_version_read_is_marked() {
        let earlier = brief(json!([
            { "kind": "text", "markdown": "stays" },
            { "kind": "text", "markdown": "was removed\nmore of it" },
        ]));
        let page = page_of(
            json!([
                { "kind": "text", "markdown": "stays" },
                { "kind": "text", "markdown": "is new" },
            ]),
            Some((1, earlier)),
        );
        let fresh: Vec<bool> = page.blocks.iter().map(|block| block.fresh).collect();
        assert_eq!(fresh, [false, true], "only what wasn't there as it is");
        let BriefBody::Text(words) = &page.blocks[1].body else {
            panic!("words draw as words");
        };
        assert_eq!(
            (words.iter())
                .filter(|word| word.has_tint(markdown::Tint::Fresh))
                .count(),
            1,
            "and of its words, what's new"
        );
        let edited = page_of(
            json!([{ "kind": "text", "markdown": "Links go to [the rest](#p9) of it, as they did" }]),
            Some((
                1,
                brief(json!([
                    { "kind": "text", "markdown": "Links go to [the rest](#p7) of it, as they did\n\nThis one went" }
                ])),
            )),
        );
        assert_eq!(
            edited.since_read.unwrap().removed,
            ["This one went"],
            "a link's new number isn't a paragraph gone, and a paragraph gone is named alone"
        );
        assert_eq!(
            page.since_read,
            Some(SinceRead {
                version: 1,
                changed: 1,
                removed: vec!["was removed more of it".to_owned()],
                kept: vec![true, false],
            }),
            "and what's gone is said, as nothing else on the page can show it"
        );
    }

    #[test]
    fn a_diff_block_numbers_each_hunk_from_its_own_start() {
        let page = page_of(
            json!([{
                "kind": "diff",
                "path": "a.rs",
                "hunks": [
                    { "oldStart": 10, "newStart": 12, "body": " keep\n-gone\n+new\n" },
                    { "oldStart": 50, "newStart": 52, "body": " other\n" },
                ],
            }]),
            None,
        );
        let BriefBody::Code(code) = &page.blocks[0].body else {
            panic!("a diff draws as code");
        };
        let numbers = |lines: &[DiffLine]| -> Vec<_> {
            (lines.iter())
                .map(|line| (line.kind, line.old, line.new))
                .collect()
        };
        assert_eq!(
            numbers(&code.parts[0]),
            [
                (LineKind::Context, Some(10), Some(12)),
                (LineKind::Removed, Some(11), None),
                (LineKind::Added, None, Some(13)),
            ],
            "the first hunk counts each side from its own start"
        );
        assert_eq!(
            numbers(&code.parts[1]),
            [(LineKind::Context, Some(50), Some(52))],
            "the next hunk starts over from its own"
        );
    }

    #[test]
    fn a_later_version_is_held_to_what_was_said_on_the_last() {
        let earlier = brief(json!([
            { "kind": "text", "markdown": "stays" },
            { "kind": "text", "markdown": "goes" },
            { "kind": "text", "markdown": "first sentence. second sentence." },
        ]));
        let page = page_of(
            json!([
                { "kind": "text", "markdown": "stays" },
                { "kind": "text", "markdown": "came instead" },
                { "kind": "text", "markdown": "first sentence. other sentence." },
            ]),
            Some((1, earlier)),
        );
        let thread = |anchor: serde_json::Value, version: u32| -> Thread {
            serde_json::from_value(json!({
                "id": "c", "actor": { "name": "Ada" }, "body": "b",
                "anchor": anchor, "version": version,
            }))
            .unwrap()
        };
        let on = |block: &str| json!({ "on": "block", "block": block });
        assert_eq!(page.read_back(&thread(on("p1"), 1)), ReadBack::Untouched);
        assert_eq!(page.read_back(&thread(on("p2"), 1)), ReadBack::Changed);
        assert_eq!(
            page.read_back(&thread(
                json!({ "on": "text", "block": "p3", "quote": "first sentence" }),
                1
            )),
            ReadBack::Untouched,
            "a passage goes by its own words, whatever else changed around it"
        );
        assert_eq!(
            page.read_back(&thread(json!({ "on": "brief" }), 1)),
            ReadBack::CannotTell
        );
        assert_eq!(
            page.read_back(&thread(on("p1"), 0)),
            ReadBack::CannotTell,
            "one said further back wasn't compared"
        );
    }

    #[test]
    fn what_tends_to_matter_is_told_from_the_rest() {
        assert!(weighty("packages/web/migrations/036_previews.sql"));
        assert!(weighty(".github/workflows/ci.yml"));
        assert!(!weighty("packages/web/src/share-card.tsx"));
    }

    #[test]
    fn a_proposal_is_never_taken_for_code_that_exists() {
        let page = page_of(
            json!([{
                "kind": "diff",
                "path": "a.rs",
                "comparison": { "head": "proposal" },
                "hunks": [],
            }]),
            None,
        );
        let BriefBody::Code(code) = &page.blocks[0].body else {
            panic!("a diff draws as code");
        };
        assert_eq!(code.source, CodeSource::Proposal);
    }

    #[test]
    fn a_diagram_says_where_it_shows_rather_than_its_source() {
        let page = page_of(
            json!([{ "kind": "text", "markdown": "Before\n\n```mermaid\nflowchart TB\n  a --> b\n```\n\nAfter" }]),
            None,
        );
        let BriefBody::Text(blocks) = &page.blocks[0].body else {
            panic!("words draw as words");
        };
        let text: Vec<&str> = blocks.iter().map(|block| block.text.as_str()).collect();
        assert_eq!(text, ["Before", DIAGRAM, "After"]);
        assert!(
            blocks[1].links[0].1.ends_with("/b/b1/v/2#p1"),
            "and it goes to where that part is on the web"
        );
    }

    #[test]
    fn a_link_to_a_block_stays_in_the_brief() {
        let page = page_of(
            json!([{ "kind": "text", "markdown": "See [the rule](#p2) and [more](https://x.dev)." }]),
            None,
        );
        let BriefBody::Text(blocks) = &page.blocks[0].body else {
            panic!("words draw as words");
        };
        let links: Vec<&str> = blocks[0]
            .links
            .iter()
            .map(|(_, url)| url.as_str())
            .collect();
        assert_eq!(links, ["anchor:p2", "https://x.dev"]);
    }
}
