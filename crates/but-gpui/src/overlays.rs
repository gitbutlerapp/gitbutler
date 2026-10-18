//! What opens above the window: right-click menus, one-line prompts, modals, and the
//! snackbars and error cards.

use std::path::Path;
use std::rc::Rc;

use anyhow::Result;
use gpui::{
    Action, AnyElement, App, ClickEvent, Context, DismissEvent, Entity, Focusable as _,
    ManagedView, MouseButton, Pixels, Point, Role, SharedString, Subscription, Window, anchored,
    deferred, div, prelude::*, px, relative,
};

use crate::app::AppView;
use crate::backend::{self, short_ref};
use crate::state::{Notice, Overlay, ProjectId, Remap, Store};
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, Icon, STATUS_BAR_HEIGHT, button, error_summary, h_flex, icon, keycap, list_row,
    muted, popover, shortcut, small_icon, v_flex,
};

/// A right-click menu.
pub struct ContextMenu {
    pub position: Point<Pixels>,
    pub items: Vec<MenuItem>,
}

type MenuHandler = Rc<dyn Fn(&mut AppView, &mut Window, &mut Context<AppView>)>;
type SubmitHandler = Box<dyn FnOnce(&mut AppView, String, &mut Window, &mut Context<AppView>)>;
type CancelHandler = Box<dyn FnOnce(&mut AppView)>;
type CheckHandler = Box<dyn Fn(&AppView, &str, &App) -> Option<SharedString>>;

#[derive(Clone)]
pub struct MenuItem {
    pub label: SharedString,
    /// The keys that do the same, shown at the end of the row.
    pub key: Option<SharedString>,
    pub danger: bool,
    /// Starts a new group, with a line above it.
    pub separated: bool,
    /// Starts a new group under this heading, as a choice of one does: "Sort by".
    pub heading: Option<SharedString>,
    /// The choice its group stands at, which carries a tick.
    pub ticked: bool,
    pub handler: MenuHandler,
}

impl MenuItem {
    pub fn new(
        label: impl Into<SharedString>,
        handler: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            key: None,
            danger: false,
            separated: false,
            heading: None,
            ticked: false,
            handler: Rc::new(handler),
        }
    }

    /// An item that dispatches `action` where focus is, showing the keys bound to it.
    pub fn action(label: impl Into<SharedString>, action: &dyn Action, cx: &App) -> Self {
        let key = shortcut(action, cx);
        let action = action.boxed_clone();
        Self {
            key,
            ..Self::new(label, move |_, window, cx| {
                window.dispatch_action(action.boxed_clone(), cx)
            })
        }
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn separated(mut self) -> Self {
        self.separated = true;
        self
    }

    pub fn under(mut self, heading: impl Into<SharedString>) -> Self {
        self.heading = Some(heading.into());
        self.separated = true;
        self
    }

    pub fn ticked(mut self, ticked: bool) -> Self {
        self.ticked = ticked;
        self
    }
}

/// The short name a branch named `name` gets, as the backend normalizes it, or why it
/// can't have that name.
fn branch_name(name: &str) -> Result<String, SharedString> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a name for the branch".into());
    }
    but_core::branch::normalize_short_name(name)
        .map(|short| short.to_string())
        .map_err(|_| "Git doesn't allow that as a branch name".into())
}

/// A small modal asking for one line of text: a branch name, a repository to clone, a pull
/// request number, a comment, or credentials for Git.
pub struct Prompt {
    title: SharedString,
    detail: Option<SharedString>,
    confirm: SharedString,
    input: Entity<TextInput>,
    on_submit: SubmitHandler,
    on_cancel: Option<CancelHandler>,
    /// Why the text can't be submitted as it is, which keeps the prompt open.
    check: Option<CheckHandler>,
    /// What `check` said of the text last submitted, until it's edited.
    hint: Option<SharedString>,
    _subscription: Subscription,
}

impl AppView {
    pub fn open_menu(
        &mut self,
        position: Point<Pixels>,
        items: Vec<MenuItem>,
        cx: &mut Context<Self>,
    ) {
        self.menu = Some(ContextMenu { position, items });
        cx.notify();
    }

    /// Ask for a branch name, then run `op` with it and hand `done` the full ref name of the
    /// branch it returns. With an `initial` name the branch is being renamed; without one,
    /// created.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prompt_branch(
        &mut self,
        project: ProjectId,
        title: &str,
        initial: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        op: impl FnOnce(&Path, &Remap, &str) -> Result<String> + Send + 'static,
        done: impl FnOnce(&mut Store, String, &mut Context<Store>) + 'static,
    ) {
        let confirm = if initial.is_empty() {
            "Create"
        } else {
            "Rename"
        };
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Branch name", false, cx);
            input.set_text(initial, cx);
            input.select_all_text(cx);
            input
        });
        let label = format!("{confirm} branch");
        let (current, checked) = (initial.to_owned(), project.clone());
        let check = move |this: &AppView, name: &str, cx: &App| {
            let short = match branch_name(name) {
                Ok(short) => short,
                Err(hint) => return Some(hint),
            };
            let taken = short != current
                && (this.store.read(cx).workspace(&checked))
                    .is_some_and(|ws| ws.branches().any(|b| short_ref(&b.ref_name) == short));
            taken.then(|| format!("{short} already exists").into())
        };
        let current = initial.to_owned();
        self.open_prompt(
            title,
            None,
            confirm,
            input,
            None,
            Some(Box::new(check)),
            window,
            cx,
            move |this, name, _, cx| {
                // Renaming to the name it has does nothing.
                if branch_name(&name).is_ok_and(|short| short == current) {
                    return;
                }
                let name = name.trim().to_owned();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        label,
                        Overlay::default(),
                        move |path, remap| op(path, remap, &name),
                        done,
                        cx,
                    )
                });
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn open_prompt(
        &mut self,
        title: &str,
        detail: Option<String>,
        confirm: &str,
        input: Entity<TextInput>,
        on_cancel: Option<CancelHandler>,
        check: Option<CheckHandler>,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_submit: impl FnOnce(&mut AppView, String, &mut Window, &mut Context<AppView>) + 'static,
    ) {
        // A prompt Git is waiting on, for credentials, stays until answered: replacing it
        // would fail the push that asked. A second one is cancelled, so its Git process
        // fails rather than waits forever.
        if self
            .prompt
            .as_ref()
            .is_some_and(|prompt| prompt.on_cancel.is_some())
        {
            if let Some(on_cancel) = on_cancel {
                on_cancel(self);
            }
            return;
        }
        let subscription =
            cx.subscribe_in(
                &input,
                window,
                |this, input, event, window, cx| match event {
                    InputEvent::Submit => {
                        let text = input.read(cx).text().to_owned();
                        let hint = (this.prompt.as_ref())
                            .and_then(|prompt| prompt.check.as_ref())
                            .and_then(|check| check(this, &text, cx));
                        if let Some(prompt) = this.prompt.as_mut().filter(|_| hint.is_some()) {
                            prompt.hint = hint;
                            return cx.notify();
                        }
                        if let Some(prompt) = this.prompt.take() {
                            (prompt.on_submit)(this, text, window, cx);
                        }
                        this.focus_main(window, cx);
                        cx.notify();
                    }
                    InputEvent::Cancel => {
                        this.cancel_prompt(window, cx);
                    }
                    InputEvent::Changed => {
                        if let Some(prompt) = &mut this.prompt
                            && prompt.hint.take().is_some()
                        {
                            cx.notify();
                        }
                    }
                },
            );
        window.focus(&input.focus_handle(cx), cx);
        self.prompt = Some(Prompt {
            title: title.to_owned().into(),
            detail: detail.map(Into::into),
            confirm: confirm.to_owned().into(),
            input,
            on_submit: Box::new(on_submit),
            on_cancel,
            check,
            hint: None,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn cancel_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(prompt) = self.prompt.take()
            && let Some(on_cancel) = prompt.on_cancel
        {
            on_cancel(self);
        }
        self.focus_main(window, cx);
        cx.notify();
    }

    /// Git asks for passphrases and passwords through GitButler; show those as a prompt.
    pub(crate) fn listen_for_credential_prompts(window: &mut Window, cx: &mut Context<Self>) {
        let (sender, mut prompts) = futures::channel::mpsc::unbounded();
        backend::askpass::init(move |prompt| {
            sender.unbounded_send(prompt).ok();
        });
        cx.spawn_in(window, async move |this, cx| {
            use futures::StreamExt as _;
            while let Some(prompt) = prompts.next().await {
                let id = prompt.id;
                let result = this.update_in(cx, |this, window, cx| {
                    let input =
                        cx.new(|cx| TextInput::new("Passphrase or password", false, cx).masked());
                    this.open_prompt(
                        "Git needs your credentials",
                        Some(prompt.text),
                        "Continue",
                        input,
                        Some(Box::new(move |_| backend::askpass::respond(id, None))),
                        None,
                        window,
                        cx,
                        move |_, secret, _, _| backend::askpass::respond(id, Some(secret)),
                    );
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Show `view` over the window until it emits `DismissEvent` or the backdrop is clicked.
    pub fn open_modal<V: ManagedView>(
        &mut self,
        view: Entity<V>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let subscription =
            cx.subscribe_in(&view, window, |this, _, _: &DismissEvent, window, cx| {
                this.close_modal(window, cx)
            });
        window.focus(&view.focus_handle(cx), cx);
        self.modal = Some((view.into(), subscription));
        cx.notify();
    }

    /// The modal on screen, when it's a `V`.
    pub fn modal_view<V: 'static>(&self) -> Option<Entity<V>> {
        self.modal.as_ref()?.0.clone().downcast::<V>().ok()
    }

    pub(crate) fn close_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal.take().is_some() {
            self.focus_main(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn render_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (view, _) = self.modal.as_ref()?;
        Some(
            div()
                .id("modal-backdrop")
                .absolute()
                .inset_0()
                .bg(gpui::black().opacity(0.25))
                .flex()
                .justify_center()
                .items_center()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_modal(window, cx)),
                )
                .child(
                    div()
                        .flex()
                        .max_h(relative(0.88))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(view.clone()),
                )
                .into_any_element(),
        )
    }

    pub(crate) fn render_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.menu.as_ref()?;
        let theme = Theme::get(cx);
        let items = menu.items.iter().enumerate().flat_map(|(ix, item)| {
            let handler = item.handler.clone();
            let line = (item.separated && ix > 0).then(|| {
                div()
                    .h(px(1.))
                    .mx(px(10.))
                    .my(px(4.))
                    .bg(theme.border_3)
                    .into_any_element()
            });
            // As ui-react's menu sections head theirs.
            let heading = item.heading.clone().map(|heading| {
                div()
                    .px(px(14.))
                    .pt(px(4.))
                    .pb(px(2.))
                    .text_size(px(12.))
                    .text_color(theme.text_3)
                    .child(heading)
                    .into_any_element()
            });
            let row = list_row(("menu-item", ix), false, theme)
                .mx(px(4.))
                .gap(px(16.))
                .text_size(px(13.))
                .text_color(if item.danger {
                    theme.danger
                } else {
                    theme.text_1
                })
                .child(div().flex_1().child(item.label.clone()))
                .when_some(item.key.clone(), |el, key| el.child(keycap(&key, theme)))
                .when(item.ticked, |el| {
                    el.child(small_icon(Icon::Tick).text_color(theme.text_1))
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.menu = None;
                    handler(this, window, cx);
                    cx.notify();
                }))
                .into_any_element();
            line.into_iter().chain(heading).chain([row])
        });
        Some(
            deferred(
                // Over the window, so what's under the menu neither hovers nor shows a
                // tooltip, and a click outside only closes it, as a native menu does.
                div().absolute().inset_0().occlude().child(
                    anchored()
                        .position(menu.position)
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            popover(theme)
                                .id("context-menu")
                                .min_w(px(180.))
                                .py(px(4.))
                                .children(items)
                                .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                    this.menu = None;
                                    cx.notify();
                                })),
                        ),
                ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    pub(crate) fn render_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let prompt = self.prompt.as_ref()?;
        let theme = Theme::get(cx);
        let input = prompt.input.clone();
        Some(
            div()
                .id("prompt-backdrop")
                .absolute()
                .inset_0()
                .bg(gpui::black().opacity(0.2))
                .flex()
                .justify_center()
                .items_start()
                .pt(px(120.))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.cancel_prompt(window, cx);
                    }),
                )
                .child(
                    popover(theme)
                        .id("prompt")
                        .w(px(380.))
                        .h_auto()
                        .p(px(16.))
                        .gap(px(12.))
                        .rounded(px(10.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(prompt.title.clone()),
                        )
                        .when_some(prompt.detail.clone(), |el, detail| {
                            el.child(muted(detail, theme))
                        })
                        .child(
                            div()
                                .px(px(8.))
                                .py(px(6.))
                                .rounded(px(4.))
                                .border_1()
                                .border_color(theme.pop)
                                .font_family(theme.mono_font.clone())
                                .text_size(px(13.))
                                .child(prompt.input.clone()),
                        )
                        .when_some(prompt.hint.clone(), |el, hint| {
                            el.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.danger)
                                    .child(hint),
                            )
                        })
                        .child(
                            h_flex()
                                .justify_end()
                                .gap(px(8.))
                                .child(
                                    button("prompt-cancel", "Cancel")
                                        .style(ButtonStyle::Ghost)
                                        .on_click({
                                            let input = input.clone();
                                            move |_, window, cx| {
                                                input.update(cx, |_, cx| {
                                                    cx.emit(InputEvent::Cancel)
                                                });
                                                window.refresh();
                                            }
                                        }),
                                )
                                .child(
                                    button("prompt-confirm", prompt.confirm.clone())
                                        .style(ButtonStyle::Gray)
                                        .on_click(move |_, _, cx| {
                                            input.update(cx, |_, cx| cx.emit(InputEvent::Submit))
                                        }),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    /// Snackbars at the bottom centre, error cards at the bottom right.
    pub(crate) fn render_toasts(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let store = self.store.read(cx);
        let mut errors = Vec::new();
        let mut snackbar = None;
        for toast in &store.toasts {
            let id = toast.id;
            let dismiss = cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.store
                    .update(cx, |store, cx| store.dismiss_toast(id, cx))
            });
            match &toast.notice {
                Notice::Snackbar { message, undo } => {
                    snackbar = Some(
                        h_flex()
                            .id(("snackbar", id))
                            .role(Role::Alert)
                            .aria_label(message.clone())
                            .h(px(32.))
                            .pl(px(12.))
                            .pr(px(if undo.is_some() { 4. } else { 12. }))
                            .gap(px(8.))
                            .rounded(px(8.))
                            .bg(theme.snackbar_bg)
                            .text_color(theme.snackbar_text)
                            .text_size(px(12.))
                            .shadow_lg()
                            .child(message.clone())
                            .when_some(undo.clone(), |el, (project, snapshot)| {
                                el.child(
                                    div()
                                        .w(px(1.))
                                        .h(px(16.))
                                        .bg(theme.snackbar_text.opacity(0.2)),
                                )
                                .child(
                                    h_flex()
                                        .id(("undo", id))
                                        .h(px(24.))
                                        .px(px(8.))
                                        .rounded(px(5.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .hover(|el| el.bg(theme.snackbar_text.opacity(0.12)))
                                        .child("Undo")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.store.update(cx, |store, cx| {
                                                store.dismiss_toast(id, cx);
                                                store.undo_snapshot(&project, snapshot, cx);
                                            })
                                        })),
                                )
                            })
                            .on_click(dismiss),
                    );
                }
                Notice::Error {
                    title,
                    detail,
                    retry,
                } => {
                    let copied = detail.to_string();
                    let retry = retry.clone().map(|(label, run)| {
                        button(("retry", id), label).small().on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                this.store.update(cx, |store, cx| {
                                    store.dismiss_toast(id, cx);
                                    run(store, cx)
                                })
                            },
                        ))
                    });
                    errors.push(
                        popover(theme)
                            .id(("error", id))
                            .role(Role::Alert)
                            .aria_label(title.clone())
                            .aria_description(detail.clone())
                            .w(px(320.))
                            .p(px(14.))
                            .gap(px(8.))
                            .child(
                                h_flex()
                                    .items_start()
                                    .gap(px(8.))
                                    .child(icon(Icon::Warning).text_color(theme.danger).mt(px(2.)))
                                    .child(
                                        // A long title wraps rather than running off the card.
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(title.clone()),
                                    ),
                            )
                            .child(muted(error_summary(detail), theme).line_clamp(4))
                            .child(
                                h_flex()
                                    .gap(px(6.))
                                    .justify_end()
                                    // At most one action beside Dismiss: a card with a way
                                    // forward is advice, with no raw error to copy.
                                    .when(retry.is_none(), |el| {
                                        el.child(
                                            button(("copy-error", id), "Copy error message")
                                                .small()
                                                .style(ButtonStyle::Ghost)
                                                .on_click(move |_, _, cx| {
                                                    cx.write_to_clipboard(
                                                        gpui::ClipboardItem::new_string(
                                                            copied.clone(),
                                                        ),
                                                    )
                                                }),
                                        )
                                    })
                                    .child(
                                        button(("dismiss", id), "Dismiss")
                                            .small()
                                            .on_click(dismiss),
                                    )
                                    .children(retry),
                            ),
                    );
                }
            }
        }
        div()
            .absolute()
            .inset_0()
            .child(
                v_flex()
                    .absolute()
                    .bottom(px(STATUS_BAR_HEIGHT + 12.))
                    .right(px(16.))
                    .gap(px(8.))
                    .children(errors),
            )
            .children(snackbar.map(|snackbar| {
                // Lifted above the commit footer's buttons, as the design system's hosts do.
                let footer = match self.detail.read(cx).shows_footer(cx) {
                    true => crate::detail::FOOTER_HEIGHT,
                    false => 0.,
                };
                h_flex()
                    .absolute()
                    .bottom(px(STATUS_BAR_HEIGHT + footer + 16.))
                    .left_0()
                    .right_0()
                    .justify_center()
                    .child(snackbar)
            }))
    }
}
