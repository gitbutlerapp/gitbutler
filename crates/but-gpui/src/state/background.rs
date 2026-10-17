//! What the app asks for on its own: fetches as often as GitButler's shared setting says,
//! and the pull requests and CI the forge has.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures::{StreamExt as _, channel::mpsc};
use gpui::{AsyncApp, Context, Task, WeakEntity};

use super::{ProjectId, Store};
use crate::backend;

impl Store {
    /// Fetch the expanded projects as often as GitButler's shared setting says, so what the
    /// target and remotes have stays current. Quietly: a failed fetch only logs, and the
    /// next one tries again.
    pub(super) fn fetch_in_background(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                let interval = backend::run(|| Ok(backend::settings::fetch_interval())).await;
                let Ok(Some(minutes)) = interval else {
                    continue;
                };
                let interval = Duration::from_secs(minutes * 60);
                let Ok(due) = this.update(cx, |store, _| store.due_for_fetch(interval)) else {
                    return;
                };
                for (project, path) in due {
                    let fetched = backend::run(move || backend::fetch(&path)).await;
                    if let Err(err) = &fetched {
                        tracing::warn!(%project, "background fetch failed: {err:#}");
                    }
                    let reloaded = this.update(cx, |store, cx| {
                        store.fetched.insert(project.clone(), Instant::now());
                        match fetched {
                            Ok(()) => store.fetch_failed.remove(&project),
                            Err(_) => store.fetch_failed.insert(project.clone(), Instant::now()),
                        };
                        store.reload(&project, cx);
                    });
                    if reloaded.is_err() {
                        return;
                    }
                }
            }
        })
    }

    fn due_for_fetch(&self, interval: Duration) -> Vec<(ProjectId, PathBuf)> {
        self.projects
            .iter()
            .filter(|entry| entry.expanded && entry.workspace.is_some())
            .filter(|entry| {
                const AFTER_FAILING: Duration = Duration::from_secs(30 * 60);
                (self.fetch_failed.get(&entry.id)).is_none_or(|at| at.elapsed() >= AFTER_FAILING)
            })
            .filter(|entry| {
                self.fetched
                    .get(&entry.id)
                    .is_none_or(|at| at.elapsed() >= interval)
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

    /// Pull requests and CI change on the forge, where no watcher sees them: opening a
    /// project asks about them, and this asks again every minute.
    pub(super) fn poll_forge(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(60))
                    .await;
                // Not the projects that failed last time: that's a missing account or a
                // keychain prompt the user turned down, and asking again every minute
                // would only nag. Opening a project or pushing asks again.
                let Ok(ids) = this.update(cx, |store, _| {
                    let mut ids = store.expanded_ids();
                    ids.retain(|id| !store.forge_failed.contains(id));
                    ids
                }) else {
                    break;
                };
                for id in ids {
                    if this
                        .update(cx, |store, _| store.refresh_forge(&id))
                        .is_err()
                    {
                        return;
                    }
                }
            }
        })
    }

    /// Works through forge refreshes one at a time. Reading the forge token can stop on a
    /// keychain prompt, and one prompt is plenty; it also keeps the forge's rate limit happy.
    pub(super) async fn forge_worker(
        this: WeakEntity<Store>,
        mut requested: mpsc::UnboundedReceiver<ProjectId>,
        cx: &mut AsyncApp,
    ) {
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
            // Pull requests show as soon as they're listed; their checks take a request each.
            let reviews_path = path.clone();
            let refreshed = backend::run(move || backend::refresh_reviews(&reviews_path)).await;
            let Ok(_) = this.update(cx, |store, _| match &refreshed {
                Ok(()) => store.forge_failed.remove(&id),
                Err(_) => store.forge_failed.insert(id.clone()),
            }) else {
                return;
            };
            if let Err(err) = refreshed {
                tracing::debug!(%id, "forge refresh failed: {err:#}");
                continue;
            }
            if this.update(cx, |store, cx| store.reload(&id, cx)).is_err() {
                return;
            }
            match backend::run(move || backend::refresh_checks(&path)).await {
                Ok(()) => {
                    if this.update(cx, |store, cx| store.reload(&id, cx)).is_err() {
                        return;
                    }
                }
                Err(err) => tracing::debug!(%id, "CI refresh failed: {err:#}"),
            }
        }
    }
}
