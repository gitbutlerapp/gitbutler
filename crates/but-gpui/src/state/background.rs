//! What the app asks for on its own: fetches as often as GitButler's shared setting says,
//! and the pull requests and CI the forge has.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use futures::{StreamExt as _, channel::mpsc};
use gpui::{Context, Task};

use super::{ProjectId, Reload, Store};
use crate::backend::reviews::{self, MergeStatus};
use crate::backend::{self, Checks, Remark, Review};

/// How long Later puts the update card off. It comes back after, as updates shouldn't wait
/// for good.
const UPDATE_LATER: Duration = Duration::from_secs(4 * 60 * 60);

/// What the forge said about whether a pull request can merge, and what it was asked for:
/// when the forge was last asked about the project, and the request's last change and
/// checks then.
pub(super) struct MergeRead {
    asked_for: (Option<i64>, Option<i64>, Option<Checks>),
    /// `None` until it answers; why it couldn't, if it couldn't.
    status: Option<Result<MergeStatus, String>>,
}

/// What the forge said has been written on a pull request, and what it was asked for: when
/// the forge was last asked about the project, and the request's last change then.
pub(super) struct ConversationRead {
    asked_for: (Option<i64>, Option<i64>),
    /// `None` until it answers. A failure shows only until a read works: after that, what
    /// was read last stays while the forge can't be asked, which the tab's banner says.
    said: Option<Result<Vec<Remark>, String>>,
}

impl Store {
    /// Fetch the expanded projects as often as GitButler's shared setting says, so what the
    /// target and remotes have stays current. Quietly: a failed fetch only logs, and the
    /// next one tries again.
    pub(super) fn fetch_in_background(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            // A few seconds after launch, by when the projects have usually loaded, as the
            // remote may have moved while the app was closed; then each minute.
            let mut wait = Duration::from_secs(5);
            loop {
                cx.background_executor().timer(wait).await;
                wait = Duration::from_secs(60);
                let interval = backend::run(|| Ok(backend::settings::fetch_interval())).await;
                let Ok(Some(minutes)) = interval else {
                    continue;
                };
                let interval = Duration::from_secs(minutes * 60);
                let Ok(due) = this.update(cx, |store, _| store.due_for_fetch(interval)) else {
                    return;
                };
                for (project, path) in due {
                    // The fetches before it take a while, and an operation started meanwhile
                    // would write refs beside it: it's due again in a minute.
                    let busy = this.update(cx, |store, _| store.pending(&project, None, false));
                    if busy.unwrap_or(true) {
                        continue;
                    }
                    let fetched = backend::run(move || backend::fetch(&path)).await;
                    if let Err(err) = &fetched {
                        tracing::warn!(%project, "background fetch failed: {err:#}");
                    }
                    let reloaded = this.update(cx, |store, cx| {
                        match fetched {
                            Ok(any_remote) => store.fetched_now(&project, any_remote),
                            Err(_) => store.fetch_failed_now(&project),
                        };
                        // What a fetch changes is in the git directory, which the watcher
                        // reloads for.
                        let watched =
                            (store.project(&project)).is_some_and(|entry| entry.watcher.is_some());
                        // A fetch writes refs, not files.
                        if fetched.is_err() || !watched {
                            store.load(&project, Reload::commits(), cx);
                        }
                    });
                    if reloaded.is_err() {
                        return;
                    }
                }
            }
        })
    }

    /// Ask the desktop app's update feed, as often as its settings say, whether there's a
    /// newer version, when the desktop app's binary started this client. Quietly: a failed
    /// check only logs, and the next one tries again.
    pub(super) fn check_for_updates(cx: &mut Context<Self>) -> Task<()> {
        let Some(crate::Embedded {
            update_feed: Some(feed),
            check_every,
            ..
        }) = cx.try_global::<crate::Embedded>().cloned()
        else {
            return Task::ready(());
        };
        cx.spawn(async move |this, cx| {
            loop {
                let feed = feed.clone();
                match backend::run(move || backend::settings::newer_version(&feed)).await {
                    Ok(newer) => {
                        let changed = this.update(cx, |store, cx| {
                            if store.newer_version != newer {
                                store.newer_version = newer;
                                cx.notify();
                            }
                        });
                        if changed.is_err() {
                            return;
                        }
                    }
                    Err(err) => tracing::warn!("couldn't check for updates: {err:#}"),
                }
                cx.background_executor().timer(check_every).await;
            }
        })
    }

    /// Put the update card off for [`UPDATE_LATER`], leaving the navigator's New version.
    pub fn put_off_update(&mut self, cx: &mut Context<Self>) {
        self.update_put_off = Some(cx.spawn(async |this, cx| {
            cx.background_executor().timer(UPDATE_LATER).await;
            this.update(cx, |store, cx| store.show_update(cx)).ok();
        }));
        cx.notify();
    }

    /// Show the update card again, now.
    pub fn show_update(&mut self, cx: &mut Context<Self>) {
        self.update_put_off = None;
        cx.notify();
    }

    /// How far installing the newer version has got, while it installs.
    pub fn update_progress(&self) -> Option<String> {
        Some(match self.updating? {
            0 => "Downloading…".to_owned(),
            100.. => "Installing…".to_owned(),
            percent => format!("Downloading {percent}%"),
        })
    }

    /// When the app last fetched `id`.
    pub fn fetched_at(&self, id: &ProjectId) -> Option<SystemTime> {
        self.fetched.get(id).copied()
    }

    /// When a fetch of `id` last failed, if none has worked since.
    pub fn fetch_failed_at(&self, id: &ProjectId) -> Option<SystemTime> {
        self.fetch_failed.get(id).copied()
    }

    /// Note that a fetch of `project` failed. Once may be the network not back yet, as just
    /// after the Mac wakes, so the next check tries again; twice holds off a while.
    pub(super) fn fetch_failed_now(&mut self, project: &ProjectId) {
        let at = match self.fetch_failed.contains_key(project) {
            true => SystemTime::now(),
            false => SystemTime::UNIX_EPOCH,
        };
        self.fetch_failed.insert(project.clone(), at);
    }

    /// Note that a fetch of `project` worked, or with `any_remote` false that it found no
    /// remote to fetch from, which isn't a fetch and holds off the next one a while.
    pub(super) fn fetched_now(&mut self, project: &ProjectId, any_remote: bool) {
        match any_remote {
            true => {
                self.fetched.insert(project.clone(), SystemTime::now());
                self.fetch_failed.remove(project);
            }
            false => _ = self.fetch_failed.insert(project.clone(), SystemTime::now()),
        }
    }

    /// The watched projects due for a fetch, each with its path. Not one with an operation
    /// waiting or running, which would race the fetch for its refs.
    fn due_for_fetch(&self, interval: Duration) -> Vec<(ProjectId, PathBuf)> {
        self.projects
            .iter()
            .filter(|entry| entry.expanded && entry.workspace.is_some())
            .filter(|entry| !self.pending(&entry.id, None, false))
            .filter(|entry| {
                const AFTER_FAILING: Duration = Duration::from_secs(30 * 60);
                (self.fetch_failed.get(&entry.id)).is_none_or(|at| since(at) >= AFTER_FAILING)
            })
            .filter(|entry| {
                self.fetched
                    .get(&entry.id)
                    .is_none_or(|at| since(at) >= interval)
            })
            .map(|entry| (entry.id.clone(), entry.project.path.clone()))
            .collect()
    }

    /// Ask the forge about `id`'s pull requests and CI in the background, then show what
    /// changed. Quiet when it can't: plenty of projects have no forge account.
    pub fn refresh_forge(&mut self, id: &ProjectId) {
        if self.forge_queued.insert(id.clone()) {
            self.forge_requests.unbounded_send(id.clone()).ok();
        }
    }

    /// Whether `id`'s forge is being asked about its pull requests right now.
    pub fn forge_checking(&self, id: &ProjectId) -> bool {
        self.forge_checking.as_ref() == Some(id)
    }

    /// Ask the forges that failed again, as once an account to read them with is added.
    pub fn retry_forges(&mut self) {
        for id in std::mem::take(&mut self.forge_failed) {
            self.refresh_forge(&id);
        }
    }

    /// Whether asking `id`'s forge failed last time, so what shows of it may be old.
    pub fn forge_failed(&self, id: &ProjectId) -> bool {
        self.forge_failed.contains(id)
    }

    /// When asking `id`'s forge last worked in this run, in seconds since the epoch.
    pub fn forge_synced(&self, id: &ProjectId) -> Option<i64> {
        self.forge_synced.get(id).copied()
    }

    /// Ask the forge whether `review` of `project` can merge, unless it was asked since the
    /// forge was last asked about the project, with the request and its checks as they are.
    /// What it said before shows meanwhile. GitHub works the answer out when asked, so while
    /// it doesn't know yet this asks again a few times. Not while the window is hidden, as
    /// nobody sees the answer; the reload as it shows again comes back here.
    pub fn read_merge_status(
        &mut self,
        project: &ProjectId,
        review: &Review,
        cx: &mut Context<Self>,
    ) {
        if self.hidden {
            return;
        }
        let asked_for = (self.forge_synced(project), review.updated_at, review.checks);
        let key = (project.clone(), review.number);
        let status = match self.merge_reads.remove(&key) {
            Some(read) if read.asked_for == asked_for => {
                self.merge_reads.insert(key, read);
                return;
            }
            read => read.and_then(|read| read.status),
        };
        let Some(path) = self.path(project) else {
            return;
        };
        self.merge_reads
            .insert(key.clone(), MergeRead { asked_for, status });
        let number = review.number;
        cx.spawn(async move |this, cx| {
            let mut asked = 0;
            let status = loop {
                let path = path.clone();
                let status = backend::run(move || reviews::merge_status(&path, number)).await;
                asked += 1;
                let unknown = status.as_ref().is_ok_and(|status| {
                    !status.is_mergeable
                        && matches!(
                            status.mergeable_state.as_deref(),
                            None | Some("unknown" | "checking" | "unchecked")
                        )
                });
                if !unknown || asked == 4 {
                    break status;
                }
                cx.background_executor().timer(Duration::from_secs(2)).await;
            };
            this.update(cx, |store, cx| {
                // Not over what a newer ask found.
                if let Some(read) =
                    (store.merge_reads.get_mut(&key)).filter(|read| read.asked_for == asked_for)
                {
                    read.status = Some(status.map_err(|err| format!("{err:#}")));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// What the forge last said about whether pull request `number` of `project` can merge.
    pub fn merge_status(
        &self,
        project: &ProjectId,
        number: u64,
    ) -> Option<&Result<MergeStatus, String>> {
        (self.merge_reads.get(&(project.clone(), number)))?
            .status
            .as_ref()
    }

    /// Ask the forge what's been written on pull request `number` of `project`, last changed
    /// at `updated_at`, unless it was asked since the forge was last asked about the project,
    /// with the request as it is. What it said before shows meanwhile. Not while the window is
    /// hidden, as nobody sees the answer; the reload as it shows again comes back here.
    pub fn read_conversation(
        &mut self,
        project: &ProjectId,
        number: u64,
        updated_at: Option<i64>,
        cx: &mut Context<Self>,
    ) {
        if self.hidden {
            return;
        }
        let asked_for = (self.forge_synced(project), updated_at);
        let key = (project.clone(), number);
        if (self.conversation_reads.get(&key)).is_some_and(|read| read.asked_for == asked_for) {
            return;
        }
        let Some(path) = self.path(project) else {
            return;
        };
        let said = (self.conversation_reads.remove(&key)).and_then(|read| read.said);
        (self.conversation_reads).insert(key.clone(), ConversationRead { asked_for, said });
        cx.spawn(async move |this, cx| {
            let said = backend::run(move || reviews::conversation(&path, number)).await;
            this.update(cx, |store, cx| {
                // Not over what a newer ask found.
                let Some(read) = (store.conversation_reads.get_mut(&key))
                    .filter(|read| read.asked_for == asked_for)
                else {
                    return;
                };
                match said {
                    Ok(said) => read.said = Some(Ok(said)),
                    Err(err) if matches!(read.said, Some(Ok(_))) => {
                        tracing::debug!(number, "couldn't read the conversation again: {err:#}");
                        return;
                    }
                    Err(err) => read.said = Some(Err(format!("{err:#}"))),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// What the forge last said has been written on pull request `number` of `project`.
    pub fn conversation(
        &self,
        project: &ProjectId,
        number: u64,
    ) -> Option<&Result<Vec<Remark>, String>> {
        (self.conversation_reads.get(&(project.clone(), number)))?
            .said
            .as_ref()
    }

    /// Pull requests and CI change on the forge, where no watcher sees them: opening a
    /// project asks about them, and this asks again every minute, when it also redraws.
    /// Hidden, it asks every fifth minute, as agents read the forge's cache through `but`
    /// though nobody sees the window, and showing the window asks at once if it skipped one.
    pub(super) fn poll_forge(cx: &mut Context<Self>) -> Task<()> {
        const HIDDEN_EVERY: u32 = 5;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                // Not the projects that failed last time: that's a missing account or a
                // keychain prompt the user turned down, and asking again every minute
                // would only nag. Opening a project or pushing asks again.
                let Ok(()) = this.update(cx, |store, cx| {
                    match store.hidden && store.forge_skipped + 1 < HIDDEN_EVERY {
                        true => store.forge_skipped += 1,
                        false => store.ask_forges(cx),
                    }
                }) else {
                    break;
                };
            }
        })
    }

    /// Ask each watched project's forge about its pull requests and CI again.
    pub(super) fn ask_forges(&mut self, cx: &mut Context<Self>) {
        tracing::debug!(hidden = self.hidden, "asking the forges");
        self.forge_skipped = 0;
        for id in self.expanded_ids() {
            if !self.forge_failed.contains(&id) {
                self.refresh_forge(&id);
            }
        }
        // What says how long ago, or that a project is active, moves on with the minute,
        // forge or not.
        cx.notify();
    }

    /// Works through forge refreshes one at a time. Reading the forge token can stop on a
    /// keychain prompt, and one prompt is plenty; it also keeps the forge's rate limit happy.
    pub(super) fn forge_worker(
        mut requested: mpsc::UnboundedReceiver<ProjectId>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |this, cx| {
            while let Some(id) = requested.next().await {
                let Ok(path) = this.update(cx, |store, _| {
                    store.forge_queued.remove(&id);
                    let open = store.project(&id).filter(|entry| entry.expanded);
                    open.map(|entry| entry.project.path.clone())
                }) else {
                    return;
                };
                let Some(path) = path else {
                    continue;
                };
                let Ok(()) = this.update(cx, |store, cx| {
                    store.forge_checking = Some(id.clone());
                    cx.notify();
                }) else {
                    return;
                };
                let reviews_path = path.clone();
                let refreshed = backend::run(move || backend::refresh_reviews(&reviews_path)).await;
                // The pull requests show without waiting for their checks.
                let Ok(_) = this.update(cx, |store, cx| match &refreshed {
                    Ok(changed) => {
                        store.forge_failed.remove(&id);
                        store.forge_synced.insert(id.clone(), crate::ui::now());
                        // The forge's answers are in the commits and branches, not the files.
                        if *changed {
                            store.load(&id, Reload::commits(), cx);
                        }
                    }
                    Err(_) => {
                        store.forge_failed.insert(id.clone());
                    }
                }) else {
                    return;
                };
                let checked = match refreshed {
                    Ok(_) => Some(backend::run(move || backend::refresh_checks(&path)).await),
                    Err(err) => {
                        tracing::debug!(%id, "forge refresh failed: {err:#}");
                        None
                    }
                };
                let reloaded = this.update(cx, |store, cx| {
                    match checked {
                        Some(Ok(true)) => store.load(&id, Reload::commits(), cx),
                        Some(Ok(false)) | None => {}
                        Some(Err(err)) => tracing::debug!(%id, "CI refresh failed: {err:#}"),
                    }
                    store.forge_checking = None;
                    cx.notify();
                });
                if reloaded.is_err() {
                    return;
                }
            }
        })
    }
}

/// How long ago `at` was, by the wall clock; long ago when the clock was set back past it.
fn since(at: &SystemTime) -> Duration {
    at.elapsed().unwrap_or(Duration::MAX)
}
