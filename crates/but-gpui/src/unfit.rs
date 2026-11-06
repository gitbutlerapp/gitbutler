//! Locked changes going where they lack some of what they build on: the user is asked before
//! the stacks they build on are stacked below where they land, or before they leave the commit
//! they land in conflicted.

use std::path::Path;

use anyhow::Result;
use gpui::{App, Context, PromptLevel, Window};

use crate::app::AppView;
use crate::backend::{self, CommitId, Conflicts, FullName, Stacking, short_ref};
use crate::state::{ProjectId, Selection};

/// Where locked changes land.
pub enum Lands {
    Amend(CommitId),
}

/// Locked changes landing in `project` as `lands` says, after stacking as `restack` says, and
/// taking what doesn't merge there as `conflicts` says.
pub struct Unfit {
    pub project: ProjectId,
    pub lands: Lands,
    pub restack: Option<Stacking>,
    pub conflicts: Conflicts,
}

impl Unfit {
    fn verb(&self) -> &'static str {
        match self.lands {
            Lands::Amend(_) => "amend",
        }
    }

    /// The commit they land in, as a dialog names it.
    fn named(&self, cx: &App, app: &AppView) -> String {
        match self.lands {
            Lands::Amend(id) => (app.store.read(cx).find_commit(&self.project, id))
                .map_or("the commit".into(), |(_, commit)| {
                    format!("\"{}\"", crate::ui::commit_title(&commit.title))
                }),
        }
    }

    /// What the dialog says where they land.
    fn at(&self) -> Selection {
        match self.lands {
            Lands::Amend(id) => Selection::Commit(self.project.clone(), id),
        }
    }
}

impl AppView {
    /// Land `unfit`'s changes as `go` does, once the user says to. Where it restacks, `tried`
    /// says first whether stacking still lets them land.
    pub fn ask_unfit(
        &mut self,
        unfit: Unfit,
        tried: impl FnOnce(&Path, &Stacking) -> Result<bool> + Send + 'static,
        go: impl FnOnce(&mut AppView, &mut Context<AppView>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(stacking) = unfit.restack.clone() else {
            return self.ask_conflicting(unfit, go, window, cx);
        };
        let Some(path) = self.store.read(cx).path(&unfit.project) else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let tried = backend::run(move || tried(&path, &stacking)).await;
            this.update_in(cx, |app, window, cx| match tried {
                Ok(keeps) => app.ask_restack(unfit, keeps, go, window, cx),
                Err(err) => (app.store).update(cx, |store, cx| {
                    store.failed("Couldn't try stacking the branches", &err, cx)
                }),
            })
            .ok();
        })
        .detach();
    }

    /// Ask before landing `unfit`'s changes, which leaves the commit they land in conflicted.
    fn ask_conflicting(
        &mut self,
        unfit: Unfit,
        go: impl FnOnce(&mut AppView, &mut Context<AppView>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let message = match unfit.lands {
            Lands::Amend(_) => format!("Amend {} with conflicts?", unfit.named(cx, self)),
        };
        let detail = "These changes build on commits it doesn't have, so it's left \
                      conflicted. They'll be in its conflict, not in your files, until you \
                      resolve it.";
        let confirm = match unfit.lands {
            Lands::Amend(_) => "Amend",
        };
        self.confirm_then(Some(unfit.at()), message, detail, confirm, window, cx, go);
    }

    /// Ask to stack the stacks `unfit`'s changes build on below where they land, then land
    /// them, when stacking `keeps` them landing as asked, or only to stack them.
    fn ask_restack(
        &mut self,
        unfit: Unfit,
        keeps: bool,
        go: impl FnOnce(&mut AppView, &mut Context<AppView>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(stacking) = unfit.restack.clone() else {
            return;
        };
        let (verb, named) = (unfit.verb(), unfit.named(cx, self));
        let names = |refs: &[FullName]| {
            let names: Vec<_> = refs.iter().map(short_ref).collect();
            names.join(", ")
        };
        let mut detail = format!(
            "These changes build on commits in other stacks. Stacking {} on {} puts those \
             below {named}.",
            names(&stacking.branches),
            short_ref(&stacking.onto),
        );
        match (keeps, unfit.conflicts) {
            (false, _) => detail.push_str(&format!(
                " Stacking leaves {named} conflicted, so it can't take them after. Restack, \
                 then drag them again."
            )),
            (true, Conflicts::Keep) => detail.push_str(&format!(
                " They also build on a commit above it, so {verb}ing leaves it conflicted."
            )),
            (true, Conflicts::Reject) => {}
        }
        match stacking.pushed.as_slice() {
            [] => {}
            [branch] => detail.push_str(&format!(
                " {} is pushed, so it needs a force push.",
                short_ref(branch)
            )),
            pushed => detail.push_str(&format!(
                " {} are pushed, so they need a force push.",
                names(pushed)
            )),
        }
        let detail = format!("{}\n{detail}", self.store.read(cx).place(&unfit.at()));
        let restack_and_land = format!("Restack and {verb}");
        let answers: &[&str] = match keeps {
            true => &[&restack_and_land, "Restack", "Cancel"],
            false => &["Restack", "Cancel"],
        };
        let message = format!("Restack to {verb} {named}?");
        let answer = window.prompt(PromptLevel::Info, &message, Some(&detail), answers, cx);
        cx.spawn(async move |this, cx| {
            let chosen = answer.await.ok();
            this.update(cx, |app, cx| match (keeps, chosen) {
                (true, Some(0)) => go(app, cx),
                (true, Some(1)) | (false, Some(0)) => {
                    let project = unfit.project;
                    (app.store).update(cx, |store, cx| store.restack(&project, stacking, cx))
                }
                _ => {}
            })
            .ok();
        })
        .detach();
    }
}
