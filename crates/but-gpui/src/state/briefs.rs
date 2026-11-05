//! What Briefs says about each project's branches, for pages to draw without asking. As with the
//! forge, a poll keeps it current from the cache `but-briefs` keeps on disk, and a failed ask logs,
//! keeps what's known and is tried again later.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use gpui::{Context, Task};

use super::{ProjectId, Store};
use crate::backend::briefs::{self, Anchored};
use crate::backend::run;

/// How often the branches of the projects that show are asked about again.
const POLL: Duration = Duration::from_secs(5 * 60);

/// What's known of each project's branches, by their names, and what's being asked.
#[derive(Default)]
pub(super) struct BriefAnswers {
    known: HashMap<ProjectId, HashMap<String, Anchored>>,
    /// Projects asked about since the app started or the account changed.
    asked: HashSet<ProjectId>,
    asking: HashSet<ProjectId>,
}

impl Store {
    /// Not while the window is hidden, as nobody sees the answer.
    pub(super) fn poll_briefs(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let Ok(()) = this.update(cx, |store, cx| {
                    if !store.hidden {
                        for id in store.expanded_ids() {
                            store.refresh_briefs(&id, cx);
                        }
                    }
                }) else {
                    break;
                };
            }
        })
    }

    /// A project loaded: what's known of it shows, and it's asked about once, as the poll may be
    /// minutes away.
    pub(super) fn briefs_project_loaded(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        if !self.brief_answers.asked.contains(id) {
            self.refresh_briefs(id, cx);
        }
    }

    /// Show what's known of project `id`, then ask Briefs about its applied branches and its
    /// newest others.
    fn refresh_briefs(&mut self, id: &ProjectId, cx: &mut Context<Self>) {
        if !self.briefs || self.account.user.is_none() {
            return;
        }
        let (Some(path), Some(workspace)) = (self.path(id), self.workspace(id)) else {
            return;
        };
        let applied: Vec<_> = (workspace.all_branches())
            .map(|branch| {
                let commits = branch.commits.iter().map(|commit| commit.id).collect();
                (branch.name.clone(), commits)
            })
            .collect();
        if !self.brief_answers.asking.insert(id.clone()) {
            return;
        }
        self.brief_answers.asked.insert(id.clone());
        let project = id.clone();
        cx.spawn(async move |this, cx| {
            let known = {
                let path = path.clone();
                run(move || briefs::known(&path)).await
            };
            this.update(cx, |store, cx| store.took_briefs(&project, known, cx))
                .ok();
            let asked = run(move || briefs::refresh_project(&path, applied)).await;
            this.update(cx, |store, cx| {
                store.brief_answers.asking.remove(&project);
                store.took_briefs(&project, asked, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Take what's known of `project`'s branches. A failure logs and keeps what was known.
    pub(crate) fn took_briefs(
        &mut self,
        project: &ProjectId,
        read: anyhow::Result<Vec<Anchored>>,
        cx: &mut Context<Self>,
    ) {
        match read {
            Ok(anchors) => {
                let known = (anchors.into_iter())
                    .map(|anchor| (anchor.branch.clone(), anchor))
                    .collect();
                self.brief_answers.known.insert(project.clone(), known);
                cx.notify();
            }
            Err(err) => tracing::debug!("couldn't ask Briefs: {err:#}"),
        }
    }

    /// What's known of `branch` of `project`.
    pub fn brief_at(&self, project: &ProjectId, branch: &str) -> Option<&Anchored> {
        self.brief_answers.known.get(project)?.get(branch)
    }

    /// Someone else is signed in, or no one is, so what's known was another's. Signed out, it
    /// leaves the disk too, as a brief can be private to who read it.
    pub(super) fn briefs_account_changed(&mut self, cx: &mut Context<Self>) {
        self.brief_answers = BriefAnswers::default();
        briefs::account_changed();
        if self.account.user.is_none() {
            cx.spawn(async move |_, _| {
                if let Err(err) = run(briefs::forget).await {
                    tracing::warn!("couldn't forget the briefs kept: {err:#}");
                }
            })
            .detach();
            return;
        }
        for id in self.expanded_ids() {
            self.refresh_briefs(&id, cx);
        }
    }
}
