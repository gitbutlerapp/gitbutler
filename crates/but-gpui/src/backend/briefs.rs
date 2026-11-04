//! Briefs: what's kept of them, asking about a project's branches to keep it current, and reading
//! one to draw. Asking goes over the network and reading the cache touches the disk, so all of
//! it runs off the UI thread like the rest. `but-briefs` keeps the file, and this says who for.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use but_api::legacy::{forge, users};
use but_briefs::{Block, BlockKind, BranchAsk, Brief, Cache, Client, How};
use but_core::unified_diff::DiffHunk;
use parking_lot::Mutex;

use super::diff::{highlight_lines, parse_hunk};
use super::{CommitId, DiffLine, LineKind, open, runtime};
use crate::ui::{markdown, plural};

pub use but_briefs::{Anchored, Changed, Found, web_url};

/// How many of a project's other branches a poll asks about, the newest first. What it leaves
/// out is asked about when its page opens.
const NEWEST_BRANCHES: usize = 30;

/// The one connection to the cache, so one that had to fall back to memory stays the same one.
static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

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

fn ask_of(branch: String, commits: impl Iterator<Item = CommitId>) -> BranchAsk {
    BranchAsk {
        branch,
        commits: (commits.take(but_briefs::MOST_COMMITS))
            .map(|id| id.0.to_string())
            .collect(),
    }
}

fn cache_dir() -> Option<PathBuf> {
    but_path::app_cache_dir().ok().map(|dir| dir.join("briefs"))
}

/// `act` on the cache of whoever's signed in. Nothing for no one. The lock is only for `act`, so
/// asking over the network never holds up a read.
fn cached<T>(act: impl FnOnce(&mut Cache) -> rusqlite::Result<T>) -> Result<Option<T>> {
    let Some(account) = users::account_id()? else {
        return Ok(None);
    };
    let account = account.to_string();
    let mut cache = CACHE.lock();
    if cache
        .as_ref()
        .is_none_or(|cache| cache.account() != account)
    {
        *cache = Some(Cache::open(cache_dir().as_deref(), &account)?);
    }
    let Some(cache) = cache.as_mut() else {
        return Ok(None);
    };
    Ok(Some(act(cache)?))
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

fn remote_of(path: &Path) -> Result<String> {
    let ctx = open(path)?;
    forge::remote_url(&ctx.project_meta()?, &*ctx.repo.get()?)
}

/// Seconds since the epoch.
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// What's kept about the branches of the project at `path`.
pub fn known(path: &Path) -> Result<Vec<Anchored>> {
    let remote = remote_of(path)?;
    Ok(cached(|cache| cache.anchors(&remote))?.unwrap_or_default())
}

/// Ask Briefs about `asks`, keep what it says, and give back all that's known. What was asked
/// before a request failed is kept.
fn ask_about(path: &Path, asks: &[BranchAsk]) -> Result<Vec<Anchored>> {
    let remote = remote_of(path)?;
    if let Some(client) = client()? {
        let runtime = runtime().handle();
        for chunk in but_briefs::chunks(asks) {
            let asked = runtime.block_on(client.ask(&remote, chunk, |id, version| {
                cached(|cache| cache.version_branch(id, version))
                    .ok()
                    .flatten()
                    .flatten()
            }))?;
            cached(|cache| cache.took(&remote, &asked, now()))?;
        }
    }
    known(path)
}

/// Ask about the project at `path`: its `applied` branches, by their commits, and the newest of
/// the others, by their tips.
pub fn refresh_project(
    path: &Path,
    applied: Vec<(String, Vec<CommitId>)>,
) -> Result<Vec<Anchored>> {
    let mut others = super::branches::all_branches(path)?;
    others.retain(|branch| !applied.iter().any(|(name, _)| *name == branch.name));
    others.sort_by_key(|branch| std::cmp::Reverse(branch.time));
    let asks: Vec<BranchAsk> = (applied.into_iter())
        .map(|(branch, commits)| ask_of(branch, commits.into_iter()))
        .chain(
            (others.into_iter().take(NEWEST_BRANCHES))
                .map(|branch| ask_of(branch.name, std::iter::once(branch.tip))),
        )
        .collect();
    ask_about(path, &asks)
}

/// Ask about the one branch `anchor`, as a page opens on one that's not known lately.
pub fn refresh_branch(anchor: &BriefAnchor) -> Result<Vec<Anchored>> {
    let ask = ask_of(anchor.branch.clone(), anchor.commits.iter().copied());
    ask_about(&anchor.path, &[ask])
}

/// The version of `found` that matched, ready to draw: as it's kept, else as Briefs says.
pub fn read(found: &Found) -> Result<BriefPage> {
    let (id, version) = (&found.id, found.matched.version);
    if let Some(brief) = cached(|cache| cache.version(id, version))?.flatten() {
        return Ok(page(id, version, brief));
    }
    let Some(client) = client()? else {
        bail!("Sign in to GitButler to read briefs");
    };
    let read = (runtime().handle()).block_on(client.read(id, version))?;
    cached(|cache| cache.put_version(&read, now()))?;
    Ok(page(id, version, serde_json::from_value(read.body)?))
}

/// What `url`, a brief's media file, serves.
pub fn media(url: &str) -> Result<Vec<u8>> {
    let Some(client) = client()? else {
        bail!("Sign in to GitButler to read briefs");
    };
    (runtime().handle()).block_on(client.media(url))
}

/// Leave nothing of what Briefs said on disk, as someone signs out.
pub fn forget() -> Result<()> {
    *CACHE.lock() = None;
    if let Some(dir) = cache_dir().filter(|dir| dir.exists()) {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// A version of a brief, ready to draw.
#[derive(Debug, Clone)]
pub struct BriefPage {
    pub id: String,
    pub version: u32,
    pub title: String,
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

fn page(id: &str, version: u32, brief: Brief) -> BriefPage {
    let published = (brief.created_at.as_deref())
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
        .map(|at| at.timestamp());
    let review = brief.review;
    let blocks = (review.blocks.into_iter())
        .map(|Block { detail, kind }| BriefBlock {
            detail,
            body: body(id, kind),
        })
        .collect();
    BriefPage {
        id: id.to_owned(),
        version,
        title: review.title,
        published,
        changed: review.changed,
        blocks,
    }
}

/// Where `found` was written, against a branch's `commits`, newest first. Worked out as it's
/// drawn, so it stays right as the branch moves without asking again.
pub fn standing(found: &Found, commits: &[CommitId]) -> String {
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
            title: "t".into(),
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
