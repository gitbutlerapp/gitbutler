//! Briefs: what's kept of them, asking about a project's branches to keep it current, and reading
//! one to draw. Asking goes over the network and reading the cache touches the disk, so all of
//! it runs off the UI thread like the rest. `but-briefs` keeps the file, and this says who for.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use but_api::legacy::{forge, users};
use but_briefs::{BranchAsk, Cache, Client};
use parking_lot::Mutex;

use super::{CommitId, open, runtime};

mod page;
mod since;

pub use page::*;

pub use but_briefs::{
    Activity, Anchor, Anchored, Ask, Changed, CommentTo, Concern, Found, Label, NewComment, Note,
    Thread, web_url,
};

/// How much of briefs' pictures stays on disk, the oldest going first.
const MOST_MEDIA_BYTES: u64 = 256 << 20;

/// The most commits since a brief it lists, as more say only that there are many.
const MOST_SINCE: usize = 30;

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
    /// The commit the branch is at, if it has any.
    pub fn tip(&self) -> Option<CommitId> {
        self.commits.first().copied()
    }

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

/// What can be fetched again, as pictures.
fn cache_dir() -> Option<PathBuf> {
    but_path::app_cache_dir().ok().map(|dir| dir.join("briefs"))
}

/// Where the file goes that holds comments not sent yet, which the cache folder is free to lose.
fn data_dir() -> Option<PathBuf> {
    but_path::app_data_dir().ok().map(|dir| dir.join("briefs"))
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
        *cache = Some(Cache::open(data_dir().as_deref(), &account)?);
    }
    let Some(cache) = cache.as_mut() else {
        return Ok(None);
    };
    Ok(Some(act(cache)?))
}

/// How long a client is used before the token is read again, as the talk of a brief that shows is
/// asked for every few seconds and each read goes to the keychain.
const CLIENT_KEPT: Duration = Duration::from_secs(10 * 60);

static CLIENT: Mutex<Option<(Instant, Arc<Client>)>> = Mutex::new(None);

/// A client signed in as whoever's signed in to GitButler, if anyone is. Only this reads the
/// keychain, so nothing that isn't asking Briefs anything does. A profile can outlive its token,
/// as in a debug build, which reads it through `git credential`. That looks the same as having
/// no briefs, so it says so at a level that reaches the log.
fn client() -> Result<Option<Arc<Client>>> {
    if let Some((_, client)) = (CLIENT.lock().as_ref()).filter(|(at, _)| at.elapsed() < CLIENT_KEPT)
    {
        return Ok(Some(client.clone()));
    }
    // Not holding the lock, as the keychain can take a while and the UI thread may want it.
    let client = users::access_token()
        .inspect_err(|err| tracing::warn!("Can't read GitButler's token for Briefs: {err:#}"))?
        .map(Client::new)
        .transpose()?
        .map(Arc::new);
    *CLIENT.lock() = client.clone().map(|client| (Instant::now(), client));
    Ok(client)
}

/// Whoever's signed in changed, so the next ask reads who that is.
pub fn account_changed() {
    *CLIENT.lock() = None;
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

/// `version` of `found` ready to draw, as it's kept else as Briefs says, how it stands against the
/// branch of `anchor` as that is now, and what's different from the version last read.
pub fn read(found: &Found, version: Option<u32>, anchor: &BriefAnchor) -> Result<BriefPage> {
    let id = &found.id;
    // Left to here, what's been read counts, so a newer version read stays what's shown.
    let version = match version {
        Some(version) => version,
        None => {
            let seen = cached(|cache| cache.seen(id))?.flatten().unwrap_or(0);
            let default = version_to_show(found);
            default.max(seen.min(found.latest.max(default)))
        }
    };
    let brief = match cached(|cache| cache.version(id, version))?.flatten() {
        Some(brief) => brief,
        None => {
            let Some(client) = client()? else {
                bail!("Sign in to GitButler to read briefs");
            };
            let read = (runtime().handle()).block_on(client.read(id, version))?;
            cached(|cache| cache.put_version(&read))?;
            serde_json::from_value(read.body)?
        }
    };
    let comparison = brief.review.comparison.as_ref();
    // What Briefs matched it by is the most it knows, for the version it matched.
    let (head, uncommitted) = match found.matched.version == version {
        true => (found.matched.head.clone(), found.matched.uncommitted),
        false => (
            comparison.and_then(|it| it.head.clone()),
            comparison.is_some_and(|it| it.uncommitted),
        ),
    };
    let standing = standing_now(head.as_deref(), anchor);
    let earlier = cached(|cache| {
        let seen = compared_with(id, version, cache.seen(id)?.filter(|seen| *seen < version));
        match seen {
            Some(seen) => Ok(cache.version(id, seen)?.map(|brief| (seen, brief))),
            None => Ok(None),
        }
    })?
    .flatten();
    let mut page = page(
        id,
        version,
        brief,
        standing,
        uncommitted,
        anchor.tip(),
        earlier,
    );
    page.written_at = head;
    Ok(page)
}

/// What `version` of brief `id` compares against: the version last read before it, `seen` if
/// that's still so, else the one it was first compared with since the app started. Reading it
/// moves what's been read on, and going elsewhere and back shouldn't lose what changed.
fn compared_with(id: &str, version: u32, seen: Option<u32>) -> Option<u32> {
    static COMPARED: Mutex<Option<HashMap<(String, u32), u32>>> = Mutex::new(None);
    let mut compared = COMPARED.lock();
    let compared = compared.get_or_insert_default();
    let key = (id.to_owned(), version);
    match seen {
        Some(seen) => {
            compared.insert(key, seen);
            Some(seen)
        }
        None => compared.get(&key).copied(),
    }
}

/// `version` of brief `id` has been shown, so the next version compares against it.
pub fn saw(id: &str, version: u32) -> Result<()> {
    cached(|cache| cache.saw(id, version)).map(|_| ())
}

/// How a brief written at commit `head` stands with the branch as it is, which is all that a
/// branch moving changes about a page.
pub fn standing_now(head: Option<&str>, anchor: &BriefAnchor) -> Standing {
    standing_of(head, anchor).unwrap_or_else(|err| {
        tracing::debug!("couldn't tell how a brief stands: {err:#}");
        Standing::Unknown
    })
}

/// How the commit a brief was written at relates to the branch's tip, by the repository alone.
fn standing_of(head: Option<&str>, anchor: &BriefAnchor) -> Result<Standing> {
    let (Some(head), Some(tip)) = (
        head.and_then(|head| gix::ObjectId::from_hex(head.as_bytes()).ok()),
        anchor.commits.first(),
    ) else {
        return Ok(Standing::Unknown);
    };
    let ctx = open(&anchor.path)?;
    let repo = ctx.repo.get()?;
    if head == tip.0 {
        return Ok(Standing::Current);
    }
    // Commit ids die in a rebase, so one that isn't here is most likely one rewritten away.
    if repo.find_commit(head).is_err() {
        return Ok(Standing::Rewritten);
    }
    if repo
        .merge_base(head, tip.0)
        .ok()
        .flatten()
        .map(|base| base.detach())
        != Some(head)
    {
        return Ok(Standing::Rewritten);
    }
    let commits = (repo.rev_walk([tip.0]).with_hidden([head]).all()?)
        .filter_map(Result::ok)
        .take(MOST_SINCE)
        .filter_map(|info| {
            let commit = repo.find_commit(info.id).ok()?;
            let title = commit.message().ok()?.title.to_string();
            Some(SinceCommit {
                id: CommitId(info.id),
                title: title.trim().to_owned(),
            })
        })
        .collect();
    let files = but_core::diff::tree_changes(&repo, Some(head), tip.0)?
        .into_iter()
        .map(|change| change.path.to_string())
        .collect();
    Ok(Standing::Since { commits, files })
}

/// The version of `found` to show: the one written at this commit if it's that exact, else the
/// newest, as a name matching says nothing about which is right, and whoever opens it wants the
/// latest word.
pub fn version_to_show(found: &Found) -> u32 {
    match found.matched.how {
        but_briefs::How::Head => found.matched.version,
        _ => found.latest.max(found.matched.version),
    }
}

/// What's kept about `version` of brief `id`: what's been said as it was last asked, what's
/// written and not sent, and the last answer given from here.
pub fn kept_talk(id: &str, version: u32) -> Result<Talk> {
    let kept = cached(|cache| {
        Ok(Talk {
            activity: (cache.activity(id, version)?.as_ref())
                .map(Activity::read)
                .unwrap_or_default(),
            notes: cache.notes(id, version)?,
        })
    })?;
    Ok(kept.unwrap_or_default())
}

/// What's kept about a brief's conversation.
#[derive(Debug, Clone, Default)]
pub struct Talk {
    pub activity: Activity,
    pub notes: Vec<Note>,
}

/// What's been said about `version` of brief `id`, as Briefs says now. Asking tells no one.
pub fn ask_talk(id: &str, version: u32) -> Result<Activity> {
    let Some(client) = client()? else {
        bail!("Sign in to GitButler to read briefs");
    };
    let raw = (runtime().handle()).block_on(client.activity(id, version))?;
    cached(|cache| cache.put_activity(id, version, &raw))?;
    Ok(Activity::read(&raw))
}

/// Keep a note, in place of note `replaces` if it's one written again, and give back those there
/// are then. The old one goes last, so what's written can't be lost between the two.
pub fn add_note(
    id: &str,
    version: u32,
    replaces: Option<i64>,
    to: &CommentTo,
    body: &str,
) -> Result<Vec<Note>> {
    let notes = cached(|cache| {
        cache.add_note(id, version, to, body)?;
        if let Some(note) = replaces {
            cache.remove_note(note)?;
        }
        cache.notes(id, version)
    })?;
    Ok(notes.unwrap_or_default())
}

/// Throw a note away, and give back those left.
pub fn remove_note(id: &str, version: u32, note: i64) -> Result<Vec<Note>> {
    let notes = cached(|cache| {
        cache.remove_note(note)?;
        cache.notes(id, version)
    })?;
    Ok(notes.unwrap_or_default())
}

/// Send what's written about `version` of brief `id`, then `answer`, if that's being changed, and
/// give back what's left and what's been said now. What was sent before a failure stays sent and
/// isn't sent again.
pub fn send_talk(id: &str, version: u32) -> Result<Talk> {
    let Some(client) = client()? else {
        bail!("Sign in to GitButler to comment on briefs");
    };
    let runtime = runtime().handle();
    let notes = cached(|cache| cache.notes(id, version))?.unwrap_or_default();
    for note in notes {
        let comment = NewComment {
            body: note.body,
            to: note.to,
        };
        runtime.block_on(client.post_comment(id, version, &comment))?;
        cached(|cache| cache.remove_note(note.id))?;
    }
    // It's sent by now, so not hearing back right away isn't a failure to send.
    if let Err(err) = ask_talk(id, version) {
        tracing::debug!("couldn't ask what's said after sending: {err:#}");
    }
    kept_talk(id, version)
}

/// What `url`, a brief's media file, serves. A media file never changes, as it goes by what's in
/// it, so one read before comes from the disk, at once and offline too.
pub fn media(url: &str) -> Result<Vec<u8>> {
    let kept = kept_media_path(url);
    if let Some(bytes) = (kept.as_ref()).and_then(|path| std::fs::read(path).ok()) {
        return Ok(bytes);
    }
    let Some(client) = client()? else {
        bail!("Sign in to GitButler to read briefs");
    };
    let bytes = (runtime().handle()).block_on(client.media(url))?;
    if let Some(path) = kept {
        keep_media(&path, &bytes);
    }
    Ok(bytes)
}

/// Where the media file at `url` is kept, by what's in it.
fn kept_media_path(url: &str) -> Option<PathBuf> {
    let sha = url.rsplit('/').next()?;
    (!sha.is_empty() && sha.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| cache_dir().map(|dir| dir.join("media").join(sha)))
        .flatten()
}

/// Keep `bytes` at `path`, making room if there's little. It's a cache, so not managing to is
/// only a slower picture next time.
fn keep_media(path: &Path, bytes: &[u8]) {
    let Some(dir) = path.parent() else {
        return;
    };
    std::fs::create_dir_all(dir).ok();
    std::fs::write(path, bytes).ok();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut kept: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            Some((meta.modified().ok()?, meta.len(), entry.path()))
        })
        .collect();
    kept.sort();
    let mut total: u64 = kept.iter().map(|(_, len, _)| len).sum();
    for (_, len, oldest) in kept {
        if total <= MOST_MEDIA_BYTES {
            break;
        }
        std::fs::remove_file(oldest).ok();
        total = total.saturating_sub(len);
    }
}

/// Leave nothing of what Briefs said on disk, as someone signs out.
pub fn forget() -> Result<()> {
    *CACHE.lock() = None;
    for dir in [cache_dir(), data_dir()].into_iter().flatten() {
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
    }
    Ok(())
}
