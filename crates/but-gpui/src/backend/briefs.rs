//! Briefs: the one that fits a branch best, ready to draw. This asks over the network, so it runs
//! off the UI thread like the rest. Nothing is kept on disk.

use std::path::PathBuf;

use anyhow::{Result, bail};
use but_api::legacy::{forge, users};
use but_briefs::{Block, BlockKind, Brief, Client, Found, How};
use but_core::unified_diff::DiffHunk;

use super::diff::{highlight_lines, parse_hunk};
use super::{CommitId, DiffLine, LineKind, open, runtime};
use crate::ui::{markdown, plural};

pub use but_briefs::Changed;

/// A branch of the project at `path`, as Briefs is asked about it: by its name and its commits,
/// newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefAnchor {
    pub path: PathBuf,
    pub branch: String,
    pub commits: Vec<CommitId>,
}

impl BriefAnchor {
    pub fn new(path: PathBuf, branch: &str, commits: impl Iterator<Item = CommitId>) -> Self {
        Self {
            path,
            branch: branch.to_owned(),
            commits: commits.take(but_briefs::MOST_COMMITS).collect(),
        }
    }
}

/// A client signed in as whoever's signed in to GitButler, if anyone is. Only this reads the
/// keychain, so nothing that isn't asking Briefs anything does. A profile can outlive its token,
/// as in a debug build, which reads it through `git credential`. That looks the same as having
/// no briefs, so it says so at a level that reaches the log.
fn client() -> Result<Option<Client>> {
    users::access_token()
        .inspect_err(|err| tracing::warn!("Can't read GitButler's token for Briefs: {err:#}"))?
        .map(Client::new)
        .transpose()
}

/// The brief that fits `anchor` best, at the version that does. None without one, or for someone
/// not signed in.
pub fn best(anchor: &BriefAnchor) -> Result<Option<BriefPage>> {
    let Some(client) = client()? else {
        return Ok(None);
    };
    let ctx = open(&anchor.path)?;
    let remote = forge::remote_url(&ctx.project_meta()?, &*ctx.repo.get()?)?;
    let commits: Vec<String> = anchor.commits.iter().map(|id| id.0.to_string()).collect();
    let runtime = runtime().handle();
    let found = runtime.block_on(client.find(&remote, &anchor.branch, &commits))?;
    let Some(found) = fitting(found) else {
        return Ok(None);
    };
    let brief = runtime.block_on(client.read(&found.id, found.matched.version))?;
    Ok(Some(page(found, brief, &anchor.commits)))
}

/// The best of `found` that's about the branch. One that only names a commit of it isn't: that
/// can be no more than where another change starts from.
fn fitting(found: Vec<Found>) -> Option<Found> {
    (found.into_iter()).find(|found| found.matched.how != How::Commit)
}

/// What `url`, a brief's media file, serves.
pub fn media(url: &str) -> Result<Vec<u8>> {
    let Some(client) = client()? else {
        bail!("Sign in to GitButler to read briefs");
    };
    (runtime().handle()).block_on(client.media(url))
}

/// A version of a brief, ready to draw.
#[derive(Debug, Clone)]
pub struct BriefPage {
    pub id: String,
    pub title: String,
    pub author: String,
    pub version: u32,
    /// Its latest published version, which may be newer.
    pub latest: u32,
    /// Where to read it in a browser.
    pub url: String,
    /// How it stands against the branch, in a few words.
    pub standing: String,
    /// Seconds since the epoch.
    pub published: Option<i64>,
    pub changed: Option<Changed>,
    pub blocks: Vec<BriefBlock>,
}

#[derive(Debug, Clone)]
pub struct BriefBlock {
    /// The line it folds away under at first, if it does.
    pub detail: Option<String>,
    pub body: BriefBody,
}

#[derive(Debug, Clone)]
pub enum BriefBody {
    /// Words, or a picture, which draws as Markdown does.
    Text(Vec<markdown::Block>),
    /// Runs of lines of a file, numbered and highlighted.
    Code {
        path: String,
        parts: Vec<Vec<DiffLine>>,
    },
    /// A picture as Markdown draws one, with the words over it that say which it is, and what
    /// it shows under it.
    Picture {
        title: Option<String>,
        image: Vec<markdown::Block>,
        caption: String,
    },
    /// What only the web shows, in a few words.
    OnTheWeb(String),
}

fn page(found: Found, brief: Brief, commits: &[CommitId]) -> BriefPage {
    let standing = standing(&found, commits);
    let published = (brief.created_at.as_deref())
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
        .map(|at| at.timestamp());
    let version = found.matched.version;
    let url = but_briefs::web_url(&found.id, version);
    let review = brief.review;
    let blocks = (review.blocks.into_iter())
        .map(|Block { detail, kind }| BriefBlock {
            detail,
            body: body(&found.id, kind),
        })
        .collect();
    BriefPage {
        id: found.id,
        title: review.title,
        author: found.author.name,
        version,
        latest: found.latest,
        url,
        standing,
        published,
        changed: review.changed,
        blocks,
    }
}

/// Where `found` was written, against a branch's `commits`, newest first.
fn standing(found: &Found, commits: &[CommitId]) -> String {
    let below = (found.matched.head.as_deref())
        .and_then(|head| gix::ObjectId::from_hex(head.as_bytes()).ok())
        .and_then(|head| commits.iter().position(|commit| commit.0 == head));
    let words = match (found.matched.how, below) {
        (How::Head, Some(0)) => "Written at this branch's latest commit".to_owned(),
        (How::Head, Some(below)) => format!("Written {} ago", plural(below, "commit")),
        (How::Branch, _) => "Written on a branch of this name".to_owned(),
        _ => "Related to this branch".to_owned(),
    };
    match found.matched.uncommitted {
        true => format!("{words}, with uncommitted changes"),
        false => words,
    }
}

fn body(id: &str, kind: BlockKind) -> BriefBody {
    match kind {
        BlockKind::Text { markdown } => BriefBody::Text(markdown::parse(&markdown)),
        BlockKind::Excerpt { path, range, lines } => {
            let lines = (lines.iter().zip(range.start..))
                .map(|(text, n)| DiffLine::new(LineKind::Context, Some(n), Some(n), text))
                .collect();
            code(path, vec![lines])
        }
        BlockKind::Diff { path, hunks } => {
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
            code(path, parts)
        }
        BlockKind::Media {
            media,
            media_type,
            title,
            width,
            height,
            alt,
        } => match media_type.starts_with("image/") {
            true => BriefBody::Picture {
                title,
                image: vec![picture(id, &media, width, height, alt.clone())],
                caption: alt,
            },
            false => BriefBody::OnTheWeb(format!("A video: {alt}")),
        },
        BlockKind::Unknown(kind) => BriefBody::OnTheWeb(format!("A {kind} block")),
    }
}

fn code(path: String, mut parts: Vec<Vec<DiffLine>>) -> BriefBody {
    for lines in &mut parts {
        highlight_lines(&path, lines.iter_mut());
    }
    BriefBody::Code { path, parts }
}

/// A picture of the brief as Markdown draws one, which fetches it, keeps it and previews it.
fn picture(id: &str, sha: &str, width: u32, height: u32, alt: String) -> markdown::Block {
    let src = but_briefs::media_url(id, sha);
    markdown::Block {
        kind: markdown::Kind::Images(vec![markdown::Image {
            opens: src.clone(),
            src,
            alt: alt.clone(),
            width: Some(width as f32),
            height: Some(height as f32),
        }]),
        depth: 0,
        quoted: false,
        list: 0,
        text: alt,
        styles: Vec::new(),
        links: Vec::new(),
        centered: false,
        cell: None,
    }
}

#[cfg(test)]
mod tests {
    use but_briefs::{Author, Match};

    use super::*;

    fn id(n: u8) -> CommitId {
        CommitId(gix::ObjectId::from_hex(format!("{n:040x}").as_bytes()).unwrap())
    }

    fn found(how: How, head: Option<CommitId>) -> Found {
        Found {
            id: "b1".into(),
            author: Author { name: "Ada".into() },
            latest: 1,
            matched: Match {
                how,
                version: 1,
                head: head.map(|id| id.0.to_string()),
                uncommitted: false,
            },
        }
    }

    #[test]
    fn a_brief_stands_by_where_it_was_written_on_the_branch() {
        let commits = [id(3), id(2), id(1)];
        for (found, expected) in [
            (
                found(How::Head, Some(id(3))),
                "Written at this branch's latest commit",
            ),
            (found(How::Head, Some(id(1))), "Written 2 commits ago"),
            (
                found(How::Branch, Some(id(9))),
                "Written on a branch of this name",
            ),
            (found(How::Other, None), "Related to this branch"),
        ] {
            assert_eq!(
                standing(&found, &commits),
                expected,
                "{:?} on commits newest first",
                found.matched.how
            );
        }
    }

    #[test]
    fn a_brief_that_only_names_a_commit_is_not_about_the_branch() {
        assert_eq!(
            fitting(vec![found(How::Commit, Some(id(1)))]),
            None,
            "another change starting from this branch's tip names it too"
        );
        assert_eq!(
            fitting(vec![
                found(How::Commit, Some(id(1))),
                found(How::Branch, Some(id(2)))
            ]),
            Some(found(How::Branch, Some(id(2)))),
            "one that is about it still counts"
        );
    }

    #[test]
    fn a_diff_block_numbers_each_hunk_from_its_own_start() {
        let body = body(
            "b1",
            BlockKind::Diff {
                path: "a.rs".into(),
                hunks: vec![
                    but_briefs::DiffHunk {
                        old_start: 10,
                        new_start: 12,
                        body: " keep\n-gone\n+new\n".into(),
                    },
                    but_briefs::DiffHunk {
                        old_start: 50,
                        new_start: 52,
                        body: " other\n".into(),
                    },
                ],
            },
        );
        let BriefBody::Code { parts, .. } = body else {
            panic!("a diff draws as code");
        };
        let numbers = |lines: &[DiffLine]| -> Vec<_> {
            (lines.iter())
                .map(|line| (line.kind, line.old, line.new))
                .collect()
        };
        assert_eq!(
            numbers(&parts[0]),
            [
                (LineKind::Context, Some(10), Some(12)),
                (LineKind::Removed, Some(11), None),
                (LineKind::Added, None, Some(13)),
            ],
            "the first hunk counts each side from its own start"
        );
        assert_eq!(
            numbers(&parts[1]),
            [(LineKind::Context, Some(50), Some(52))],
            "the next hunk starts over from its own"
        );
    }
}
