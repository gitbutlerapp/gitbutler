//! The remotes and the target branch: pushing and pulling branches, updating the workspace
//! from the target, fetching and landing; and the target's history, with what came in that
//! the workspace doesn't have yet, the pull request each commit landed, and when the
//! project last fetched.

use std::path::Path;

use anyhow::Result;
use bstr::ByteSlice;
use but_core::DryRun;
use but_core::sync::RepoExclusive;

use super::ops::{full_name, holds};
use super::reviews::{cached_reviews, from_cache};
use super::{Agent, CommitId, RemoteCommit, Review, ReviewState, open, runtime, short_ref, title};

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
/// and whether there are older ones.
pub fn target_commits(
    path: &Path,
    after: Option<CommitId>,
    limit: u32,
) -> Result<(Vec<TargetCommit>, bool)> {
    target_page(&open(path)?, after, limit)
}

/// The newest `limit` of the target's commits the workspace doesn't have yet, each named by
/// the pull request it landed, as the project's page names them.
pub(super) fn incoming(ctx: &but_ctx::Context, limit: usize) -> Result<Vec<RemoteCommit>> {
    let (commits, _) = target_page(ctx, None, limit as u32)?;
    Ok((commits.into_iter())
        .filter(|commit| commit.incoming)
        .map(|commit| RemoteCommit {
            id: commit.id,
            key: String::new(),
            authored: String::new(),
            title: (commit.review)
                .map(|review| review.title)
                .filter(|title| !title.is_empty())
                .unwrap_or(commit.title),
            author: commit.author,
            email: commit.email,
            agent: commit.agent,
            time: commit.time,
            merge: commit.merge,
        })
        .collect())
}

fn target_page(
    ctx: &but_ctx::Context,
    after: Option<CommitId>,
    limit: u32,
) -> Result<(Vec<TargetCommit>, bool)> {
    let page = but_api::target_commits::workspace_target_commits(
        ctx,
        after.map(|id| but_api::json::HexHash(id.0)),
        Some(limit),
    )?;
    let cached = cached_reviews(ctx);
    let repo = ctx.repo.get()?;
    let commits = page
        .commits
        .into_iter()
        .map(|target| {
            let commit = target.commit;
            let author = (commit.author.name, commit.author.email);
            let committer = super::committer(&repo, commit.id).unwrap_or_else(|| author.clone());
            let ((author, email), agent) = super::byline(author, committer);
            TargetCommit {
                id: CommitId(commit.id),
                title: title(&commit.message),
                body: (commit.message.lines().skip(1))
                    .map(|line| line.to_str_lossy())
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim()
                    .to_owned(),
                time: (commit.committed_at / 1000) as i64,
                author,
                email,
                agent,
                review: target.review.map(|review| {
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
                }),
                incoming: !target.in_workspace,
                merge: (repo.find_commit(commit.id)).is_ok_and(|c| c.parent_ids().count() > 1),
            }
        })
        .collect::<Vec<TargetCommit>>();
    // The first page stops where the workspace forked off, with older commits below that
    // unless it's where the history starts.
    let forked_above_root = after.is_none()
        && (commits.last()).is_some_and(|last| {
            // A shallow clone's oldest commits name parents it doesn't have.
            (repo.find_commit(last.id.0))
                .is_ok_and(|c| c.parent_ids().next().is_some_and(|p| repo.has_object(p)))
        });
    Ok((commits, page.has_more || forked_above_root))
}

/// How many days back the project's page counts the target's activity.
pub const ACTIVITY_DAYS: usize = 30;

const DAY: i64 = 24 * 60 * 60;

/// The day `seconds` (since the epoch) falls on here, numbered from the epoch's, so a day
/// picked stays that day as the clock moves on.
pub fn day_of(seconds: i64) -> i64 {
    (seconds + local_offset()).div_euclid(DAY)
}

/// When day `day`, as [`day_of`] numbers it, starts here, in seconds since the epoch.
pub fn day_start(day: i64) -> i64 {
    day * DAY - local_offset()
}

fn local_offset() -> i64 {
    gix::date::Time::now_local_or_utc().offset as i64
}

/// The target as the project's page sums it up: its README, and who's been committing to it.
#[derive(Debug, Clone, Default)]
pub struct Overview {
    /// The README's file name and text at the target's tip.
    pub readme: Option<(String, String)>,
    /// How many commits landed on the target's own line, as its history lists them, each of
    /// the last [`ACTIVITY_DAYS`] days here, oldest first.
    pub daily: Vec<u32>,
    /// Who wrote the commits on all its lines, merges left out, most commits first: name,
    /// email and how many, each by [`super::byline`].
    pub team: Vec<(String, String, u32)>,
    /// The day the first of `daily` counts, as [`day_of`] numbers days.
    pub first_day: i64,
}

/// `target` (a short name, like `origin/main`) summed up as of `now`, in seconds since the
/// epoch. A commit that can't be read is left out rather than failing the rest.
pub fn overview(path: &Path, target: &str, now: i64) -> Result<Overview> {
    use gix::revision::walk::Sorting;
    use gix::traverse::commit::simple::CommitTimeOrder;
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let tip = repo.rev_parse_single(target)?.object()?.peel_to_commit()?;
    let first_day = day_of(now) - (ACTIVITY_DAYS as i64 - 1);
    let since = day_start(first_day);
    let newest_since = Sorting::ByCommitTimeCutoff {
        order: CommitTimeOrder::NewestFirst,
        seconds: since,
    };
    let mut daily = vec![0; ACTIVITY_DAYS];
    let line = (repo.rev_walk([tip.id]))
        .first_parent_only()
        .sorting(newest_since)
        .all()?;
    for commit in commits_since(line, since) {
        let day = commit.time().map_or(first_day, |time| day_of(time.seconds));
        // A clock ahead of this one counts as today.
        daily[((day - first_day) as usize).min(ACTIVITY_DAYS - 1)] += 1;
    }
    let mut team: Vec<(String, String, u32)> = Vec::new();
    let all = (repo.rev_walk([tip.id])).sorting(newest_since).all()?;
    for commit in commits_since(all, since).filter(|commit| commit.parent_ids().count() < 2) {
        let (Ok(author), Ok(committer)) = (commit.author(), commit.committer()) else {
            continue;
        };
        let ((name, email), _) = super::byline(
            super::signature(author.name, author.email),
            super::signature(committer.name, committer.email),
        );
        let email = email.to_lowercase();
        match team.iter_mut().find(|(_, known, _)| *known == email) {
            Some(person) => person.2 += 1,
            None => team.push((name, email, 1)),
        }
    }
    team.sort_by_key(|person| std::cmp::Reverse(person.2));
    let readme = readme(&repo, &tip)
        .inspect_err(|err| tracing::warn!(?err, "couldn't read the README"))
        .ok()
        .flatten();
    Ok(Overview {
        readme,
        daily,
        team,
        first_day,
    })
}

/// The commits of `walk` from `since` on, in seconds since the epoch, leaving out any that
/// can't be read.
fn commits_since(
    walk: gix::revision::Walk<'_>,
    since: i64,
) -> impl Iterator<Item = gix::Commit<'_>> {
    walk.filter_map(|info| info.ok()?.object().ok())
        .filter(move |commit| commit.time().is_ok_and(|time| time.seconds >= since))
}

/// The README at the top of `commit`'s tree, Markdown first, by its file name and text.
fn readme(repo: &gix::Repository, commit: &gix::Commit<'_>) -> Result<Option<(String, String)>> {
    const NAMES: [&str; 5] = [
        "readme.md",
        "readme.markdown",
        "readme",
        "readme.txt",
        "readme.rst",
    ];
    let tree = commit.tree()?;
    let found = (tree.iter())
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.filename().to_str_lossy();
            let rank = NAMES.iter().position(|n| name.eq_ignore_ascii_case(n))?;
            (entry.mode().is_blob()).then(|| (rank, name.into_owned(), entry.object_id()))
        })
        .min_by_key(|(rank, ..)| *rank);
    let Some((_, name, id)) = found else {
        return Ok(None);
    };
    let blob = repo.find_object(id)?;
    Ok(Some((
        name,
        String::from_utf8_lossy(&blob.data).into_owned(),
    )))
}

/// How large an image a README shows may be.
const MAX_IMAGE_BYTES: usize = 8 << 20;

/// An image a README shows, by its `src`: a web address, fetched, or a path from the top of
/// the repository, read from `target` (a short name, like `origin/main`).
pub fn readme_image(path: &Path, target: &str, src: &str) -> Result<Vec<u8>> {
    let lower = src.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        let url = reqwest::Url::parse(src)?;
        anyhow::ensure!(public_https(&url), "{src} isn't a public https address");
        return runtime().block_on(async {
            let mut response = image_client().get(url).send().await?.error_for_status()?;
            // Read as it comes, as a server needn't say how much it sends.
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                bytes.extend_from_slice(&chunk);
                anyhow::ensure!(bytes.len() <= MAX_IMAGE_BYTES, "{src} is too large to show");
            }
            Ok(bytes)
        });
    }
    let ctx = open(path)?;
    let repo = ctx.repo.get()?;
    let tree = (repo.rev_parse_single(target)?.object()?)
        .peel_to_commit()?
        .tree()?;
    let entry = (tree.lookup_entry_by_path(src)?)
        .ok_or_else(|| anyhow::anyhow!("{target} has no {src}"))?;
    let blob = entry.object()?;
    anyhow::ensure!(
        blob.data.len() <= MAX_IMAGE_BYTES,
        "{src} is too large to show"
    );
    Ok(blob.data.clone())
}

/// The one client README images are fetched with, which follows a redirect only to another
/// public https address.
fn image_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        let policy = reqwest::redirect::Policy::custom(|attempt| {
            match attempt.previous().len() < 5 && public_https(attempt.url()) {
                true => attempt.follow(),
                false => attempt.stop(),
            }
        });
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .redirect(policy)
            .build()
            .unwrap_or_default()
    })
}

/// Whether `url` is https on the internet: a README's images mustn't reach this machine or
/// the network it's on.
fn public_https(url: &reqwest::Url) -> bool {
    use std::net::IpAddr;
    let public = |ip: IpAddr| match ip {
        IpAddr::V4(ip) => {
            !(ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_unspecified())
        }
        IpAddr::V6(ip) => {
            let local =
                (ip.segments()[0] & 0xfe00) == 0xfc00 || (ip.segments()[0] & 0xffc0) == 0xfe80;
            !(local || ip.is_loopback() || ip.is_unspecified())
        }
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = (host.trim_start_matches('[').trim_end_matches(']')).to_ascii_lowercase();
    url.scheme() == "https"
        && match host.parse::<IpAddr>() {
            Ok(ip) => public(ip),
            Err(_) => {
                host != "localhost" && !host.ends_with(".localhost") && !host.ends_with(".local")
            }
        }
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
    let walk = (repo.rev_walk(others))
        .with_hidden([first])
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
        .all()?;
    let mut commits = Vec::new();
    for info in walk.take(limit + 1) {
        let commit = info?.object()?;
        let message = commit.message_raw_sloppy();
        let author = commit.author()?;
        let author = (
            author.name.to_str_lossy().into_owned(),
            author.email.to_str_lossy().into_owned(),
        );
        let committer = super::committer(&repo, commit.id).unwrap_or_else(|| author.clone());
        let ((author, email), agent) = super::byline(author, committer);
        commits.push(TargetCommit {
            id: CommitId(commit.id),
            title: title(message),
            body: (message.lines().skip(1))
                .map(|line| line.to_str_lossy())
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_owned(),
            time: commit.time()?.seconds,
            author,
            email,
            agent,
            review: None,
            incoming: false,
            merge: commit.parent_ids().count() > 1,
        });
    }
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
    // worktree, as an agent fetching in its own leaves one there.
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
    ref_name: &str,
    protect: bool,
    drops: Option<&[CommitId]>,
    run_hooks: bool,
) -> Result<(Option<String>, Option<String>)> {
    // Forced, as decided when it runs, the push goes past the lease, which would refuse to
    // drop even the commits only the remote has that were shown. Unless pushing anyway, it
    // fetches again first and stops if the remote moved, as a teammate's amend still matches
    // the commit rewritten here, or has commits that weren't shown.
    if let Some(shown) = drops {
        let (info, tips) = with_remote_tips(path, ref_name)?;
        if needs_force(&info, ref_name) {
            fetch(path)?;
            let (info, now) = with_remote_tips(path, ref_name)?;
            // A stack pushes the branches below it too: the refusal names the one it's about.
            let name = |segment: Option<&but_workspace::ref_info::Segment>| {
                (segment.and_then(|segment| segment.ref_name())).map_or_else(
                    || short_ref(ref_name).to_owned(),
                    |n| n.shorten().to_string(),
                )
            };
            if now != tips {
                let moved = name(
                    (pushed_with(&info, ref_name).zip(tips.iter().zip(&now)))
                        .find(|(_, (before, after))| before != after)
                        .map(|(segment, _)| segment),
                );
                anyhow::bail!(
                    "Someone pushed to {moved} since the last fetch, and forcing would replace \
                     it. Check what's there before pushing again."
                );
            }
            if let Some(unseen) = pushed_with(&info, ref_name).find(|segment| {
                (segment.commits_on_remote.iter()).any(|c| !shown.contains(&CommitId(c.id)))
            }) {
                anyhow::bail!(
                    "The remote has commits on {} that weren't shown, and forcing would drop \
                     them. Push again to see them.",
                    name(Some(unseen))
                );
            }
        }
    }
    let ctx = open(path)?;
    // Decided when it runs, as an amend queued before it rewrites the branch.
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    // As in the desktop app, a conflicted commit stays here until it's resolved.
    if pushed_with(&info, ref_name).any(|segment| segment.commits.iter().any(|c| c.has_conflicts)) {
        anyhow::bail!("Resolve the conflicted commits first, so they aren't pushed");
    }
    let force = needs_force(&info, ref_name);
    let outcome = runtime()
        .handle()
        .block_on(
            but_api::legacy::workspace::workspace_branch_and_ancestors_push(
                ctx.into_sync(),
                force,
                !protect,
                ref_name.to_owned(),
                run_hooks,
                Vec::new(),
            ),
        )
        .map_err(|err| {
            if err.downcast_ref::<but_error::Code>() != Some(&but_error::Code::GitNonFastForward) {
                return err;
            }
            tracing::warn!(?err, "push rejected");
            anyhow::anyhow!("The remote has commits this branch doesn't. Fetch, then pull them in.")
        })?;
    let short = short_ref(ref_name);
    let remote = (outcome.push.branch_to_remote.into_iter())
        .find(|(branch, _, _)| branch == short || branch == ref_name)
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

/// Whether pushing `ref_name` has to force: it or a branch below it in its stack was rewritten
/// since it was pushed.
fn needs_force(info: &but_workspace::RefInfo, ref_name: &str) -> bool {
    use but_core::ui::PushStatus;
    pushed_with(info, ref_name)
        .any(|segment| segment.push_status == PushStatus::UnpushedCommitsRequiringForce)
}

/// The workspace now, and where the remote-tracking branches of `ref_name` and the ones below
/// it point.
fn with_remote_tips(
    path: &Path,
    ref_name: &str,
) -> Result<(but_workspace::RefInfo, Vec<Option<gix::ObjectId>>)> {
    let ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let repo = ctx.repo.get()?;
    let tips = pushed_with(&info, ref_name)
        .map(|segment| {
            (segment.remote_tracking_ref_name.as_ref())
                .and_then(|name| repo.find_reference(name.as_ref()).ok())
                .and_then(|mut tip| tip.peel_to_id().ok())
                .map(|id| id.detach())
        })
        .collect();
    Ok((info, tips))
}

/// `ref_name`'s branch and the ones below it in its stack, which pushing it pushes too.
fn pushed_with<'a>(
    info: &'a but_workspace::RefInfo,
    ref_name: &'a str,
) -> impl Iterator<Item = &'a but_workspace::ref_info::Segment> {
    (info.stacks.iter()).flat_map(move |stack| {
        (stack.segments.iter()).skip_while(move |segment| !holds(segment, ref_name))
    })
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
pub fn pull_remote(path: &Path, branch: &str, theirs: bool) -> Result<()> {
    integrate_remote(&mut open(path)?, branch, theirs, DryRun::No)?;
    Ok(())
}

/// A dry run of pulling into `branch`: the branches (full refs) it would leave with
/// conflicted commits they don't have now, each with how many.
pub fn pull_conflicts(path: &Path, branch: &str) -> Result<Vec<(String, usize)>> {
    let mut ctx = open(path)?;
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    let preview = integrate_remote(&mut ctx, branch, false, DryRun::Yes)?;
    Ok(newly_conflicted(
        &info,
        &preview.workspace.conflicted_commits,
    ))
}

fn integrate_remote(
    ctx: &mut but_ctx::Context,
    branch: &str,
    theirs: bool,
    dry_run: DryRun,
) -> Result<but_api::branch::IntegrateBranchResult> {
    use but_api::branch::json::BranchIntegrationStrategy as Strategy;
    let name = full_name(branch)?;
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
    but_api::branch::apply_branch_integration_with_perm(
        ctx,
        name.as_ref(),
        plan.integration,
        dry_run,
        guard.write_permission(),
    )
}

/// What [`update_from_target()`] did.
#[derive(Debug)]
pub enum UpdateOutcome {
    /// It would have left these conflicted that aren't now, so nothing changed.
    WouldConflict(UpdateConflicts),
    Updated {
        /// Branches (full refs) left with conflicted commits they didn't have, each with
        /// how many.
        conflicted: Vec<(String, usize)>,
        /// Branches (full refs) that landed on the target, which the update removed.
        landed: Vec<String>,
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
    let info = but_api::legacy::workspace::head_info(&ctx)?;
    // Held from the dry run through the update, so nothing lands in between.
    let mut guard = ctx.exclusive_worktree_access();
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
    let names = |info: &but_workspace::RefInfo| -> Vec<String> {
        (info.stacks.iter())
            .flat_map(|stack| &stack.segments)
            .filter_map(|segment| Some(segment.ref_name()?.to_string()))
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
    pub branches: Vec<(String, usize)>,
    /// Uncommitted files the target changed too, which get conflict markers.
    pub files: Vec<String>,
}

/// The branches (full refs, empty for an unnamed one) in `info` with commits among
/// `conflicted` that aren't conflicted yet, each with how many.
fn newly_conflicted(
    info: &but_workspace::RefInfo,
    conflicted: &[(gix::ObjectId, but_core::ChangeId)],
) -> Vec<(String, usize)> {
    // A commit made without a change ID gets one from its commit ID, as the rebase does.
    let newly = |commit: &&but_workspace::ref_info::LocalCommit| {
        let id = commit.inner.change_id();
        !commit.inner.has_conflicts && conflicted.iter().any(|(_, change)| *change == *id)
    };
    (info.stacks.iter())
        .flat_map(|stack| &stack.segments)
        .filter_map(|segment| {
            let count = segment.commits.iter().filter(newly).count();
            let name = segment.ref_name().map(ToString::to_string);
            (count > 0).then(|| (name.unwrap_or_default(), count))
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

/// Put `branch` (a short name), or with `whole_stack` it and every branch below it,
/// straight onto the target and push the target. Returns whether the other branches still
/// need an update to sit on the moved target.
pub fn land(path: &Path, branch: &str, tip: CommitId, whole_stack: bool) -> Result<bool> {
    let mut ctx = open(path)?;
    // A push can't be undone, so not of commits an agent added after the user was asked.
    let now = {
        let repo = ctx.repo.get()?;
        (repo.try_find_reference(&format!("refs/heads/{branch}"))?)
            .map(|mut r| r.peel_to_id().map(|id| id.detach()))
            .transpose()?
    };
    if now != Some(tip.0) {
        anyhow::bail!("{branch} changed while you were asked; look at it again");
    }
    // A merge commit when the branch has diverged, as `but land` makes one without its flags.
    let landed =
        but_api::land::branch_land(&mut ctx, branch.to_owned(), false, false, whole_stack)?;
    Ok(landed.reconcile_skipped)
}

#[cfg(test)]
mod tests {
    use super::{ACTIVITY_DAYS, DAY, overview, public_https, push_refusal};

    #[test]
    fn sums_up_the_target_by_day_and_author() {
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
        commit("Ann", now - 60);

        let summed = overview(path, "main", now).unwrap();
        assert_eq!(
            summed.readme,
            Some(("README.md".to_owned(), "# Hello\n".to_owned())),
            "the README at the tip, by its name"
        );
        assert_eq!(summed.daily.len(), ACTIVITY_DAYS, "a bar a day");
        assert_eq!(
            summed.daily.iter().sum::<u32>(),
            4,
            "the target's own line, as its history lists it: the merge once, not the commit it \
             brought, and not the commit from before the window"
        );
        assert_eq!(summed.daily[ACTIVITY_DAYS - 1], 1, "the last day is today");
        assert_eq!(
            summed.team,
            [
                ("Ann".to_owned(), "ann@example.com".to_owned(), 2),
                ("Cat".to_owned(), "cat@example.com".to_owned(), 1),
                ("Bob".to_owned(), "bob@example.com".to_owned(), 1),
            ],
            "most commits first, the merged branch's author too and the merge left out"
        );
    }

    #[test]
    fn readme_images_come_only_from_public_https_addresses() {
        let public = |url: &str| public_https(&reqwest::Url::parse(url).unwrap());
        assert!(
            public("https://img.shields.io/badge/ci-passing.svg"),
            "a badge service"
        );
        for url in [
            "http://example.com/logo.png",
            "https://localhost/logo.png",
            "https://127.0.0.1/logo.png",
            "https://192.168.1.1/logo.png",
            "https://10.0.0.2/logo.png",
            "https://[::1]/logo.png",
            "https://printer.local/logo.png",
        ] {
            assert!(
                !public(url),
                "{url} is plain http or on this machine or its network"
            );
        }
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
