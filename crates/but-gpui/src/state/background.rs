//! What the app asks for on its own: fetches as often as GitButler's shared setting says,
//! and the pull requests and CI the forge has.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use futures::{StreamExt as _, channel::mpsc};
use gpui::{Context, Task};

use super::{ProjectId, Reload, Store};
use crate::backend;

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
