//! A branch's pull request: opening one (pushing the branch first when it needs it), and
//! merging one that's open.

use gpui::{
    App, Context, DismissEvent, Div, Entity, EventEmitter, FocusHandle, Focusable, PromptLevel,
    SharedString, Stateful, Task, Window, actions, div, prelude::*, px,
};

use crate::app::AppView;
use crate::backend::ai;
use crate::backend::reviews::{self, MergeMethod};
use crate::backend::{self, Forge, Review, ReviewState, Workspace, short_ref};
use crate::state::{Overlay, ProjectId, Selection, Store, StoreEvent};
use crate::theme::Theme;
use crate::ui::modal::{ModalSize, modal};
use crate::ui::text_input::{InputEvent, Newline, TextInput};
use crate::ui::{ButtonStyle, button, faint, h_flex, muted, switch, v_flex};

actions!(pr, [CreatePullRequest, MergePullRequest]);

pub struct ReviewView {
    store: Entity<Store>,
    project: ProjectId,
    /// The branch, by full ref.
    branch: String,
    title: Entity<TextInput>,
    body: Entity<TextInput>,
    /// The branch it would merge into, once known.
    target: Option<String>,
    draft: bool,
    /// The title and description being written with AI, streaming into the fields.
    writing: Option<Task<()>>,
    /// The pull request is being created; the form closes once it is.
    submitting: bool,
    /// Why creating it failed, with the form left as it was to try again.
    error: Option<String>,
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
        // The store can turn the operation down before it starts, as while `but resolve`
        // edits a commit, and then nothing reports back: the form is free again.
        cx.subscribe(&store, |this, store, event, cx| {
            if let StoreEvent::Pending(project) = event
                && this.submitting
                && *project == this.project
                && !store.read(cx).publishing(&this.project, &this.branch)
            {
                this.submitting = false;
                cx.notify();
            }
        })
        .detach();
        let path = store.read(cx).path(&project);
        // A branch of one commit starts from that commit's message, as `but pr` does.
        let (subject, description) = store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_branch(&branch))
            .filter(|b| b.commits.len() == 1)
            .map(|b| {
                let message = &b.commits[0].message;
                let (subject, rest) = message.split_once('\n').unwrap_or((message, ""));
                (subject.trim().to_owned(), rest.trim().to_owned())
            })
            .unwrap_or_else(|| (short_ref(&branch).to_owned(), String::new()));
        title.update(cx, |input, cx| {
            input.set_text(subject, cx);
            input.select_all_text(cx);
        });
        body.update(cx, |input, cx| input.set_text(description.clone(), cx));
        let ref_name = branch.clone();
        if let Some(path) = path {
            cx.spawn(async move |this, cx| {
                let found = backend::run(move || reviews::new_review(&path, &ref_name)).await;
                this.update(cx, |this, cx| match found {
                    Ok(new) => {
                        this.target = Some(new.target);
                        if let Some(template) = new.template.filter(|_| description.is_empty()) {
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
        Self {
            store,
            project,
            branch,
            title,
            body,
            target: None,
            draft: false,
            writing: None,
            submitting: false,
            error: None,
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
        if title.is_empty() || self.writing.is_some() || self.submitting {
            return;
        }
        let body = self.body.read(cx).text().trim().to_owned();
        let (branch, draft) = (self.branch.clone(), self.draft);
        let project = self.project.clone();
        let refreshed = project.clone();
        let view = cx.entity().downgrade();
        self.submitting = true;
        self.error = None;
        cx.notify();
        self.store.update(cx, |store, cx| {
            let run_hooks = store.runs_hooks(&project);
            store.run(
                &project,
                "Create pull request",
                Overlay::push(&branch, true),
                // A failure goes to the form, which keeps what was typed.
                move |path, _| {
                    let published = reviews::publish(path, &branch, title, body, draft, run_hooks);
                    Ok(
                        published.map_err(|err| match backend::push_refusal(&err).as_deref() {
                            None => format!("{err:#}"),
                            Some("") => "Someone pushed to the branch since the last fetch, and \
                                     pushing would drop their commits. Fetch to see them."
                                .to_owned(),
                            Some(said) => format!("The remote refused the push: {said}"),
                        }),
                    )
                },
                move |store, published, cx| {
                    // The push can move the stack's other pull requests too.
                    store.refresh_forge(&refreshed);
                    match published {
                        Ok(published) => {
                            store.snack(format!("Created pull request #{}", published.number), cx);
                            if let Some(message) = published.sync_failed {
                                let title = "The stack's other pull requests weren't updated";
                                store.error(title, message, cx);
                            }
                            view.update(cx, |_, cx| cx.emit(DismissEvent)).ok();
                        }
                        Err(failed) => {
                            let shown = view.update(cx, |this, cx| {
                                this.submitting = false;
                                this.error = Some(failed.clone());
                                cx.notify();
                            });
                            if shown.is_err() {
                                store.error("Create pull request failed", failed, cx);
                            }
                        }
                    }
                },
                cx,
            )
        });
    }
}

impl Render for ReviewView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let route = match &self.target {
            Some(target) => format!("{} → {target}", short_ref(&self.branch)),
            None => short_ref(&self.branch).to_owned(),
        };
        let push = (self.store.read(cx).workspace(&self.project))
            .and_then(|ws| push_first(ws, &self.branch))
            .map(|force| {
                if force {
                    "Gets force-pushed first, since it or a branch below it was rewritten"
                } else {
                    "Gets pushed first"
                }
            });
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
                    .gap(px(4.))
                    .px(px(20.))
                    .pt(px(18.))
                    .pb(px(12.))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Create a pull request"),
                    )
                    .child(
                        div()
                            .font_family(theme.mono_font.clone())
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child(SharedString::from(route)),
                    )
                    .children(push.map(|note| muted(note, &theme))),
            )
            .child(
                v_flex()
                    .px(px(20.))
                    .pb(px(16.))
                    .gap(px(6.))
                    .child(label("Title"))
                    .child(field(&self.title, false).capture_action(cx.listener(
                        |this, _: &Newline, window, cx| {
                            window.focus(&this.body.focus_handle(cx), cx);
                            cx.stop_propagation();
                        },
                    )))
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
                                .disabled_because(self.writing.is_some().then_some("Writing…"))
                                .tooltip("Write the title and description from the commits")
                                .on_click(cx.listener(|this, _, _, cx| this.write_with_ai(cx))),
                            ),
                    )
                    .child(field(&self.body, true))
                    .child(
                        h_flex()
                            .pt(px(8.))
                            .gap(px(10.))
                            .child(switch("pr-draft", self.draft, &theme).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.draft = !this.draft;
                                    cx.notify();
                                },
                            )))
                            .child(
                                v_flex()
                                    .child(div().text_size(px(13.)).child("Draft"))
                                    .child(muted("Not ready for review yet", &theme)),
                            ),
                    )
                    .children(self.error.clone().map(|err| {
                        div()
                            .pt(px(4.))
                            .text_size(px(12.))
                            .text_color(theme.danger)
                            .child(err)
                    })),
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
                    .child(faint("⌘↩ to submit", &theme).flex_1())
                    .child(
                        button("pr-cancel", "Cancel")
                            .style(ButtonStyle::Ghost)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    )
                    .child(
                        button(
                            "pr-submit",
                            if self.submitting {
                                "Creating…"
                            } else {
                                confirm
                            },
                        )
                        .style(ButtonStyle::Gray)
                        .disabled_because(if self.submitting {
                            Some("Creating the pull request…")
                        } else if self.writing.is_some() {
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
        let Some(branch) = (store.selection.as_ref())
            .filter(|s| on_forge(store, s))
            .and_then(|s| store.branch_for(s))
            .filter(|b| !b.ref_name.is_empty())
        else {
            return el;
        };
        match branch.review.as_ref().map(|r| r.state) {
            None => el.when(!branch.commits.is_empty(), |el| {
                el.on_action(cx.listener(Self::create_pull_request))
            }),
            Some(ReviewState::Open) => el.on_action(cx.listener(Self::merge_pull_request)),
            Some(ReviewState::Draft | ReviewState::Closed | ReviewState::Merged) => el,
        }
    }

    /// The selected branch, or the branch of the selected commit, with its pull request if
    /// it has one.
    fn review_target(&self, cx: &App) -> Option<(ProjectId, String, Option<Review>)> {
        let store = self.store.read(cx);
        let selection = store.selection.clone()?;
        let branch = store.branch_for(&selection)?;
        let review = branch.review.clone();
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
        let store = self.store.clone();
        let view = cx.new(|cx| ReviewView::new(store, project, branch, window, cx));
        self.open_modal(view, window, cx);
    }

    /// Merge the pull request, asking how.
    pub fn merge_pull_request(
        &mut self,
        _: &MergePullRequest,
        window: &mut Window,
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
        // GitLab merges as the project is set up to, whatever is asked for.
        let gitlab = (self.store.read(cx).workspace(&project)).and_then(|ws| ws.forge)
            == Some(Forge::GitLab);
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
                let refreshed = project.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Merge pull request",
                        Overlay::remote(),
                        move |path, _| reviews::merge(path, number, method),
                        move |store, (), cx| {
                            store.snack("Merged. Update to bring it into your workspace", cx);
                            store.refresh_forge(&refreshed);
                        },
                        cx,
                    )
                })
            })
            .ok();
        })
        .detach();
    }
}

/// Whether creating `branch`'s pull request pushes first, and whether forced: it or a branch
/// below it has something to push.
fn push_first(ws: &Workspace, branch: &str) -> Option<bool> {
    ws.can_push(branch).then(|| ws.needs_force(branch))
}

/// Whether `selection`'s project is on a forge, so pull requests apply.
pub fn on_forge(store: &Store, selection: &Selection) -> bool {
    store
        .workspace(selection.project())
        .is_some_and(|ws| ws.forge.is_some())
}
