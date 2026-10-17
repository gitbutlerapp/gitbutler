//! A branch's pull request: opening one (pushing the branch first when it needs it), and
//! for one that exists, editing it, marking it ready or a draft, merging, closing and
//! reopening it.

use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, PromptLevel,
    SharedString, Task, Window, actions, div, prelude::*, px,
};

use crate::app::AppView;
use crate::backend::ai;
use crate::backend::branches::{self, ApplyOutcome};
use crate::backend::reviews::{self, MergeMethod};
use crate::backend::{self, PushState, ReviewState};
use crate::state::{Overlay, ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::transfer::short_ref;
use crate::ui::modal::{ModalSize, modal};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{ButtonStyle, button, h_flex, switch, v_flex};

actions!(
    pr,
    [
        CreatePullRequest,
        EditPullRequest,
        ToggleDraft,
        ToggleAutoMerge,
        RequestReview,
        CommentOnPullRequest,
        LabelPullRequest,
        ApplyPullRequest,
        MergePullRequest,
        ClosePullRequest,
    ]
);

enum Mode {
    /// A new pull request; the branch is pushed first when that's `Some`, forced if `true`.
    Create {
        push: Option<bool>,
    },
    Edit {
        number: u64,
    },
}

pub struct ReviewView {
    store: Entity<Store>,
    project: ProjectId,
    /// The branch, by full ref.
    branch: String,
    mode: Mode,
    title: Entity<TextInput>,
    body: Entity<TextInput>,
    /// The branch it would merge into, once known.
    target: Option<String>,
    draft: bool,
    /// The title and description being written with AI, streaming into the fields.
    writing: Option<Task<()>>,
    /// The fields hold the pull request's text. Editing waits for it, so saving can't
    /// replace the description with an empty one.
    loaded: bool,
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
        branch: String,
        mode: Mode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| TextInput::new("Title", false, cx));
        let body = cx.new(|cx| TextInput::new("What it changes, and why", true, cx));
        for input in [&title, &body] {
            cx.subscribe_in(input, window, |this, _, event, _, cx| match event {
                InputEvent::Submit => this.submit(cx),
                InputEvent::Cancel => cx.emit(DismissEvent),
                InputEvent::Changed => cx.notify(),
            })
            .detach();
        }
        let path = store.read(cx).path(&project);
        // A branch of one commit starts from that commit's message, as `but pr` does.
        let single = store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_branch(&branch))
            .filter(|b| b.commits.len() == 1)
            .map(|b| {
                let message = &b.commits[0].message;
                let (subject, rest) = message.split_once('\n').unwrap_or((message, ""));
                (subject.trim().to_owned(), rest.trim().to_owned())
            });
        match &mode {
            Mode::Create { .. } => {
                let (subject, description) =
                    single.unwrap_or_else(|| (short_ref(&branch).to_owned(), String::new()));
                title.update(cx, |input, cx| {
                    input.set_text(subject, cx);
                    input.select_all_text(cx);
                });
                body.update(cx, |input, cx| input.set_text(description.clone(), cx));
                let ref_name = branch.clone();
                if let Some(path) = path {
                    cx.spawn(async move |this, cx| {
                        let found =
                            backend::run(move || reviews::new_review(&path, &ref_name)).await;
                        this.update(cx, |this, cx| match found {
                            Ok(new) => {
                                this.target = Some(new.target);
                                if let Some(template) =
                                    new.template.filter(|_| description.is_empty())
                                {
                                    this.body
                                        .update(cx, |input, cx| input.set_text(template, cx));
                                }
                                cx.notify();
                            }
                            Err(err) => this.store.update(cx, |store, cx| {
                                store.error("Can't open a pull request", format!("{err:#}"), cx)
                            }),
                        })
                        .ok();
                    })
                    .detach();
                }
            }
            Mode::Edit { number } => {
                let number = *number;
                if let Some(path) = path {
                    cx.spawn(async move |this, cx| {
                        let text = backend::run(move || reviews::review_text(&path, number)).await;
                        this.update(cx, |this, cx| match text {
                            Ok((title, body)) => {
                                this.title.update(cx, |input, cx| input.set_text(title, cx));
                                this.body.update(cx, |input, cx| input.set_text(body, cx));
                                this.loaded = true;
                                cx.notify();
                            }
                            Err(err) => this.store.update(cx, |store, cx| {
                                store.error(
                                    "Couldn't read the pull request",
                                    format!("{err:#}"),
                                    cx,
                                )
                            }),
                        })
                        .ok();
                    })
                    .detach();
                }
            }
        }
        let loaded = matches!(mode, Mode::Create { .. });
        Self {
            store,
            project,
            branch,
            mode,
            title,
            body,
            target: None,
            draft: false,
            writing: None,
            loaded,
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
            .and_then(|ws| ws.find_branch(&self.branch))
            .map(|b| b.commits.iter().rev().map(|c| c.message.clone()).collect())
            .unwrap_or_default();
        let (title, body) = (
            self.title.read(cx).text().to_owned(),
            self.body.read(cx).text().to_owned(),
        );
        let prompt = ai::pr_prompt(&title, &body, &messages);
        let fields = (self.title.clone(), self.body.clone());
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
                            store.error("Couldn't write the description", format!("{err:#}"), cx)
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

    fn submit(&mut self, cx: &mut Context<Self>) {
        let title = self.title.read(cx).text().trim().to_owned();
        if title.is_empty() || !self.loaded || self.writing.is_some() {
            return;
        }
        let body = self.body.read(cx).text().trim().to_owned();
        let branch = self.branch.clone();
        let project = self.project.clone();
        let refreshed = project.clone();
        match self.mode {
            Mode::Create { push } => {
                let draft = self.draft;
                self.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Create pull request",
                        Overlay::default(),
                        move |path, _| reviews::publish(path, &branch, push, title, body, draft),
                        move |store, number, cx| {
                            store.snack(format!("Created pull request #{number}"), cx);
                            store.refresh_forge(&refreshed);
                        },
                        cx,
                    )
                });
            }
            Mode::Edit { number } => {
                self.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Edit pull request",
                        Overlay::default(),
                        move |path, _| reviews::update(path, number, Some((title, body)), None),
                        move |store, (), cx| {
                            store.snack(format!("Updated #{number}"), cx);
                            store.refresh_forge(&refreshed);
                        },
                        cx,
                    )
                });
            }
        }
        cx.emit(DismissEvent);
    }
}

impl Render for ReviewView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let creating = matches!(self.mode, Mode::Create { .. });
        let heading = match self.mode {
            Mode::Create { .. } => "Create a pull request".to_owned(),
            Mode::Edit { number } => format!("Edit pull request #{number}"),
        };
        let route = match (&self.target, &self.mode) {
            (Some(target), _) => format!("{} → {target}", short_ref(&self.branch)),
            (None, Mode::Create { push: Some(_) }) => {
                format!("{} gets pushed first", short_ref(&self.branch))
            }
            _ => short_ref(&self.branch).to_owned(),
        };
        let field = |input: &Entity<TextInput>, tall: bool| {
            let focused = input.focus_handle(cx).is_focused(window);
            div()
                .id(if tall { "pr-body" } else { "pr-title" })
                .px(px(10.))
                .py(px(7.))
                .rounded(px(6.))
                .border_1()
                .border_color(if focused { theme.pop } else { theme.border_2 })
                .text_size(px(13.))
                .line_height(px(19.))
                .when(tall, |el| {
                    el.min_h(px(180.)).max_h(px(320.)).overflow_y_scroll()
                })
                .child(input.clone())
        };
        let label = |text: &'static str| {
            div()
                .text_size(px(12.))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.text_2)
                .child(text)
        };
        let confirm = match (creating, self.draft) {
            (true, true) => "Create draft",
            (true, false) => "Create pull request",
            (false, _) => "Save",
        };
        let empty_title = self.title.read(cx).text().trim().is_empty();
        modal(ModalSize::Medium, &theme)
            .id("pull-request")
            .child(
                v_flex()
                    .flex_none()
                    .gap(px(4.))
                    .px(px(20.))
                    .pt(px(18.))
                    .pb(px(12.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(heading),
                    )
                    .child(
                        div()
                            .font_family(theme.mono_font.clone())
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child(SharedString::from(route)),
                    ),
            )
            .child(
                v_flex()
                    .px(px(20.))
                    .pb(px(16.))
                    .gap(px(6.))
                    .child(label("Title"))
                    .child(field(&self.title, false))
                    .child(div().h(px(6.)))
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
                                .disabled(self.writing.is_some())
                                .tooltip("Write the title and description from the commits")
                                .on_click(cx.listener(|this, _, _, cx| this.write_with_ai(cx))),
                            ),
                    )
                    .child(field(&self.body, true))
                    .when(creating, |el| {
                        el.child(
                            h_flex()
                                .pt(px(8.))
                                .gap(px(10.))
                                .child(switch("pr-draft", self.draft, &theme).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.draft = !this.draft;
                                        cx.notify();
                                    }),
                                ))
                                .child(
                                    v_flex()
                                        .child(div().text_size(px(13.)).child("Draft"))
                                        .child(
                                            div()
                                                .text_size(px(12.))
                                                .text_color(theme.text_2)
                                                .child("Not ready for review yet"),
                                        ),
                                ),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_none()
                    .justify_end()
                    .gap(px(8.))
                    .px(px(20.))
                    .py(px(12.))
                    .border_t_1()
                    .border_color(theme.border_3)
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .text_color(theme.text_3)
                            .child("⌘↩ in the description to submit"),
                    )
                    .child(
                        button("pr-cancel", "Cancel")
                            .style(ButtonStyle::Ghost)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    )
                    .child(
                        button("pr-submit", confirm)
                            .style(ButtonStyle::Gray)
                            .disabled(empty_title || !self.loaded || self.writing.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                    ),
            )
    }
}

/// A pull request's number and state.
type Review = (u64, ReviewState);

impl AppView {
    /// The selected branch, or the branch of the selected commit, with its pull request
    /// number and state if it has one.
    fn review_target(&self, cx: &App) -> Option<(ProjectId, String, Option<Review>)> {
        let store = self.store.read(cx);
        let selection = store.selection.clone()?;
        let branch = store.branch_for(&selection)?;
        let review = branch.review.as_ref().map(|r| (r.number, r.state));
        Some((selection.project().clone(), branch.ref_name.clone(), review))
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
        let Some(ws) = (self.store.read(cx).workspace(&project)).filter(|ws| ws.forge.is_some())
        else {
            self.store.update(cx, |store, cx| {
                store.snack("This project's remote isn't on a forge GitButler knows", cx)
            });
            return;
        };
        let push = ws.find_branch(&branch).and_then(|b| match b.push {
            PushState::NothingToPush | PushState::Integrated => None,
            PushState::NeedsForce => Some(true),
            PushState::Unpushed | PushState::NeverPushed => Some(false),
        });
        let store = self.store.clone();
        let view =
            cx.new(|cx| ReviewView::new(store, project, branch, Mode::Create { push }, window, cx));
        self.open_modal(view, window, cx);
    }

    pub fn edit_pull_request(
        &mut self,
        _: &EditPullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((project, branch, Some((number, _)))) = self.review_target(cx) else {
            return;
        };
        let store = self.store.clone();
        let view =
            cx.new(|cx| ReviewView::new(store, project, branch, Mode::Edit { number }, window, cx));
        self.open_modal(view, window, cx);
    }

    /// Mark a draft ready for review, or an open pull request a draft again.
    pub fn toggle_draft(&mut self, _: &ToggleDraft, _: &mut Window, cx: &mut Context<Self>) {
        let Some((project, _, Some((number, state)))) = self.review_target(cx) else {
            return;
        };
        let draft = match state {
            ReviewState::Draft => false,
            ReviewState::Open => true,
            ReviewState::Merged | ReviewState::Closed => return,
        };
        self.run_on_review(
            project,
            "Change draft",
            move |path| reviews::set_draft(path, number, draft),
            if draft {
                "Converted to a draft"
            } else {
                "Marked ready for review"
            },
            cx,
        );
    }

    /// Have an open pull request merge once its checks pass, or stop it from doing so.
    pub fn toggle_auto_merge(
        &mut self,
        _: &ToggleAutoMerge,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((project, _, Some((number, ReviewState::Open)))) = self.review_target(cx) else {
            return;
        };
        let store = self.store.read(cx);
        let on = (store.selection.as_ref())
            .and_then(|selection| store.branch_for(selection)?.review.as_ref())
            .is_some_and(|review| review.auto_merge);
        self.run_on_review(
            project,
            "Auto-merge",
            move |path| reviews::set_auto_merge(path, number, !on),
            if on {
                "It won't merge on its own"
            } else {
                "Merges once its checks pass"
            },
            cx,
        );
    }

    /// Ask for a pull request's number, then fetch its branch, from a fork too, and apply
    /// it to the workspace to try it out.
    pub fn apply_pull_request(
        &mut self,
        _: &ApplyPullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(selection) = self.store.read(cx).selection.clone() else {
            return;
        };
        if !on_forge(self.store.read(cx), &selection) {
            return;
        }
        let project = selection.project().clone();
        let input = cx.new(|cx| TextInput::new("Pull request number", false, cx));
        self.open_prompt(
            "Apply a pull request",
            Some("Its branch is fetched, from a fork too, and added to the workspace.".into()),
            "Apply",
            input,
            None,
            window,
            cx,
            move |this, text, _, cx| {
                let Ok(number) = text.trim().trim_start_matches('#').parse::<u64>() else {
                    let message = "That isn't a pull request number";
                    return this.store.update(cx, |store, cx| store.snack(message, cx));
                };
                let selected = project.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        format!("Apply pull request #{number}"),
                        Overlay::default(),
                        move |path, _| branches::apply_review(path, number),
                        move |store, (applied, branch), cx| match applied {
                            ApplyOutcome::Applied => {
                                if let Some(branch) = branch {
                                    store.select(Some(Selection::Branch(selected, branch)), cx);
                                }
                                store.snack_undoable(format!("Applied #{number}"), cx)
                            }
                            ApplyOutcome::AlreadyApplied => store.snack(
                                format!(
                                    "#{number} is in the workspace already: pull its branch \
                                     for any new commits"
                                ),
                                cx,
                            ),
                            ApplyOutcome::Conflicts(stacks) => store.error(
                                format!("#{number} wasn't applied"),
                                format!("It conflicts with {}", stacks.join(", ")),
                                cx,
                            ),
                        },
                        cx,
                    )
                })
            },
        );
    }

    /// Pick a label of the repository to put on the pull request, or to take off one it has.
    pub fn label_pull_request(
        &mut self,
        _: &LabelPullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((project, branch, Some((number, ReviewState::Open | ReviewState::Draft)))) =
            self.review_target(cx)
        else {
            return;
        };
        let store = self.store.read(cx);
        let Some(path) = store.path(&project) else {
            return;
        };
        let applied: Vec<String> = (store.workspace(&project))
            .and_then(|ws| ws.find_branch(&branch)?.review.as_ref())
            .map(|review| review.labels.clone())
            .unwrap_or_default();
        let load = move || reviews::repo_labels(&path);
        self.pick_from_forge(
            load,
            "Couldn't list the labels",
            window,
            cx,
            move |this, labels, window, cx| {
                let choices = (labels.into_iter())
                    .map(|label| {
                        let on = !applied.contains(&label);
                        let shown = if on {
                            label.clone()
                        } else {
                            format!("✓ {label}")
                        };
                        let project = project.clone();
                        crate::palette::Choice::new(shown, move |this, _, cx| {
                            this.run_on_review(
                                project,
                                "Label",
                                move |path| reviews::set_label(path, number, label, on),
                                "Labels changed",
                                cx,
                            )
                        })
                    })
                    .collect();
                this.open_picker("Put on or take off a label", "Toggle", choices, window, cx);
            },
        );
    }

    /// Ask for a comment on the pull request as a whole, post it, then have the branch
    /// page ask again where the review stands.
    pub fn comment_on_pull_request(
        &mut self,
        _: &CommentOnPullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((project, branch, Some((number, _)))) = self.review_target(cx) else {
            return;
        };
        self.post_to_review(
            (project, branch),
            format!("Comment on #{number}"),
            ("Comment", "Commented"),
            move |path, body| reviews::comment(path, number, body),
            window,
            cx,
        );
    }

    /// Ask for an answer to a conversation on `ref_name`'s pull request, post it, then have
    /// the branch page ask again where the review stands.
    pub fn reply_to_thread(
        &mut self,
        project: ProjectId,
        ref_name: String,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.post_to_review(
            (project, ref_name),
            "Reply to the conversation".into(),
            ("Reply", "Replied"),
            move |path, body| reviews::reply_to_thread(path, id, body),
            window,
            cx,
        );
    }

    /// Ask for a line to `verb` with, then `post` it to the forge off the UI thread: on
    /// success say `done` and have the branch page ask the forge where the review stands.
    fn post_to_review(
        &mut self,
        (project, branch): (ProjectId, String),
        title: String,
        (verb, done): (&'static str, &'static str),
        post: impl FnOnce(&std::path::Path, String) -> anyhow::Result<()> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.store.read(cx).path(&project) else {
            return;
        };
        let input = cx.new(|cx| TextInput::new(verb, false, cx));
        self.open_prompt(
            &title,
            None,
            verb,
            input,
            None,
            window,
            cx,
            move |_, text, _, cx| {
                let text = text.trim().to_owned();
                if text.is_empty() {
                    return;
                }
                // Kept for the error, so Copy error gets it back.
                let sent = text.clone();
                cx.spawn(async move |this, cx| {
                    let posted = backend::run(move || post(&path, text)).await;
                    this.update(cx, |this, cx| match posted {
                        Ok(()) => {
                            this.store.update(cx, |store, cx| store.snack(done, cx));
                            this.detail.update(cx, |detail, cx| {
                                detail.load_feedback(&project, &branch, cx)
                            })
                        }
                        Err(err) => this.store.update(cx, |store, cx| {
                            let title = format!("Couldn't {}", verb.to_lowercase());
                            store.error(title, format!("{err:#}\n\nWhat you wrote: {sent}"), cx)
                        }),
                    })
                    .ok();
                })
                .detach();
            },
        );
    }

    /// `load` a list from the forge off the UI thread, then `open` a picker of it, or say
    /// what went wrong under `failed`.
    fn pick_from_forge<T: Send + 'static>(
        &mut self,
        load: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
        failed: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
        open: impl FnOnce(&mut AppView, T, &mut Window, &mut Context<AppView>) + 'static,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            let loaded = backend::run(load).await;
            this.update_in(cx, |this, window, cx| match loaded {
                Ok(list) => open(this, list, window, cx),
                Err(err) => {
                    let message = format!("{err:#}");
                    this.store
                        .update(cx, |store, cx| store.error(failed, message, cx))
                }
            })
            .ok();
        })
        .detach();
    }

    /// Pick someone of the repository's to ask for a review of the pull request, or, ticked,
    /// to ask no longer. Its author isn't offered: the forge won't ask them.
    pub fn request_review(
        &mut self,
        _: &RequestReview,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((project, branch, Some((number, ReviewState::Open | ReviewState::Draft)))) =
            self.review_target(cx)
        else {
            return;
        };
        let store = self.store.read(cx);
        let Some(path) = store.path(&project) else {
            return;
        };
        let review =
            (store.workspace(&project)).and_then(|ws| ws.find_branch(&branch)?.review.as_ref());
        let asked = review.map_or_else(Vec::new, |review| review.reviewers.clone());
        let author = review.and_then(|review| review.author.clone());
        let load = move || reviews::reviewer_candidates(&path);
        self.pick_from_forge(
            load,
            "Couldn't list who can review",
            window,
            cx,
            move |this, logins, window, cx| {
                let choices = (logins.into_iter())
                    .filter(|login| author.as_ref() != Some(login))
                    .map(|login| {
                        let ask = !asked.contains(&login);
                        let shown = if ask {
                            login.clone()
                        } else {
                            format!("✓ {login}")
                        };
                        let project = project.clone();
                        crate::palette::Choice::new(shown, move |this, _, cx| {
                            this.run_on_review(
                                project,
                                "Request review",
                                move |path| reviews::request_review(path, number, login, ask),
                                "Reviewers changed",
                                cx,
                            )
                        })
                    })
                    .collect();
                this.open_picker(
                    "Ask someone to review, or no longer",
                    "Toggle",
                    choices,
                    window,
                    cx,
                );
            },
        );
    }

    /// Merge the pull request, asking how.
    pub fn merge_pull_request(
        &mut self,
        _: &MergePullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((project, branch, Some((number, ReviewState::Open)))) = self.review_target(cx)
        else {
            return;
        };
        // GitLab merges as the project is set up to, whatever is asked for.
        let gitlab = (self.store.read(cx).workspace(&project)).and_then(|ws| ws.forge.as_deref())
            == Some("GitLab");
        let methods: &[&str] = if gitlab {
            &["Merge", "Cancel"]
        } else {
            &["Merge", "Squash and merge", "Rebase and merge", "Cancel"]
        };
        let answer = window.prompt(
            PromptLevel::Info,
            &format!("Merge #{number}, {}?", short_ref(&branch)),
            Some("Its commits go into the branch it targets on the forge."),
            methods,
            cx,
        );
        cx.spawn(async move |this, cx| {
            let method = match (answer.await, gitlab) {
                (Ok(0), _) => MergeMethod::Merge,
                (Ok(1), false) => MergeMethod::Squash,
                (Ok(2), false) => MergeMethod::Rebase,
                _ => return,
            };
            this.update(cx, |this, cx| {
                this.run_on_review(
                    project,
                    "Merge pull request",
                    move |path| reviews::merge(path, number, method),
                    "Merged. Update to bring it into your workspace",
                    cx,
                )
            })
            .ok();
        })
        .detach();
    }

    /// Close an open pull request after asking, or reopen a closed one.
    pub fn close_pull_request(
        &mut self,
        _: &ClosePullRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((project, _, Some((number, state)))) = self.review_target(cx) else {
            return;
        };
        match state {
            ReviewState::Closed => self.run_on_review(
                project,
                "Reopen pull request",
                move |path| reviews::update(path, number, None, Some(true)),
                "Reopened",
                cx,
            ),
            ReviewState::Open | ReviewState::Draft => self.confirm_then(
                format!("Close pull request #{number}?"),
                "The branch stays as it is. You can reopen it later.",
                "Close",
                window,
                cx,
                move |this, cx| {
                    this.run_on_review(
                        project,
                        "Close pull request",
                        move |path| reviews::update(path, number, None, Some(false)),
                        "Closed",
                        cx,
                    )
                },
            ),
            ReviewState::Merged => {}
        }
    }

    fn run_on_review(
        &mut self,
        project: ProjectId,
        label: &'static str,
        op: impl FnOnce(&std::path::Path) -> anyhow::Result<()> + Send + 'static,
        done: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.store.update(cx, |store, cx| {
            let refreshed = project.clone();
            store.run(
                &project,
                label,
                Overlay::default(),
                move |path, _| op(path),
                move |store, (), cx| {
                    store.snack(done, cx);
                    store.refresh_forge(&refreshed);
                },
                cx,
            )
        });
    }
}

/// Whether `selection` is a branch of a project on a forge, so pull requests apply.
pub fn on_forge(store: &Store, selection: &Selection) -> bool {
    store
        .workspace(selection.project())
        .is_some_and(|ws| ws.forge.is_some())
}
