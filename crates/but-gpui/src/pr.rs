//! A branch's pull request: opening one (pushing the branch first when it needs it), and
//! merging one that's open, at once or by auto-merge once it can.

use gpui::{
    Action, App, Context, DismissEvent, Div, Entity, EventEmitter, FocusHandle, Focusable,
    ScrollHandle, SharedString, Stateful, Task, Window, actions, div, prelude::*,
};

use crate::app::AppView;
use crate::backend::ai;
use crate::backend::reviews::{self, MergeMethod, MergeStatus};
use crate::backend::{self, Forge, FullName, Review, ReviewState, short_ref};
use crate::state::{Overlay, ProjectId, Selection, Store, StoreEvent, snack};
use crate::theme::Theme;
use crate::ui::modal::{ModalSize, modal};
use crate::ui::text_input::{InputEvent, Newline, TextInput};
use crate::ui::{ButtonStyle, button, dp, faint, field, h_flex, muted, switch, v_flex};

actions!(
    pr,
    [
        CreatePullRequest,
        MergePullRequest,
        EnableAutoMerge,
        DisableAutoMerge
    ]
);

pub struct ReviewView {
    store: Entity<Store>,
    project: ProjectId,
    /// The branch, by full ref.
    branch: FullName,
    title: Entity<TextInput>,
    body: Entity<TextInput>,
    /// What scrolls the description's box, which keeps the caret in view.
    body_scroll: ScrollHandle,
    /// The branch it would merge into, once known.
    target: Option<String>,
    draft: bool,
    /// The title and description being written with AI, streaming into the fields.
    writing: Option<Task<()>>,
}

impl EventEmitter<DismissEvent> for ReviewView {}

impl Focusable for ReviewView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.title.focus_handle(cx)
    }
}

impl ReviewView {
    fn new(
        store: Entity<Store>,
        project: ProjectId,
        branch: FullName,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| TextInput::new("Title", false, cx));
        let body_scroll = ScrollHandle::new();
        let body = cx.new(|cx| {
            TextInput::new("What it changes, and why", true, cx).scrolled_by(body_scroll.clone())
        });
        for input in [&title, &body] {
            cx.subscribe_in(input, window, |this, _, event, _, cx| match event {
                InputEvent::Submit => this.submit(cx),
                InputEvent::Cancel => cx.emit(DismissEvent),
                InputEvent::Changed => cx.notify(),
            })
            .detach();
        }
        // What's written stays for the branch under its new name, as an agent renamed it.
        cx.subscribe(&store, |this, _, event, cx| match event {
            StoreEvent::Renamed(project, from, to)
                if *project == this.project && from.as_ref() == Some(&this.branch) =>
            {
                this.branch = to.clone();
                cx.notify();
            }
            _ => {}
        })
        .detach();
        let path = store.read(cx).path(&project);
        // A branch of one commit starts from that commit's message, as `but pr` does.
        let (subject, description) = store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_branch(Some(&branch)))
            .and_then(|b| match b.commits.as_slice() {
                [commit] => Some(&commit.message),
                _ => None,
            })
            .map(|message| backend::subject_and_body(message))
            .unwrap_or_else(|| (short_ref(&branch).into_owned(), String::new()));
        title.update(cx, |input, cx| {
            input.set_text(subject, cx);
            input.select_all_text(cx);
        });
        body.update(cx, |input, cx| input.set_text(description, cx));
        let ref_name = branch.clone();
        if let Some(path) = path {
            cx.spawn(async move |this, cx| {
                let found = backend::run(move || reviews::new_review(&path, &ref_name)).await;
                this.update(cx, |this, cx| match found {
                    Ok(new) => {
                        this.target = Some(new.target);
                        // What's typed meanwhile stays, as the template fills only an empty body.
                        let empty = this.body.read(cx).text().is_empty();
                        if let Some(template) = new.template.filter(|_| empty) {
                            this.body
                                .update(cx, |input, cx| input.set_text(template, cx));
                        }
                        cx.notify();
                    }
                    Err(err) => this.store.update(cx, |store, cx| {
                        store.failed("Can't open a pull request", &err, cx)
                    }),
                })
                .ok();
            })
            .detach();
        }
        Self {
            store,
            project,
            branch,
            title,
            body,
            body_scroll,
            target: None,
            draft: false,
            writing: None,
        }
    }

    /// Write the title and description with AI from the branch's commit messages, streaming
    /// them into the fields. What's written so far guides it.
    fn write_with_ai(&mut self, cx: &mut Context<Self>) {
        if self.writing.is_some() {
            return;
        }
        // Oldest first, the order the work happened in.
        let messages: Vec<String> = self
            .store
            .read(cx)
            .workspace(&self.project)
            .and_then(|ws| ws.find_branch(Some(&self.branch)))
            .map(|b| b.commits.iter().rev().map(|c| c.message.clone()).collect())
            .unwrap_or_default();
        let (title, body) = (
            self.title.read(cx).text().to_owned(),
            self.body.read(cx).text().to_owned(),
        );
        let prompt = ai::pr_prompt(&title, &body, &messages);
        let fields = (self.title.clone(), self.body.clone());
        for field in [&fields.0, &fields.1] {
            field.update(cx, |input, _| input.break_undo_run());
        }
        let task = cx.spawn(async move |this, cx| {
            let written = ai::write(ai::PR_SYSTEM, prompt, |text| {
                let (title, body) = ai::split_title_body(text);
                fields.0.update(cx, |input, cx| input.set_text(title, cx));
                fields.1.update(cx, |input, cx| input.set_text(body, cx));
            })
            .await;
            this.update(cx, |this, cx| {
                this.writing = None;
                match written {
                    Ok(text) => {
                        let (new_title, new_body) = ai::split_title_body(&text);
                        this.title
                            .update(cx, |input, cx| input.set_text(new_title, cx));
                        this.body
                            .update(cx, |input, cx| input.set_text(new_body, cx));
                    }
                    Err(err) => {
                        this.title.update(cx, |input, cx| input.set_text(title, cx));
                        this.body.update(cx, |input, cx| input.set_text(body, cx));
                        this.store.update(cx, |store, cx| {
                            store.failed("Couldn't write the description", &err, cx)
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.writing = Some(task);
        cx.notify();
    }

    /// Close the form and create the pull request: the branch's row shows it coming, as the
    /// push and the forge take seconds, and a failure's card tries again with what was typed.
    fn submit(&mut self, cx: &mut Context<Self>) {
        let title = self.title.read(cx).text().trim().to_owned();
        if title.is_empty() || self.writing.is_some() {
            return;
        }
        let body = self.body.read(cx).text().trim().to_owned();
        let (project, branch, draft) = (self.project.clone(), self.branch.clone(), self.draft);
        (self.store).update(cx, |store, cx| {
            store.publish(&project, branch, title, body, draft, cx)
        });
        cx.emit(DismissEvent);
    }
}

impl Render for ReviewView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let route = match &self.target {
            Some(target) => format!("{} → {target}", short_ref(&self.branch)),
            None => short_ref(&self.branch).into_owned(),
        };
        // Creating it pushes first when it or a branch below it has something to push.
        let push = (self.store.read(cx).workspace(&self.project))
            .filter(|ws| ws.can_push(&self.branch))
            .map(|ws| match ws.needs_force(&self.branch) {
                true => "Gets force-pushed first, since it or a branch below it was rewritten",
                false => "Gets pushed first",
            });
        let text_box = |input: &Entity<TextInput>, tall: bool| {
            let focused = input.focus_handle(cx).is_focused(window);
            field(input, focused, &theme)
                .id(if tall { "pr-body" } else { "pr-title" })
                .when(tall, |el| {
                    (el.min_h(dp(180.)).max_h(dp(320.)))
                        .overflow_y_scroll()
                        .track_scroll(&self.body_scroll)
                })
        };
        let label = |text: &'static str| {
            div()
                .text_size(dp(12.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text_2)
                .child(text)
        };
        let confirm = if self.draft {
            "Create draft"
        } else {
            "Create pull request"
        };
        let empty_title = self.title.read(cx).text().trim().is_empty();
        modal(ModalSize::Medium, &theme)
            .id("pull-request")
            .child(
                v_flex()
                    .flex_none()
                    .gap(dp(4.))
                    .px(dp(20.))
                    .pt(dp(18.))
                    .pb(dp(12.))
                    .child(muted(
                        (self.store.read(cx)).place(&Selection::Branch(
                            self.project.clone(),
                            Some(self.branch.clone()),
                        )),
                        &theme,
                    ))
                    .child(
                        div()
                            .text_size(dp(15.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Create a pull request"),
                    )
                    .child(
                        div()
                            .font_family(theme.mono_font.clone())
                            .text_size(dp(12.))
                            .text_color(theme.text_2)
                            .child(SharedString::from(route)),
                    )
                    .children(push.map(|note| muted(note, &theme))),
            )
            .child(
                v_flex()
                    .px(dp(20.))
                    .pb(dp(16.))
                    .gap(dp(6.))
                    .child(label("Title"))
                    .child(text_box(&self.title, false).capture_action(cx.listener(
                        |this, _: &Newline, window, cx| {
                            window.focus(&this.body.focus_handle(cx), cx);
                            cx.stop_propagation();
                        },
                    )))
                    .child(div().h(dp(6.)))
                    .child(
                        h_flex()
                            .child(div().flex_1().child(label("Description")))
                            .child(
                                button(
                                    "pr-write",
                                    if self.writing.is_some() {
                                        "Writing…"
                                    } else {
                                        "Write with AI"
                                    },
                                )
                                .small()
                                .style(ButtonStyle::Ghost)
                                .disabled_because(self.writing.is_some().then_some("Writing…"))
                                .tooltip("Write the title and description from the commits")
                                .on_click(cx.listener(|this, _, _, cx| this.write_with_ai(cx))),
                            ),
                    )
                    .child(text_box(&self.body, true))
                    .child(
                        h_flex()
                            .pt(dp(8.))
                            .gap(dp(10.))
                            .child(switch("pr-draft", self.draft, &theme).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.draft = !this.draft;
                                    cx.notify();
                                },
                            )))
                            .child(
                                v_flex()
                                    .child(div().text_size(dp(13.)).child("Draft"))
                                    .child(muted("Not ready for review yet", &theme)),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .justify_end()
                    .gap(dp(8.))
                    .px(dp(20.))
                    .py(dp(12.))
                    .border_t_1()
                    .border_color(theme.border_3)
                    .child(faint(crate::ui::keys_text("⌘↩ to create"), &theme).flex_1())
                    .child(
                        button("pr-cancel", "Cancel")
                            .style(ButtonStyle::Ghost)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    )
                    .child(
                        button("pr-submit", confirm)
                            .style(ButtonStyle::Gray)
                            .disabled_because(if self.writing.is_some() {
                                Some("Wait for the AI to finish writing")
                            } else if empty_title {
                                Some("Give it a title first")
                            } else {
                                None
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                    ),
            )
    }
}

impl AppView {
    /// Register the pull request actions on the window where they apply to the selected
    /// branch, or the selected commit's branch: what the navigator's menu offers for it.
    pub fn pr_actions(&self, el: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        let store = self.store.read(cx);
        let Some((selection, branch)) = (store.selection.as_ref())
            .filter(|s| on_forge(store, s))
            .and_then(|s| Some((s, store.branch_for(s)?)))
            .filter(|(_, b)| b.ref_name.is_some())
        else {
            return el;
        };
        let project = selection.project();
        match (branch.review.as_ref()).map(|r| (r, store.review_state(project, r))) {
            None => el.when(!branch.commits.is_empty(), |el| {
                el.on_action(cx.listener(Self::create_pull_request))
            }),
            Some((review, ReviewState::Open)) => match mergeability(store, project, review).offer()
            {
                Some(Offer::Merge) => el.on_action(cx.listener(Self::merge_pull_request)),
                Some(Offer::EnableAutoMerge) => el.on_action(cx.listener(Self::enable_auto_merge)),
                Some(Offer::DisableAutoMerge) => {
                    el.on_action(cx.listener(Self::disable_auto_merge))
                }
                None => el,
            },
            Some((_, ReviewState::Draft | ReviewState::Closed | ReviewState::Merged)) => el,
        }
    }

    /// The selected branch, or the branch of the selected commit, with its pull request if
    /// it has one.
    fn review_target(&self, cx: &App) -> Option<(ProjectId, FullName, Option<Review>)> {
        let store = self.store.read(cx);
        let selection = store.selection.clone()?;
        let branch = store.branch_for(&selection)?;
        let review = branch.review.clone().map(|review| Review {
            state: store.review_state(selection.project(), &review),
            ..review
        });
        Some((
            selection.project().clone(),
            branch.ref_name.clone()?,
            review,
        ))
    }

    pub fn create_pull_request(
        &mut self,
        _: &CreatePullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((project, branch, None)) = self.review_target(cx) else {
            return;
        };
        // The form closes as it's submitted, so another would open a second pull request.
        if self.store.read(cx).publishing(&project, &branch) {
            return snack(&self.store, "Its pull request is being created", cx);
        }
        // Creating one pushes first.
        let refused =
            (self.store.read(cx).workspace(&project)).and_then(|ws| ws.push_refused(&branch));
        if let Some(refused) = refused {
            return snack(&self.store, refused, cx);
        }
        let store = self.store.clone();
        let view = cx.new(|cx| ReviewView::new(store, project, branch, window, cx));
        self.open_modal(view, window, cx);
    }

    /// Merge the pull request, the way its project last picked.
    pub fn merge_pull_request(
        &mut self,
        _: &MergePullRequest,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((
            project,
            branch,
            Some(Review {
                number,
                state: ReviewState::Open,
                ..
            }),
        )) = self.review_target(cx)
        else {
            return;
        };
        if let Some(why) = push_first(self.store.read(cx), &project, &branch) {
            return snack(&self.store, why, cx);
        }
        let method = merge_method(self.store.read(cx), &project).unwrap_or(MergeMethod::Merge);
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Merge pull request",
                Overlay {
                    merges: Some(number),
                    ..Overlay::remote()
                },
                move |path, _| reviews::merge(path, number, method),
                |store, merged, cx| {
                    let said = match merged {
                        true => "Merged, Update brings it into your workspace",
                        false => "In the merge queue, it merges when the queue gets to it",
                    };
                    store.snack(said, cx)
                },
                cx,
            )
        });
    }

    pub fn enable_auto_merge(
        &mut self,
        _: &EnableAutoMerge,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_auto_merge(true, cx);
    }

    pub fn disable_auto_merge(
        &mut self,
        _: &DisableAutoMerge,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_auto_merge(false, cx);
    }

    /// Turn the pull request's auto-merge on or off. It's shown at once as it'll be.
    fn set_auto_merge(&mut self, on: bool, cx: &mut Context<Self>) {
        let Some((
            project,
            branch,
            Some(Review {
                number,
                state: ReviewState::Open,
                ..
            }),
        )) = self.review_target(cx)
        else {
            return;
        };
        if let Some(why) = push_first(self.store.read(cx), &project, &branch).filter(|_| on) {
            return snack(&self.store, why, cx);
        }
        let (label, done) = match on {
            true => (
                "Enable auto-merge",
                "Auto-merge is on: it merges once its required checks and reviews pass",
            ),
            false => ("Disable auto-merge", "Auto-merge is off"),
        };
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                label,
                Overlay {
                    auto_merges: Some((number, on)),
                    ..Overlay::remote()
                },
                move |path, _| reviews::set_auto_merge(path, number, on),
                move |store, (), cx| store.snack(done, cx),
                cx,
            )
        });
    }
}

/// Why `branch` (a full ref) can't merge, nor auto-merge, yet: the forge merges what it has,
/// not the commits only here.
pub fn push_first(store: &Store, project: &ProjectId, branch: &FullName) -> Option<&'static str> {
    (store.workspace(project))
        .is_some_and(|ws| ws.can_push(branch))
        .then_some("Push first, so the pull request has these commits")
}

/// Where an open pull request stands on merging, which decides the one thing it offers, as
/// GitHub's merge box does: merging once it can, auto-merge while it waits on required checks
/// or reviews, and turning auto-merge off while that's on.
#[derive(Debug, Clone, PartialEq)]
pub enum Mergeability {
    /// Auto-merge is on, so it merges by itself once it can.
    AutoMerging,
    /// It can merge now, with a note when checks that aren't required haven't passed.
    Ready(Option<&'static str>),
    /// It can't merge yet, but auto-merge can wait for it to: for required checks or reviews,
    /// catching up with the branch it merges into, or its conflicts being resolved.
    Waiting(String),
    /// Not even auto-merge can merge it, as a draft or one stacked on another.
    Blocked(String),
    /// The forge hasn't said yet, or couldn't say why, so merging is left to it to refuse.
    Unknown(Option<String>),
}

/// The one thing an open pull request offers on merging.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Offer {
    Merge,
    EnableAutoMerge,
    DisableAutoMerge,
}

impl Offer {
    /// What it's called, merging by `method`, as GitHub's merge button says it.
    pub fn label(self, method: Option<MergeMethod>) -> &'static str {
        match (self, method) {
            (Offer::Merge, Some(MergeMethod::Squash)) => "Squash and merge",
            (Offer::Merge, Some(MergeMethod::Rebase)) => "Rebase and merge",
            (Offer::Merge, Some(MergeMethod::Merge) | None) => "Merge pull request",
            (Offer::EnableAutoMerge, _) => "Enable auto-merge",
            (Offer::DisableAutoMerge, _) => "Disable auto-merge",
        }
    }

    pub fn action(self) -> &'static dyn Action {
        match self {
            Offer::Merge => &MergePullRequest,
            Offer::EnableAutoMerge => &EnableAutoMerge,
            Offer::DisableAutoMerge => &DisableAutoMerge,
        }
    }
}

impl Mergeability {
    /// From whether auto-merge is on, the branch it merges into, `base`, the workspace's
    /// `target` branch without its remote, like `main`, and what the forge last said, if it
    /// was asked. GitHub's and GitLab's words for where it stands differ; GitLab's say nothing
    /// of its pipeline.
    pub fn of(
        auto_merge: bool,
        base: &str,
        target: Option<&str>,
        status: Option<&Result<MergeStatus, String>>,
    ) -> Self {
        let status = match status {
            _ if auto_merge => return Self::AutoMerging,
            // One into another branch is stacked on the pull request below it. GitHub merges a
            // stacked one together with every one below it, so a stack merges from the bottom,
            // as in the desktop app.
            _ if target.is_some_and(|target| target != base) => {
                return Self::Blocked(format!(
                    "Merges into {base}, so the pull requests below it merge first"
                ));
            }
            None => return Self::Unknown(None),
            Some(Err(err)) => return Self::Unknown(Some(err.clone())),
            Some(Ok(status)) => status,
        };
        match (status.is_mergeable, status.mergeable_state.as_deref()) {
            (true, Some("unstable")) => {
                Self::Ready(Some("Checks that aren't required haven't passed"))
            }
            (true, _) => Self::Ready(None),
            (false, Some("blocked")) => {
                Self::Waiting("Waiting on required checks or reviews".into())
            }
            (false, Some("behind")) => Self::Waiting("Behind the branch it merges into".into()),
            (false, Some("dirty" | "cannot_be_merged")) => {
                Self::Waiting("Has conflicts with the branch it merges into".into())
            }
            (false, Some("draft")) => Self::Blocked("A draft can't merge".into()),
            (false, None | Some("unknown" | "checking" | "unchecked")) => Self::Unknown(None),
            (false, Some(state)) => Self::Waiting(format!("Can't merge yet ({state})")),
        }
    }

    pub fn offer(&self) -> Option<Offer> {
        match self {
            Self::AutoMerging => Some(Offer::DisableAutoMerge),
            Self::Ready(_) | Self::Unknown(_) => Some(Offer::Merge),
            Self::Waiting(_) => Some(Offer::EnableAutoMerge),
            Self::Blocked(_) => None,
        }
    }
}

/// Where `review`, open, of `project` stands on merging, as far as the app knows.
pub fn mergeability(store: &Store, project: &ProjectId, review: &Review) -> Mergeability {
    let target = (store.workspace(project))
        .and_then(|ws| ws.target.as_ref())
        .map(|target| target.branch.as_str());
    Mergeability::of(
        store.auto_merge_on(project, review),
        &review.target_branch,
        target,
        store.merge_status(project, review.number),
    )
}

/// How pull requests in `project` merge: as last picked there, with a merge commit until one
/// is. `None` on GitLab, which merges as the project is set up to, whatever is asked for.
pub fn merge_method(store: &Store, project: &ProjectId) -> Option<MergeMethod> {
    let forge = store.workspace(project).and_then(|ws| ws.forge);
    (forge != Some(Forge::GitLab)).then(|| {
        (store.prefs.merge_methods.get(project.as_ref()).copied()).unwrap_or(MergeMethod::Merge)
    })
}

/// Whether `selection`'s project is on a forge, so pull requests apply.
pub fn on_forge(store: &Store, selection: &Selection) -> bool {
    store
        .workspace(selection.project())
        .is_some_and(|ws| ws.forge.is_some())
}

/// Whether the selected branch is offered a pull request to create, a named one on a forge
/// without one yet, and if so why it can't be created now: it has no commits, or can't push
/// them first.
pub fn create_offer(store: &Store, selection: &Selection) -> Option<Result<(), &'static str>> {
    let Selection::Branch(project, Some(ref_name)) = selection else {
        return None;
    };
    let ws = store.workspace(project)?;
    let branch = store.branch_for(selection)?;
    if !on_forge(store, selection) || branch.review.is_some() {
        return None;
    }
    Some(match ws.push_refused(ref_name) {
        _ if branch.commits.is_empty() => Err("Commit something first"),
        Some(refused) => Err(refused),
        None => Ok(()),
    })
}

#[cfg(test)]
mod tests {
    use super::{MergeStatus, Mergeability, Offer};

    #[test]
    fn offers_one_thing_as_github_does() {
        let status = |state: Option<&str>, mergeable| {
            Some(Ok(MergeStatus {
                mergeable_state: state.map(Into::into),
                comments_count: 0,
                is_mergeable: mergeable,
            }))
        };
        let offer = |auto_merge, status: Option<Result<MergeStatus, String>>| {
            Mergeability::of(auto_merge, "main", Some("main"), status.as_ref()).offer()
        };
        assert_eq!(
            [
                offer(false, status(Some("clean"), true)),
                offer(false, status(Some("unstable"), true)),
                offer(false, status(Some("blocked"), false)),
                offer(false, status(Some("behind"), false)),
                offer(false, status(Some("dirty"), false)),
                offer(false, status(Some("ci_must_pass"), false)),
                offer(false, status(Some("draft"), false)),
                offer(false, status(Some("unknown"), false)),
                offer(false, None),
                offer(true, status(Some("blocked"), false)),
                offer(true, status(Some("dirty"), false)),
            ],
            [
                Some(Offer::Merge),
                Some(Offer::Merge),
                Some(Offer::EnableAutoMerge),
                Some(Offer::EnableAutoMerge),
                Some(Offer::EnableAutoMerge),
                Some(Offer::EnableAutoMerge),
                None,
                Some(Offer::Merge),
                Some(Offer::Merge),
                Some(Offer::DisableAutoMerge),
                Some(Offer::DisableAutoMerge),
            ],
            "merge once it can, auto-merge while it can't however checks or conflicts stand, \
             nothing for a draft, and with auto-merge on only turning it off"
        );
    }

    #[test]
    fn a_stacked_pull_request_merges_only_from_the_bottom() {
        let ready = Some(Ok(MergeStatus {
            mergeable_state: Some("clean".into()),
            comments_count: 0,
            is_mergeable: true,
        }));
        let offer = |auto_merge, base, target| {
            Mergeability::of(auto_merge, base, target, ready.as_ref()).offer()
        };
        assert_eq!(
            [
                offer(false, "main", Some("main")),
                offer(false, "main", Some("release/main")),
                offer(false, "gpui-stack-2", Some("main")),
                offer(true, "gpui-stack-2", Some("main")),
                offer(false, "gpui-stack-2", None),
            ],
            [
                Some(Offer::Merge),
                None,
                None,
                Some(Offer::DisableAutoMerge),
                Some(Offer::Merge),
            ],
            "only a pull request into the target, by its whole name, merges, as GitHub merges \
             a stacked one with every one below it; auto-merge already on can still be turned \
             off, and without a target nothing is held back"
        );
    }
}
