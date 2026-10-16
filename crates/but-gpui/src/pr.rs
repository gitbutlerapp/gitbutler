//! A branch's pull request: opening one (pushing the branch first when it needs it), and
//! for one that exists, editing it, marking it ready or a draft, merging, closing and
//! reopening it.

use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, PromptLevel,
    SharedString, Task, Window, actions, div, prelude::*, px,
};

use crate::app::AppView;
use crate::backend::ai;
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
    /// The title and description being written with AI: the task streaming them into
    /// the fields, and the one asking for them.
    writing: Option<(Task<()>, Task<()>)>,
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
        let (sender, mut tokens) = futures::channel::mpsc::unbounded::<String>();
        let fields = (self.title.clone(), self.body.clone());
        let pump = cx.spawn(async move |_, cx| {
            let mut text = String::new();
            while let Some(token) = futures::StreamExt::next(&mut tokens).await {
                text.push_str(&token);
                let (title, body) = ai::split_title_body(&text);
                fields.0.update(cx, |input, cx| input.set_text(title, cx));
                fields.1.update(cx, |input, cx| input.set_text(body, cx));
            }
        });
        let write = cx.spawn(async move |this, cx| {
            let written = backend::run(move || {
                ai::stream(ai::PR_SYSTEM, prompt, move |token| {
                    sender.unbounded_send(token.to_owned()).ok();
                })
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
        self.writing = Some((pump, write));
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
        if !self
            .store
            .read(cx)
            .workspace(&project)
            .is_some_and(|ws| ws.forge.is_some())
        {
            self.store.update(cx, |store, cx| {
                store.snack("This project's remote isn't on a forge GitButler knows", cx)
            });
            return;
        }
        let push = self
            .store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_branch(&branch))
            .map(|b| b.push)
            .and_then(|push| match push {
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
