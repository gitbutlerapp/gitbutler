//! Locked changes going where they lack some of what they build on: the user is asked before
//! the stacks they build on are stacked below where they land, or before they leave the commit
//! they land in conflicted.

use std::path::Path;

use anyhow::Result;
use gpui::{App, Context, PromptLevel, Window};

use crate::app::AppView;
use crate::backend::{self, Checkout, Conflicts, FullName, Stacking, short_ref};
use crate::state::{Held, Lands, Selection, Unfit};

impl Unfit {
    fn verb(&self) -> &'static str {
        match self.lands {
            Lands::Amend(_) => "amend",
            Lands::Commit => "commit",
        }
    }

    /// The commit they land in, as a dialog names it.
    fn named(&self, cx: &App, app: &AppView) -> String {
        match self.lands {
            Lands::Amend(id) => (app.store.read(cx).find_commit(&self.project, id))
                .map_or("the commit".into(), |(_, commit)| {
                    format!("\"{}\"", crate::ui::commit_title(&commit.title))
                }),
            Lands::Commit => "the new commit".into(),
        }
    }

    /// What the dialog says where they land.
    fn at(&self) -> Selection {
        match self.lands {
            Lands::Amend(id) => Selection::Commit(self.project.clone(), id),
            Lands::Commit => Selection::Uncommitted(self.project.clone(), Checkout::Main),
        }
    }
}

impl AppView {
    /// Ask about the landing the store last held back, if one is waiting, and land it once the
    /// user says to.
    pub(crate) fn ask_held(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Held { unfit, tried, go }) = self.store.update(cx, |store, _| store.take_held())
        else {
            return;
        };
        let go = move |app: &mut AppView, cx: &mut Context<AppView>| app.store.update(cx, go);
        self.ask_unfit(unfit, tried, go, window, cx);
    }

    /// Land `unfit`'s changes as `go` does, once the user says to. Where it restacks, `tried`
    /// says first whether stacking still lets them land.
    fn ask_unfit(
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
        let (message, lacks, confirm) = match unfit.lands {
            Lands::Amend(_) => (
                format!("Amend {} with conflicts?", unfit.named(cx, self)),
                "it doesn't have",
                "Amend",
            ),
            Lands::Commit => (
                "Commit with conflicts?".to_owned(),
                "it won't sit on",
                "Commit",
            ),
        };
        let detail = format!(
            "These changes build on commits {lacks}, so it's left conflicted. They'll be in its \
             conflict, not in your files, until you resolve it."
        );
        self.confirm_then(Some(unfit.at()), message, &detail, confirm, window, cx, go);
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
        let above = match unfit.lands {
            Lands::Amend(_) => "above it",
            Lands::Commit => "above where it goes",
        };
        match (keeps, unfit.conflicts, &unfit.lands) {
            (false, _, Lands::Amend(_)) => detail.push_str(&format!(
                " Stacking leaves {named} conflicted, so it can't take them after. Restack, \
                 then try again."
            )),
            (false, _, Lands::Commit) => detail.push_str(
                " Stacking leaves commits conflicted, so these can't be committed after. \
                 Restack, then try again.",
            ),
            (true, Conflicts::Keep, _) => detail.push_str(&format!(
                " They also build on a commit {above}, so {verb}ing leaves it conflicted."
            )),
            (true, Conflicts::Reject, _) => {}
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
        let message = match unfit.lands {
            Lands::Amend(_) => format!("Restack to amend {named}?"),
            Lands::Commit => "Restack to commit?".to_owned(),
        };
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
