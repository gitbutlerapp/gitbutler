//! The remotes and the target branch: pushing and pulling branches, updating the workspace
//! from the target, fetching and landing; and the target's history, with what came in that
//! the workspace doesn't have yet, the pull request each commit landed, and when the
//! project last fetched.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use bstr::ByteSlice;
use but_core::DryRun;
use but_core::sync::RepoExclusive;

use super::reviews::{cached_reviews, from_cache};
use super::{
    Agent, CommitId, RemoteCommit, Review, ReviewState, open, present_first_parent, runtime,
    short_ref, title,
};

#[derive(Debug, Clone)]
pub struct TargetCommit {
    pub id: CommitId,
    pub title: String,
    /// The rest of its message, after the title.
    pub body: String,
    /// Seconds since the epoch.
    pub time: i64,
    /// Who it's shown by, as [`super::byline`] picks.
    pub author: String,
    /// Theirs, for their picture.
    pub email: String,
    /// The agent that wrote it, when `author` is the person who committed it.
    pub agent: Option<Agent>,
    /// The pull request it landed, without CI checks.
    pub review: Option<Review>,
    /// The workspace doesn't have it yet.
    pub incoming: bool,
    /// It has more than one parent, bringing in the commits of the others.
    pub merge: bool,
}

/// Up to `limit` of the target's commits, newest first, after `after` or from its tip;
/// and whether there are older ones. From the tip, the first page stops where the workspace
/// forked off.
pub fn target_commits(
    path: &Path,
    after: Option<CommitId>,
    limit: u32,
) -> Result<(Vec<TargetCommit>, bool)> {
    let ctx = open(path)?;
    let cached = cached_reviews(&ctx);
    let (mut commits, mut more) = target_page(&ctx, &cached, after, limit)?;
    // The API reads a thousand at most at a time.
    while after.is_some()
        && more
        && let Some(last) = commits.last().map(|c| c.id)
        && let Some(left) = limit
            .checked_sub(commits.len() as u32)
            .filter(|left| *left > 0)
    {
        let (older, older_more) = target_page(&ctx, &cached, Some(last), left)?;
        if older.is_empty() {
            break;
        }
        more = older_more;
        commits.extend(older);
    }
    Ok((commits, more))
}

/// The newest `limit` of the target's commits the workspace doesn't have yet, each named by
/// the pull request it landed, as the project's page names them, with the pull requests the
/// forge cache has, `cached`.
pub(super) fn incoming(
    ctx: &but_ctx::Context,
    cached: &HashMap<u64, but_forge::ForgeReview>,
    limit: usize,
) -> Result<Vec<RemoteCommit>> {
    let (commits, _) = target_page(ctx, cached, None, limit as u32)?;
    Ok((commits.into_iter())
        .filter(|commit| commit.incoming)
        .map(|commit| RemoteCommit {
            id: commit.id,
            key: String::new(),
            authored: String::new(),
            title: (commit.review.as_ref())
                .map(|review| review.title.clone())
                .filter(|title| !title.is_empty())
                .unwrap_or(commit.title),
            body: commit.body,
            author: commit.author,
            email: commit.email,
            agent: commit.agent,
            time: commit.time,
            merge: commit.merge,
            review: commit.review,
        })
        .collect())
}

fn target_page(
    ctx: &but_ctx::Context,
    cached: &HashMap<u64, but_forge::ForgeReview>,
    after: Option<CommitId>,
    limit: u32,
) -> Result<(Vec<TargetCommit>, bool)> {
    let repo = ctx.repo.get()?;
    let meta = ctx.project_meta()?;
    let source = super::target_source(&repo, &meta);
    let tip = (source.as_ref()).and_then(|name| super::ref_tip(&repo, name.as_ref()));
    // What's checked out has some already when the target was merged into its branch, as
    // the count of new commits leaves them out too.
    let new = (tip.map(|tip| super::new_commits(&repo, &meta, tip))).unwrap_or_default();
    if let Some(tip) = tip.filter(|_| {
        (source.as_ref())
            .is_some_and(|name| name.category() == Some(gix::refs::Category::LocalBranch))
    }) {
        return local_target_page(&repo, tip, &new, after, limit);
    }
    // An upstream that isn't there, before a first push or after the remote deleted it, has
    // no history to list; the project's page says which.
    if source.is_some() && tip.is_none() {
        return Ok((Vec::new(), false));
    }
    drop(repo);
    let page = but_api::target_commits::workspace_target_commits(
        ctx,
        after.map(|id| but_api::json::HexHash(id.0)),
        Some(limit),
    )?;
    let repo = ctx.repo.get()?;
    let mailmap = repo.open_mailmap();
    let commits = (page.commits.into_iter())
        .map(|target| {
            let review = target.review.map(|review| {
                let mut found = match cached.get(&(review.number as u64)) {
                    Some(found) => from_cache(found),
                    // Named by the merge commit's message, which the cache has no record of.
                    None => Review {
                        number: review.number as u64,
                        title: review.title,
                        url: Some(review.html_url),
                        ..Review::default()
                    },
                };
                // It landed, whatever the cache saw last.
                found.state = ReviewState::Merged;
                found
            });
            Ok(TargetCommit {
                review,
                incoming: !target.in_workspace && new.contains(&target.commit.id),
                ..target_commit(&mailmap, &repo.find_commit(target.commit.id)?)?
            })
        })
        .collect::<Result<Vec<TargetCommit>>>()?;
    // The first page stops where the workspace forked off, with older commits below that
    // unless it's where the history starts.
    let forked_above_root = after.is_none()
        && (commits.last()).is_some_and(|last| {
            (repo.find_commit(last.id.0)).is_ok_and(|c| present_first_parent(&c).is_some())
        });
    Ok((commits, page.has_more || forked_above_root))
}

/// [`target_page`] for a target with no upstream, which the API lists nothing of: the local
/// branch's own history from `tip`, along first parents, with those among `new` incoming. The
/// first page stops where the workspace is.
fn local_target_page(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    new: &HashSet<gix::ObjectId>,
    after: Option<CommitId>,
    limit: u32,
) -> Result<(Vec<TargetCommit>, bool)> {
    let mut cursor = match after {
        Some(after) => present_first_parent(&repo.find_commit(after.0)?),
        None => Some(tip),
    };
    let mut incoming = true;
    let mut commits = Vec::new();
    let mailmap = repo.open_mailmap();
    while commits.len() < limit as usize
        && let Some(id) = cursor
    {
        // Once one is in the workspace's history, so is every commit below it.
        incoming = incoming && new.contains(&id);
        let commit = repo.find_commit(id)?;
        cursor = present_first_parent(&commit);
        commits.push(TargetCommit {
            incoming,
            ..target_commit(&mailmap, &commit)?
        });
        if after.is_none() && !incoming {
            break;
        }
    }
    Ok((commits, cursor.is_some()))
}

/// `commit` as a target's history lists it. Whether it's incoming, and the pull request it
/// landed, are for the caller to set.
pub(super) fn target_commit(
    mailmap: &gix::mailmap::Snapshot,
    commit: &gix::Commit<'_>,
) -> Result<TargetCommit> {
    let (title, body) = super::subject_and_body(&commit.message_raw_sloppy().to_str_lossy());
    let ((author, email), agent) = super::commit_byline(mailmap, commit)?;
    Ok(TargetCommit {
        id: CommitId(commit.id),
        title,
        body,
        time: commit.time()?.seconds,
        author,
        email,
        agent,
        review: None,
        incoming: false,
        merge: commit.parent_ids().count() > 1,
    })
}

/// How many days back the project's page counts the target's activity.
pub const ACTIVITY_DAYS: usize = 30;

const DAY: i64 = 24 * 60 * 60;

/// The day `seconds` (since the epoch) falls on here, numbered from the epoch's, so a day
/// picked stays that day as the clock moves on.
pub fn day_of(seconds: i64) -> i64 {
    (seconds + offset_at(seconds)).div_euclid(DAY)
}

/// When day `day`, as [`day_of`] numbers it, starts here, in seconds since the epoch.
pub fn day_start(day: i64) -> i64 {
    // Midnight in UTC, moved by the offset here at about midnight here. Read at midnight in
    // UTC, the offset would be an hour out when the clocks change between the two.
    let midnight = day * DAY;
    midnight - offset_at(midnight - offset_at(midnight))
}

/// How far the clock here is ahead of UTC at `seconds` (since the epoch), in seconds: the
/// summer offset for a summer moment, whatever the season now.
fn offset_at(seconds: i64) -> i64 {
    use chrono::TimeZone as _;
    (chrono::Local.timestamp_opt(seconds, 0).single())
        .map_or(0, |at| at.offset().local_minus_utc().into())
}

/// The target as the project's page sums it up: what landed on it, and who's been committing
/// to it.
#[derive(Debug, Clone, Default)]
pub struct Overview {
    /// How many commits landed on the target's own line, as its history lists them, each of
    /// the last [`ACTIVITY_DAYS`] days here, oldest first.
    pub daily: Vec<u32>,
    /// Whose commits landed on it in those days, merges left out, the most landings first.
    pub people: Vec<Person>,
    /// The commits of its own line in those days, newest first.
    pub landings: Vec<Landing>,
    /// The tags on its own line in those days, newest first.
    pub releases: Vec<Release>,
    /// The day the first of `daily` counts, as [`day_of`] numbers days.
    pub first_day: i64,
}

/// A commit of the target's own line: one of its own, or a merge bringing others in.
#[derive(Debug, Clone, PartialEq)]
pub struct Landing {
    pub id: CommitId,
    /// What it's called: its pull request's title when it merged one, else its own.
    pub title: String,
    /// The day it landed, as [`day_of`] numbers days.
    pub day: i64,
    /// It landed a pull request: a merge, or a commit whose message names the one it merged.
    pub pull: bool,
    /// How many commits it brought in, merges left out.
    pub commits: u32,
    /// For a merge, seconds from the first commit it brought being written to it landing. A
    /// squashed pull request's commit was written as it landed, so it has none.
    pub took: Option<i64>,
    /// Who wrote the newest commit it brought in, by email as [`Person`]s are keyed: a pull
    /// request's author, most likely.
    pub by: Option<String>,
    /// The parts of the repository it changed, as [`areas`] names them, each once.
    pub areas: Vec<String>,
}

/// A tag on the target's own line.
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub name: String,
    pub id: CommitId,
    pub day: i64,
}

/// Someone whose commits landed on the target lately, by [`super::byline`].
#[derive(Debug, Clone, PartialEq)]
pub struct Person {
    pub name: String,
    pub email: String,
    /// How many times theirs landed each day, as [`Overview::daily`] counts them: a merge of
    /// many of their commits counts once, as hundreds landing in one pull request isn't
    /// hundreds of changes to the target.
    pub daily: Vec<u32>,
    /// The commits of the target's own line that landed theirs: one of theirs, or a merge
    /// bringing theirs in.
    pub landed: HashSet<CommitId>,
}

/// The target summed up as of `now`, in seconds since the epoch, or nothing without one. A
/// commit that can't be read is left out rather than failing the rest.
pub fn overview(path: &Path, now: i64) -> Result<Option<Overview>> {
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    // By its full name, as a tag or a branch can have its short one.
    let meta = ctx.project_meta()?;
    let tip =
        super::target_source(&repo, &meta).and_then(|name| super::ref_tip(&repo, name.as_ref()));
    let Some(tip) = tip else {
        return Ok(None);
    };
    let tip = repo.find_commit(tip)?;
    let first_day = day_of(now) - (ACTIVITY_DAYS as i64 - 1);
    let since = day_start(first_day);
    let mut daily = vec![0; ACTIVITY_DAYS];
    let mut people: Vec<Person> = Vec::new();
    let mut landings = Vec::new();
    // People go by the names and emails `.mailmap` gives them, as one with a work and a
    // personal email is one person.
    let mailmap = repo.open_mailmap();
    // A walk of first parents only ignores a time cutoff, so it stops at the first commit
    // from before the window itself rather than reading the whole history.
    let line: Vec<_> = (repo.rev_walk([tip.id]).first_parent_only().all()?)
        .filter_map(|info| info.ok()?.object().ok())
        .take_while(|commit| commit.time().is_ok_and(|time| time.seconds >= since))
        .collect();
    let mut brought = brought_in(&repo, tip.id, &line)?;
    for landed in &line {
        let day = landed.time().map_or(first_day, |time| day_of(time.seconds));
        // A clock ahead of this one counts as today.
        let day = ((day - first_day) as usize).min(ACTIVITY_DAYS - 1);
        if let Some(count) = daily.get_mut(day) {
            *count += 1;
        }
        // Theirs land on the day it did: the commit itself, or what a merge brought in.
        let first = present_first_parent(landed);
        let brought = match first.is_some() && landed.parent_ids().count() > 1 {
            true => brought.remove(&landed.id).unwrap_or_default(),
            false => vec![landed.clone()],
        };
        for commit in brought.iter().filter(|c| c.parent_ids().count() < 2) {
            let Ok(((name, email), _)) = super::commit_byline(&mailmap, commit) else {
                continue;
            };
            let email = email.to_lowercase();
            let at = match people.iter().position(|person| person.email == email) {
                Some(at) => at,
                None => {
                    let daily = vec![0; ACTIVITY_DAYS];
                    let landed = HashSet::new();
                    people.push(Person {
                        name,
                        email,
                        daily,
                        landed,
                    });
                    people.len() - 1
                }
            };
            let Some(person) = people.get_mut(at) else {
                continue;
            };
            if person.landed.insert(CommitId(landed.id))
                && let Some(count) = person.daily.get_mut(day)
            {
                *count += 1;
            }
        }
        landings.push(landing(
            &repo,
            &mailmap,
            landed,
            first,
            &brought,
            first_day + day as i64,
        ));
    }
    let releases = releases(&repo, &landings);
    people.sort_by_key(|person| std::cmp::Reverse(person.daily.iter().sum::<u32>()));
    Ok(Some(Overview {
        daily,
        people,
        landings,
        releases,
        first_day,
    }))
}

/// What each merge of `line`, the target's own commits from `tip` back, newest first, brought
/// in: what its other parents reach that its first doesn't, the tip of its branch first.
///
/// One walk reads everything the line took in. A merge's first parent reaches all that the
/// merges before it brought, so going oldest first, each merge takes what it reaches that no
/// older one took, which is the same answer as one walk for each merge.
fn brought_in<'repo>(
    repo: &'repo gix::Repository,
    tip: gix::ObjectId,
    line: &[gix::Commit<'repo>],
) -> Result<HashMap<gix::ObjectId, Vec<gix::Commit<'repo>>>> {
    let Some(oldest) = line.last() else {
        return Ok(HashMap::new());
    };
    let before = present_first_parent(oldest);
    let window: HashMap<gix::ObjectId, gix::Commit<'repo>> = (repo.rev_walk([tip]))
        .with_hidden(before)
        .all()?
        .filter_map(|info| info.ok()?.object().ok())
        .map(|commit| (commit.id, commit))
        .collect();
    let mut taken: HashSet<gix::ObjectId> = line.iter().map(|c| c.id).collect();
    let mut brought = HashMap::new();
    for landed in line.iter().rev() {
        let mut next: Vec<gix::ObjectId> = (landed.parent_ids().skip(1))
            .map(|id| id.detach())
            .collect();
        let mut theirs = Vec::new();
        while let Some(id) = next.pop() {
            let Some(commit) = window.get(&id).filter(|_| taken.insert(id)) else {
                continue;
            };
            // Its first parent next, to follow its own branch first.
            let parents: Vec<_> = commit.parent_ids().map(|id| id.detach()).collect();
            next.extend(parents.into_iter().rev());
            theirs.push(commit.clone());
        }
        brought.insert(landed.id, theirs);
    }
    Ok(brought)
}

/// The parts of the repository where trees `old` and `new` differ, sorted: the first two
/// folders of a file that changed, as `crates/but`, or its folder when it's only one deep, or
/// the top level for a file there. So it reads only two folders deep.
fn areas(
    repo: &gix::Repository,
    old: Option<gix::ObjectId>,
    new: Option<gix::ObjectId>,
) -> Vec<String> {
    let mut areas = BTreeSet::new();
    for (name, file, [old, new]) in differing(repo, old, new) {
        if file {
            areas.insert("Top level".to_owned());
        }
        for (inner, file, folders) in differing(repo, old, new) {
            if file {
                areas.insert(name.to_str_lossy().into_owned());
            }
            if folders.iter().any(Option::is_some) {
                areas.insert(format!("{}/{}", name.to_str_lossy(), inner.to_str_lossy()));
            }
        }
    }
    areas.into_iter().collect()
}

/// The entries where trees `old` and `new` differ, by name: whether a file is among what
/// differs there, and the folder each side has there.
fn differing(
    repo: &gix::Repository,
    old: Option<gix::ObjectId>,
    new: Option<gix::ObjectId>,
) -> Vec<(bstr::BString, bool, [Option<gix::ObjectId>; 2])> {
    let entries = |tree: Option<gix::ObjectId>| -> Option<HashMap<bstr::BString, _>> {
        let Some(tree) = tree else {
            return Some(HashMap::new());
        };
        let tree = repo.find_tree(tree).ok()?;
        Some(
            (tree.iter())
                .filter_map(|entry| {
                    let entry = entry.ok()?;
                    Some((
                        entry.filename().to_owned(),
                        (entry.mode(), entry.object_id()),
                    ))
                })
                .collect(),
        )
    };
    // A tree this clone doesn't have, as a treeless one, says nothing of what differs.
    let (Some(old), Some(new)) = (entries(old), entries(new)) else {
        return Vec::new();
    };
    let names: BTreeSet<_> = old.keys().chain(new.keys()).collect();
    (names.into_iter())
        .filter_map(|name| {
            let sides = [old.get(name), new.get(name)];
            (sides[0] != sides[1]).then(|| {
                let file = sides.iter().flatten().any(|(mode, _)| !mode.is_tree());
                let folders =
                    sides.map(|side| side.filter(|(mode, _)| mode.is_tree()).map(|(_, id)| *id));
                (name.clone(), file, folders)
            })
        })
        .collect()
}

/// `landed` summed up as it landed on `day`: from its `first` parent, `brought` in.
fn landing(
    repo: &gix::Repository,
    mailmap: &gix::mailmap::Snapshot,
    landed: &gix::Commit<'_>,
    first: Option<gix::ObjectId>,
    brought: &[gix::Commit<'_>],
    day: i64,
) -> Landing {
    let message = (landed.message_raw().ok()).map_or(String::new(), |m| m.to_str_lossy().into());
    let merge = landed.parent_ids().count() > 1;
    let review = but_forge::merged_review_from_message(&but_forge::ForgeName::GitHub, &message);
    let own = brought.iter().filter(|c| c.parent_ids().count() < 2);
    let written = (own.clone())
        .filter_map(|c| Some(c.author().ok()?.time().ok()?.seconds))
        .min();
    let took = (landed.time().ok())
        .zip(written)
        .filter(|_| merge)
        .map(|(t, w)| t.seconds - w);
    let by = (own.clone().next())
        .and_then(|c| super::commit_byline(mailmap, c).ok())
        .map(|((_, email), _)| email.to_lowercase());
    // What it changed from where the line was, which for a merge is all it brought; nothing
    // for a commit whose parent this clone doesn't have, as a shallow one's oldest, which
    // didn't add everything it has.
    let tree = |id: gix::ObjectId| Some(repo.find_commit(id).ok()?.tree_id().ok()?.detach());
    let areas = match first.is_none() && landed.parent_ids().next().is_some() {
        true => Vec::new(),
        false => areas(
            repo,
            first.and_then(tree),
            landed.tree_id().ok().map(|id| id.detach()),
        ),
    };
    Landing {
        id: CommitId(landed.id),
        title: (review.as_ref())
            .map(|r| r.title)
            .filter(|t| !t.is_empty())
            .map_or_else(|| title(message.as_bytes()), str::to_owned),
        day,
        pull: merge || review.is_some(),
        commits: own.count() as u32,
        took,
        by,
        areas,
    }
}

/// The tags on the commits of `landings`, newest first.
fn releases(repo: &gix::Repository, landings: &[Landing]) -> Vec<Release> {
    let at: HashMap<CommitId, (usize, i64)> = (landings.iter().enumerate())
        .map(|(ix, l)| (l.id, (ix, l.day)))
        .collect();
    let Ok(references) = repo.references() else {
        return Vec::new();
    };
    let Ok(tags) = references.tags() else {
        return Vec::new();
    };
    let mut releases: Vec<(usize, Release)> = tags
        .filter_map(|tag| {
            let mut tag = tag.ok()?;
            let id = CommitId(tag.peel_to_id().ok()?.detach());
            let &(ix, day) = at.get(&id)?;
            let name = tag.name().shorten().to_string();
            Some((ix, Release { name, id, day }))
        })
        .collect();
    releases.sort_by(|(a, x), (b, y)| a.cmp(b).then_with(|| x.name.cmp(&y.name)));
    releases.into_iter().map(|(_, release)| release).collect()
}

/// The commits `merge` brought in: those its other parents have that its first doesn't,
/// newest first, up to `limit`; and whether there are more. A shallow clone may not have
/// them.
pub fn merged_commits(
    path: &Path,
    merge: CommitId,
    limit: usize,
) -> Result<(Vec<TargetCommit>, bool)> {
    use gix::revision::walk::Sorting;
    use gix::traverse::commit::simple::CommitTimeOrder;
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let commit = repo.find_commit(merge.0)?;
    let mut parents = commit.parent_ids().map(|id| id.detach());
    let Some(first) = parents.next() else {
        return Ok((Vec::new(), false));
    };
    let others: Vec<_> = parents.filter(|id| repo.has_object(id)).collect();
    anyhow::ensure!(
        !others.is_empty(),
        "this clone doesn't have the commits it merged"
    );
    let mailmap = repo.open_mailmap();
    let walk = (repo.rev_walk(others))
        .with_hidden([first])
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
        .all()?;
    let mut commits = (walk.take(limit + 1))
        .map(|info| target_commit(&mailmap, &info?.object()?))
        .collect::<Result<Vec<_>>>()?;
    let more = commits.len() > limit;
    commits.truncate(limit);
    Ok((commits, more))
}

/// When the project last fetched, in seconds since the epoch, and why the last attempt
/// failed if it did. `None` when it has no remote to fetch from.
pub fn fetch_status(path: &Path) -> Result<Option<(Option<i64>, Option<String>)>> {
    let ctx = open(path)?;
    if ctx.repo.get()?.remote_names().is_empty() {
        return Ok(None);
    }
    let status = but_api::workspace::workspace_fetch_status(&ctx)?;
    // A fetch in a terminal counts too: Git notes every fetch in FETCH_HEAD, one for each
    // worktree, as an agent fetching in its own leaves one there. One that failed leaves it
    // empty, which doesn't count.
    let fetch_head = super::last_written(ctx.repo.get()?.common_dir(), &["FETCH_HEAD"]);
    let fetched = status.last_successful_ms.map(|ms| (ms / 1000) as i64);
    // Not a failure: an error from before a later fetch elsewhere, or from a remote the
    // workspace doesn't need, which leaves the fetch counted a success.
    let attempted = status.last_attempted_ms.map(|ms| (ms / 1000) as i64);
    let succeeded = fetched.is_some() && fetched == attempted;
    let error = status
        .last_error
        .filter(|_| !succeeded && fetch_head <= attempted);
    Ok(Some((fetched.max(fetch_head), error)))
}

/// Push `ref_name` and the branches below it, forced when one of them was rewritten since it
/// was pushed, and with `run_hooks` after the repository's pre-push hook passes. Returns the
/// name it has on its remote, which needn't be its local one, and why updating the stack's
/// pull requests afterwards failed, if it did.
pub fn push(
    path: &Path,
    ref_name: &super::FullName,
    protect: bool,
    drops: Option<&[CommitId]>,
    run_hooks: bool,
) -> Result<(Option<String>, Option<String>)> {
    let branch = ref_name.as_ref();
    let mut ctx = open(path)?;
    // Decided when it runs, as an amend queued before it rewrites the branch.
    let mut info = but_api::legacy::workspace::head_info(&ctx)?;
    // Forced, as decided when it runs, the push goes past the lease, which would refuse to
    // drop even the commits only the remote has that were shown. Unless pushing anyway, it
    // fetches again first and stops if the remote moved, as a teammate's amend still matches
    // the commit rewritten here, or has commits that weren't shown.
    if let Some(shown) = drops.filter(|_| needs_force(&info, branch)) {
        let tips = remote_tips(&ctx, &info, branch)?;
        fetch(path)?;
        // Opened again after the fetch, so the remote's tips are read as it left them.
        ctx = open(path)?;
        info = but_api::legacy::workspace::head_info(&ctx)?;
        let now = remote_tips(&ctx, &info, branch)?;
        // A lane pushes the branches below it too: the refusal names the one it's about.
        let name = |segment: Option<&but_workspace::ref_info::Segment>| {
            (segment.and_then(|segment| segment.ref_name())).map_or_else(
                || short_ref(ref_name).into_owned(),
                |n| n.shorten().to_string(),
            )
        };
        if now != tips {
            let moved = name(
                (pushed_with(&info, branch).zip(tips.iter().zip(&now)))
                    .find(|(_, (before, after))| before != after)
                    .map(|(segment, _)| segment),
            );
            anyhow::bail!(
                "Someone pushed to {moved} since the last fetch, and forcing would replace \
                 it. Check what's there before pushing again."
            );
        }
        if let Some(unseen) = pushed_with(&info, branch).find(|segment| {
            (segment.commits_on_remote.iter()).any(|c| !shown.contains(&CommitId(c.id)))
        }) {
            anyhow::bail!(
                "The remote has commits on {} that weren't shown, and forcing would drop \
                 them. Push again to see them.",
                name(Some(unseen))
            );
        }
    }
    // As in the desktop app, a conflicted commit stays here until it's resolved.
    if pushed_with(&info, branch).any(|segment| segment.commits.iter().any(|c| c.has_conflicts)) {
        anyhow::bail!("Resolve the conflicted commits first, so they aren't pushed");
    }
    let force = needs_force(&info, branch);
    let outcome = runtime()
        .handle()
        .block_on(
            but_api::legacy::workspace::workspace_branch_and_ancestors_push(
                ctx.into_sync(),
                force,
                !protect,
                ref_name.to_string(),
                run_hooks,
                Vec::new(),
            ),
        )
        .map_err(|err| {
            let root = err.root_cause().to_string();
            if let Some(output) = root.strip_prefix("pre-push hook failed: ") {
                let output = super::ops::hook_output(output);
                return anyhow::anyhow!("The pre-push hook refused the push:\n{output}");
            }
            if err.downcast_ref::<but_error::Code>() != Some(&but_error::Code::GitNonFastForward) {
                return err;
            }
            tracing::warn!(?err, "push rejected");
            anyhow::anyhow!(match force {
                // Forced, it's the remote that won't rewrite it.
                true => "The remote refused the force push, as it does for a protected branch.",
                false => "The remote has commits this branch doesn't. Fetch, then pull them in.",
            })
        })?;
    let (short, full) = (short_ref(ref_name), ref_name.to_string());
    let remote = (outcome.push.branch_to_remote.into_iter())
        .find(|(branch, _, _)| *branch == short || *branch == full)
        .map(|(_, _, remote)| remote);
    Ok((remote, sync_failure(outcome.review_sync)))
}

/// Why updating the stack's pull requests failed, if it did.
pub(super) fn sync_failure(outcome: but_forge::ReviewSyncOutcome) -> Option<String> {
    match outcome {
        but_forge::ReviewSyncOutcome::Failed { message } => {
            tracing::warn!(%message, "the stack's pull requests weren't updated");
            Some(message)
        }
        _ => None,
    }
}

/// Whether pushing `branch` has to force: a segment [`pushed_with()`] yields was rewritten since
/// it was pushed.
fn needs_force(info: &but_workspace::RefInfo, branch: &gix::refs::FullNameRef) -> bool {
    use but_core::ui::PushStatus;
    pushed_with(info, branch)
        .any(|segment| segment.push_status == PushStatus::UnpushedCommitsRequiringForce)
}

/// Where the remote-tracking branches of what pushing `branch` pushes point.
fn remote_tips(
    ctx: &but_ctx::Context,
    info: &but_workspace::RefInfo,
    branch: &gix::refs::FullNameRef,
) -> Result<Vec<Option<gix::ObjectId>>> {
    let repo = ctx.repo.get()?;
    Ok(pushed_with(info, branch)
        .map(|segment| {
            (segment.remote_tracking_ref_name.as_ref())
                .and_then(|name| super::ref_tip(&repo, name.as_ref()))
        })
        .collect())
}

/// `branch`'s segment and the ones below it, down the lanes it rests on, which pushing it
/// pushes too.
fn pushed_with<'a>(
    info: &'a but_workspace::RefInfo,
    branch: &gix::refs::FullNameRef,
) -> impl Iterator<Item = &'a but_workspace::ref_info::Segment> {
    but_workspace::legacy::push::branch_and_ancestor_segments(info, branch).into_values()
}

/// When `err` is force push protection stopping a push, what the remote itself said, if
/// anything: Git counts any refused force push as that, a protected branch's too.
pub fn push_refusal(err: &anyhow::Error) -> Option<String> {
    if err.downcast_ref::<but_error::Code>() != Some(&but_error::Code::GitForcePushProtection) {
        return None;
    }
    tracing::warn!(?err, "push refused");
    let text = format!("{err:#}");
    let said: Vec<&str> = (text.lines().map(str::trim))
        .filter_map(|line| line.strip_prefix("remote:").map(str::trim))
        .filter(|line| !line.is_empty())
        .collect();
    Some(said.join("\n"))
}

/// Bring in the commits only `branch`'s remote has: the local commits rebased on top of
/// them, leaving out the remote's older versions of commits rewritten here; or with `theirs`
/// the remote's version of the branch instead of the local one.
pub fn pull_remote(path: &Path, branch: &super::FullName, theirs: bool) -> Result<()> {
    integrate_remote(&mut open(path)?, branch, theirs, DryRun::No)?;
    Ok(())
}

/// A dry run of pulling into `branch`: the branches (full refs) it would leave with
/// conflicted commits they don't have now, each with how many.
pub fn pull_conflicts(
    path: &Path,
    branch: &super::FullName,
) -> Result<Vec<(Option<super::FullName>, usize)>> {
    let mut ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let (preview, incoming) = integrate_remote(&mut ctx, branch, false, DryRun::Yes)?;
    let conflicted = &preview.workspace.conflicted_commits;
    let mut found = newly_conflicted(&info, conflicted);
    // The remote's own commits can land conflicted too, rebased onto the branch's, and they
    // aren't the workspace's yet.
    let incoming = (conflicted.iter())
        .filter(|(_, change)| incoming.contains(&change.to_string()))
        .count();
    if incoming > 0 {
        let name = Some(branch.clone());
        match found.iter_mut().find(|(ref_name, _)| *ref_name == name) {
            Some((_, count)) => *count += incoming,
            None => found.push((name, incoming)),
        }
    }
    Ok(found)
}

fn integrate_remote(
    ctx: &mut but_ctx::Context,
    branch: &super::FullName,
    theirs: bool,
    dry_run: DryRun,
) -> Result<(but_api::branch::IntegrateBranchResult, Vec<String>)> {
    use but_api::branch::json::BranchIntegrationStrategy as Strategy;
    let name = branch;
    let strategy = if theirs {
        Strategy::PickRemote
    } else {
        Strategy::PullRebase
    };
    let mut plan =
        but_api::branch::get_initial_branch_integration(ctx, name.as_ref(), Some(strategy))?;
    // Rebasing would bring back the remote's versions of commits rewritten here, beside
    // their new ones, so they're left out, as Lite's "Combine both" does. A version is the
    // same change ID on both sides, one made without counting as the one the engine gives it
    // from its sha when it first rebases it; or with neither having one, as after plain Git,
    // the same subject by the same author. The remote's new commits were made on those versions, so they go
    // after the newest of them as it is here, keeping the remote's order, rather than under
    // every commit of the branch, and of the branches it's stacked on.
    // The change IDs of the remote's commits a rebase brings in.
    let mut incoming = Vec::new();
    if !theirs {
        use but_workspace::branch::IntegrationDivergenceCommit as Version;
        use but_workspace::branch::integrate_branch_upstream::InteractiveIntegrationStep as Step;
        let key = |c: &Version| {
            (c.change_id.clone()).unwrap_or_else(|| {
                but_core::commit::Headers::synthetic_change_id_from_commit_id(c.id).to_string()
            })
        };
        let same = |a: &Version, b: &Version| {
            key(a) == key(b)
                || (a.change_id.is_none()
                    && b.change_id.is_none()
                    && a.subject == b.subject
                    && a.author == b.author)
        };
        let divergence = &plan.divergence;
        let (remote, local) = (&divergence.upstream_only, &divergence.local_only);
        let versions = |of: &[Version], among: &[Version]| -> Vec<gix::ObjectId> {
            (of.iter())
                .filter(|c| among.iter().any(|other| same(c, other)))
                .map(|c| c.id)
                .collect()
        };
        let (older, rewritten) = (versions(remote, local), versions(local, remote));
        let upstream: Vec<gix::ObjectId> = remote.iter().map(|c| c.id).collect();
        incoming = (remote.iter())
            .filter(|c| !older.contains(&c.id))
            .map(key)
            .collect();
        let picked = |step: &Step, among: &[gix::ObjectId]| matches!(step, Step::Pick { commit_id } if among.contains(commit_id));
        let steps = std::mem::take(&mut plan.integration.steps);
        let (incoming, mut ours): (Vec<_>, Vec<_>) = (steps.into_iter())
            .filter(|step| !picked(step, &older))
            .partition(|step| picked(step, &upstream));
        let at = (ours.iter())
            .rposition(|step| picked(step, &rewritten))
            .map_or(0, |at| at + 1);
        ours.splice(at..at, incoming);
        plan.integration.steps = ours;
    }
    let mut guard = ctx.exclusive_worktree_access();
    let result = but_api::branch::apply_branch_integration_with_perm(
        ctx,
        name.as_ref(),
        plan.integration,
        dry_run,
        guard.write_permission(),
    )?;
    Ok((result, incoming))
}

/// What [`update_from_target()`] did.
#[derive(Debug)]
pub enum UpdateOutcome {
    /// It would have left these conflicted that aren't now, so nothing changed.
    WouldConflict(UpdateConflicts),
    Updated {
        /// Branches (full refs) left with conflicted commits they didn't have, each with
        /// how many.
        conflicted: Vec<(Option<super::FullName>, usize)>,
        /// Branches (full refs) that landed on the target, which the update removed.
        landed: Vec<super::FullName>,
    },
}

/// Rebase every stack onto the target as it is now. Doesn't fetch first; see [`fetch()`].
/// A dry run goes first, and when the rebase would leave anything conflicted that isn't now,
/// it stops there, unless that's what was `accepted`: a fetch since can bring more.
pub fn update_from_target(
    path: &Path,
    accepted: Option<&UpdateConflicts>,
) -> Result<UpdateOutcome> {
    let mut ctx = open(path)?;
    // Held from reading the workspace through the update, so nothing changes it in between.
    let mut guard = ctx.exclusive_worktree_access();
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let preview = integrate_upstream(&mut ctx, &info, DryRun::Yes, guard.write_permission())?;
    let found = UpdateConflicts {
        branches: newly_conflicted(&info, &preview.workspace_state.conflicted_commits),
        files: (preview.worktree_conflicts.iter())
            .map(|path| path.to_str_lossy().into_owned())
            .collect(),
    };
    if (!found.branches.is_empty() || !found.files.is_empty()) && accepted != Some(&found) {
        return Ok(UpdateOutcome::WouldConflict(found));
    }
    let updated = integrate_upstream(&mut ctx, &info, DryRun::No, guard.write_permission())?;
    let after = but_api::legacy::workspace::head_info(&ctx)?;
    let names = |info: &but_workspace::RefInfo| -> Vec<super::FullName> {
        (info.lanes())
            .flat_map(|lane| lane.segments)
            .filter_map(|segment| Some(segment.ref_name()?.to_owned()))
            .collect()
    };
    let remaining = names(&after);
    Ok(UpdateOutcome::Updated {
        conflicted: newly_conflicted(&info, &updated.workspace_state.conflicted_commits),
        landed: (names(&info).into_iter())
            .filter(|name| !remaining.contains(name))
            .collect(),
    })
}

/// What [`update_from_target()`] would leave conflicted that isn't now.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateConflicts {
    /// Branches (full refs), each with how many of its commits.
    pub branches: Vec<(Option<super::FullName>, usize)>,
    /// Uncommitted files the target changed too, which get conflict markers.
    pub files: Vec<String>,
}

/// The branches (full refs, `None` for an unnamed one) in `info` with commits among
/// `conflicted` that aren't conflicted yet, each with how many.
pub(super) fn newly_conflicted(
    info: &but_workspace::RefInfo,
    conflicted: &[(gix::ObjectId, but_core::ChangeId)],
) -> Vec<(Option<super::FullName>, usize)> {
    // A commit made without a change ID gets one from its commit ID, as the rebase does.
    let newly = |commit: &&but_workspace::ref_info::LocalCommit| {
        let id = commit.inner.change_id();
        !commit.inner.has_conflicts && conflicted.iter().any(|(_, change)| *change == *id)
    };
    (info.lanes())
        .flat_map(|lane| lane.segments)
        .filter_map(|segment| {
            let count = segment.commits.iter().filter(newly).count();
            let name = segment.ref_name().map(ToOwned::to_owned);
            (count > 0).then_some((name, count))
        })
        .collect()
}

fn integrate_upstream(
    ctx: &mut but_ctx::Context,
    info: &but_workspace::RefInfo,
    dry_run: DryRun,
    perm: &mut RepoExclusive,
) -> Result<but_api::workspace::WorkspaceIntegrateUpstreamOutcome> {
    let updates = but_api::workspace::rebase_lane_bottoms(info.stacks.iter().map(|s| s.lane()));
    but_api::workspace::workspace_integrate_upstream_with_perm(ctx, updates, dry_run, perm)
}

/// Fetch from the project's remotes. `false` when it has none to fetch from.
pub fn fetch(path: &Path) -> Result<bool> {
    let mut ctx = open(path)?;
    if ctx.repo.get()?.remote_names().is_empty() {
        return Ok(false);
    }
    but_api::workspace::workspace_fetch_from_remotes(&mut ctx, Some("fetch".into()))?;
    Ok(true)
}

/// Put `branch`, or with `whole_stack` it and every branch below it,
/// straight onto the target and push the target, or with no upstream, just move it. Returns
/// what that left for the other branches.
pub fn land(
    path: &Path,
    branch: &super::FullName,
    tip: CommitId,
    whole_stack: bool,
) -> Result<Landed> {
    let mut ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    // A merge commit when the branch has diverged, as `but land` makes one without its flags.
    // Only at `tip`, the one the user was asked about: a push can't be undone, so not of
    // commits an agent added since.
    let landed = but_api::land::branch_land(
        &mut ctx,
        branch.shorten().to_string(),
        false,
        false,
        whole_stack,
        Some(tip.0),
    )?;
    Ok(Landed {
        update_needed: landed.reconcile_skipped,
        conflicted: newly_conflicted(&info, &landed.workspace.conflicted_commits),
        marked: landed.workspace.checkout_conflict_occurred,
    })
}

/// What [`land()`] left for the other branches.
pub struct Landed {
    /// They still need an update to sit on the moved target.
    pub update_needed: bool,
    /// Branches (full refs) the update after it left with conflicted commits they didn't
    /// have, each with how many.
    pub conflicted: Vec<(Option<super::FullName>, usize)>,
    /// The update gave uncommitted changes conflict markers.
    pub marked: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        ACTIVITY_DAYS, DAY, UpdateOutcome, day_of, overview, push_refusal, pushed_with,
        update_from_target,
    };
    use crate::backend::model::build::full;

    #[test]
    fn an_update_that_would_conflict_a_worktree_branch_names_it() {
        use crate::backend::ops::tests::git;
        crate::backend::lane_tests::linked_workspace(|path| {
            let workspace = crate::backend::load_workspace(path, Default::default()).unwrap();
            let wt = &(workspace.worktrees.iter())
                .find(|wt| wt.name == "wt")
                .unwrap()
                .path;
            git(wt, &["checkout", "-q", "--detach", "main"]);
            std::fs::write(wt.join("file"), "target\n").unwrap();
            git(wt, &["commit", "-q", "-am", "target edit"]);
            git(wt, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
            git(wt, &["checkout", "-q", "wt"]);
            std::fs::write(wt.join("file"), "wt\n").unwrap();
            git(wt, &["commit", "-q", "-am", "wt edit"]);

            let outcome = update_from_target(path, None).unwrap();
            let UpdateOutcome::WouldConflict(found) = outcome else {
                panic!("{outcome:?}");
            };
            assert_eq!(
                found.branches,
                [(Some(full("refs/heads/wt")), 1)],
                "the update rebases wt with middle, and its edit of `file` meets the target's"
            );
        })
    }

    /// Pushes go without the app's askpass broker, which a process can turn off only once.
    fn without_askpass() {
        static ASKPASS: std::sync::Once = std::sync::Once::new();
        ASKPASS.call_once(but_askpass::disable);
    }

    #[test]
    fn a_worktree_branch_pushes_the_stack_branches_it_rests_on() {
        use crate::backend::ops::tests::git;
        without_askpass();
        crate::backend::lane_tests::linked_workspace(|path| {
            let remote = tempfile::tempdir().unwrap();
            git(remote.path(), &["init", "-q", "--bare"]);
            let url = remote.path().to_str().unwrap();
            git(path, &["remote", "set-url", "origin", url]);
            let info = but_api::legacy::workspace::head_info(&super::open(path).unwrap()).unwrap();
            let branch = gix::refs::FullName::try_from("refs/heads/wt").unwrap();
            let mut guarded: Vec<_> = pushed_with(&info, branch.as_ref())
                .filter_map(|segment| Some(format!("{}\n", segment.ref_name()?)))
                .collect();
            guarded.sort();

            super::push(path, &full("refs/heads/wt"), false, None, false).unwrap();

            let arrived = git(remote.path(), &["for-each-ref", "--format=%(refname)"]);
            assert_eq!(
                guarded.concat(),
                arrived,
                "the guards cover what the push sends"
            );
            // wt rests on middle, so the push takes middle and bottom along.
            snapbox::assert_data_eq!(
                arrived,
                snapbox::str![[r#"
refs/heads/bottom
refs/heads/middle
refs/heads/wt

"#]]
            );
        })
    }

    #[test]
    fn a_worktree_started_on_the_workspace_commit_isnt_pushed() {
        use crate::backend::ops::tests::git;
        without_askpass();
        crate::backend::lane_tests::linked_workspace(|path| {
            let remote = tempfile::tempdir().unwrap();
            git(remote.path(), &["init", "-q", "--bare"]);
            let url = remote.path().to_str().unwrap();
            git(path, &["remote", "set-url", "origin", url]);
            // As an agent makes one from the main checkout, on its `HEAD`, and one on that.
            let outside = tempfile::tempdir().unwrap();
            let at = |name: &str| outside.path().join(name);
            let on_head = at("on-head");
            let on_head_dir = on_head.to_str().unwrap();
            git(
                path,
                &["worktree", "add", "-q", "-b", "on-head", on_head_dir],
            );
            git(
                &on_head,
                &["commit", "-q", "--allow-empty", "-m", "on HEAD"],
            );
            let on_it = at("on-it");
            let on_it_dir = on_it.to_str().unwrap();
            git(
                path,
                &["worktree", "add", "-q", "-b", "on-it", on_it_dir, "on-head"],
            );
            git(&on_it, &["commit", "-q", "--allow-empty", "-m", "on it"]);

            let loaded = crate::backend::load_workspace(path, Default::default()).unwrap();
            for branch in ["refs/heads/on-head", "refs/heads/on-it"].map(full) {
                assert!(
                    (loaded.push_refused(&branch)).is_some_and(|why| why.contains("workspace")),
                    "{branch} isn't offered a push, as it would publish every applied branch"
                );
                let err = super::push(path, &branch, false, None, false)
                    .expect_err("nor does the push go ahead");
                assert!(
                    err.to_string().contains("GitButler's workspace commit"),
                    "{branch}: {err:#}"
                );
            }
            assert_eq!(
                git(remote.path(), &["for-each-ref"]),
                "",
                "nothing reached the remote"
            );
        })
    }

    #[test]
    fn a_day_goes_by_the_clock_here_on_that_day_not_today() {
        use chrono::TimeZone as _;
        // Half past every hour of 2026, both changes of clocks included where the clocks
        // change. Where they don't, as in UTC, it can't tell.
        let year = 1_767_225_600;
        let epoch = chrono::NaiveDate::default();
        for at in (0..365 * 24).map(|hour| year + hour * 3600 + 1800) {
            let here = chrono::Local.timestamp_opt(at, 0).unwrap().date_naive();
            assert_eq!(
                day_of(at),
                (here - epoch).num_days(),
                "{at} falls on {here} here, by that day's offset from UTC"
            );
        }
    }

    #[test]
    fn sums_up_the_target_by_day_and_author() {
        but_testsupport::isolated_app_data_dir(|| {
            use std::process::Command;
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path();
            let git = |args: &[&str], at: i64| {
                let date = format!("@{at} +0000");
                let out = Command::new("git")
                    .current_dir(path)
                    .args(["-c", "commit.gpgsign=false"])
                    .args(args)
                    .env("GIT_AUTHOR_DATE", &date)
                    .env("GIT_COMMITTER_DATE", &date)
                    .output()
                    .unwrap();
                assert!(out.status.success(), "git {args:?}: {out:?}");
            };
            let now = 2_000_000_000;
            git(&["init", "-q", "-b", "main"], now);
            let commit = |who: &str, at: i64| {
                let author = format!("{who} <{}@example.com>", who.to_lowercase());
                let message = format!("by {who}");
                git(&["config", "user.name", who], at);
                git(
                    &[
                        "config",
                        "user.email",
                        &format!("{}@example.com", who.to_lowercase()),
                    ],
                    at,
                );
                git(
                    &[
                        "commit",
                        "-q",
                        "--allow-empty",
                        "--author",
                        &author,
                        "-m",
                        &message,
                    ],
                    at,
                );
            };
            commit("Old", now - 40 * DAY);
            std::fs::write(path.join("notes.txt"), "not it").unwrap();
            std::fs::write(path.join("README.md"), "# Hello\n").unwrap();
            git(&["add", "."], now - 3 * DAY);
            commit("Ann", now - 3 * DAY);
            commit("Bob", now - 2 * DAY);
            git(&["checkout", "-q", "-b", "side"], now - DAY);
            commit("Cat", now - DAY);
            git(&["checkout", "-q", "main"], now - DAY / 2);
            git(
                &["merge", "-q", "--no-ff", "-m", "merge side", "side"],
                now - DAY / 2,
            );
            // Built on the branch merged before, so it holds Cat's commit too.
            git(&["checkout", "-q", "-b", "later", "side"], now - DAY / 3);
            std::fs::create_dir_all(path.join("crates/but/src")).unwrap();
            std::fs::create_dir_all(path.join("docs")).unwrap();
            std::fs::write(path.join("crates/but/src/lib.rs"), "").unwrap();
            std::fs::write(path.join("docs/guide.md"), "").unwrap();
            git(&["add", "."], now - DAY / 3);
            commit("Dan", now - DAY / 3);
            commit("Dan", now - DAY / 3);
            git(&["checkout", "-q", "main"], now - DAY / 4);
            git(
                &["merge", "-q", "--no-ff", "-m", "merge later", "later"],
                now - DAY / 4,
            );
            commit("Ann", now - 60);

            crate::backend::settings::set_target(path, &full("refs/heads/main")).unwrap();
            let summed = overview(path, now).unwrap().expect("a target");
            assert_eq!(summed.daily.len(), ACTIVITY_DAYS, "a bar a day");
            assert_eq!(
                summed.daily.iter().sum::<u32>(),
                5,
                "the target's own line, as its history lists it: each merge once, not the commits \
                 they brought, and not the commit from before the window"
            );
            assert_eq!(summed.daily[ACTIVITY_DAYS - 1], 1, "the last day is today");
            let people: Vec<(&str, &str, u32)> = (summed.people.iter())
                .map(|p| (p.name.as_str(), p.email.as_str(), p.daily.iter().sum()))
                .collect();
            assert_eq!(
                people,
                [
                    ("Ann", "ann@example.com", 2),
                    ("Dan", "dan@example.com", 1),
                    ("Cat", "cat@example.com", 1),
                    ("Bob", "bob@example.com", 1),
                ],
                "most landings first, the merged branches' authors too and the merges left out; \
                 Dan's two commits landed in one merge, once"
            );
            let repo = gix::open(path).unwrap();
            let landed_by = |name: &str| {
                let person = summed.people.iter().find(|p| p.name == name).unwrap();
                person.landed.clone()
            };
            let main = |back: usize| {
                let at = repo.rev_parse_single(format!("main~{back}").as_str());
                [super::CommitId(at.unwrap().detach())].into()
            };
            assert_eq!(
                landed_by("Cat"),
                main(2),
                "the first merge landed Cat's commit, which the target's own line lists in its \
                 place, and the later merge, whose branch holds it too, didn't land it again"
            );
            assert_eq!(landed_by("Dan"), main(1), "the later merge landed Dan's");
            let landings: Vec<(&str, u32, Vec<&str>)> = (summed.landings.iter())
                .map(|l| {
                    (
                        l.title.as_str(),
                        l.commits,
                        l.areas.iter().map(String::as_str).collect(),
                    )
                })
                .collect();
            assert_eq!(
                landings,
                [
                    ("by Ann", 1, vec![]),
                    ("merge later", 2, vec!["crates/but", "docs"]),
                    ("merge side", 1, vec![]),
                    ("by Bob", 1, vec![]),
                    ("by Ann", 1, vec!["Top level"]),
                ],
                "newest first, each with the commits it brought and where it changed things: the \
                 first two folders of a file, its folder when one deep, the top level for one there"
            );
        })
    }

    #[test]
    fn people_are_named_as_the_mailmap_has_them() {
        but_testsupport::isolated_app_data_dir(|| {
            use crate::backend::ops::tests::{git, repo_with};
            let dir = repo_with("one\n");
            let path = dir.path();
            for (email, message) in [("ama@gmail.com", "home"), ("ama@acme.io", "work")] {
                let author = format!("--author=Ama Silva <{email}>");
                git(path, &["commit", "--allow-empty", &author, "-qm", message]);
            }
            std::fs::write(path.join(".mailmap"), "<ama@acme.io> <ama@gmail.com>\n").unwrap();

            crate::backend::settings::set_target(path, &full("refs/heads/main")).unwrap();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            let summed = overview(path, now).unwrap().expect("a target");
            let people: Vec<(&str, u32)> = (summed.people.iter())
                .map(|p| (p.email.as_str(), p.daily.iter().sum()))
                .collect();
            assert_eq!(
                people,
                [("ama@acme.io", 2), ("t@example.com", 1)],
                "one person with a work and a personal email counts once, as `git shortlog` does"
            );
        })
    }

    #[test]
    fn a_refused_push_keeps_only_what_the_remote_said() {
        use but_error::Code;
        let refused = |stderr: &str| {
            anyhow::anyhow!("git command exited with non-zero exit code 1:\n\nSTDERR:\n{stderr}")
                .context(Code::GitForcePushProtection)
        };
        let protected = refused(
            "remote: error: GH006: Protected branch update failed for refs/heads/main.\n\
             remote: \n ! [remote rejected] main -> main (protected branch hook declined)",
        );
        assert_eq!(
            push_refusal(&protected).as_deref(),
            Some("error: GH006: Protected branch update failed for refs/heads/main."),
            "the remote's own words, blank lines dropped"
        );
        let stale = refused(" ! [rejected] main -> main (stale info)");
        assert_eq!(
            push_refusal(&stale).as_deref(),
            Some(""),
            "nothing from the remote: someone pushed since the last fetch"
        );
        let other = anyhow::anyhow!("failed to connect");
        assert_eq!(push_refusal(&other), None, "not protection at all");
    }
}
