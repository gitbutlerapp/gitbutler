//! What opens above the window: right-click menus, one-line prompts, modals, and the
//! snackbars and error cards.

use std::path::Path;
use std::rc::Rc;

use anyhow::Result;
use gpui::{
    Action, AnyElement, App, ClickEvent, Context, DismissEvent, Div, Entity, FocusHandle,
    Focusable as _, ManagedView, MouseButton, Pixels, Point, Role, SharedString, Stateful,
    Subscription, Window, anchored, deferred, div, prelude::*, relative,
};

use crate::app::{AppView, Cancel, FocusNextPane, FocusPreviousPane};
use crate::backend::{self, FullName, short_ref};
use crate::help::OpenReleaseNotes;
use crate::state::{Notice, Overlay, ProjectId, Remap, Selection, Store};
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, FocusedAt, Icon, STATUS_BAR_HEIGHT, ShowFocused, button, dp, error_summary, field,
    fixed, h_flex, icon, keycap, list_row, muted, popover, shortcut, small_icon, v_flex,
};

/// A right-click menu.
pub struct ContextMenu {
    pub position: Point<Pixels>,
    pub items: Vec<MenuItem>,
    /// What it opened on: see [`Store::acting_on`].
    pub acting_on: (Option<Selection>, Vec<backend::CommitId>),
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
    /// A word on it, quieter, before the keys: how many it would show, say.
    pub detail: Option<SharedString>,
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
            detail: None,
            danger: false,
            separated: false,
            heading: None,
            ticked: false,
            handler: Rc::new(handler),
        }
    }

    /// An item that runs `handler` on what it opened on, showing the keys of `action`, which
    /// does the same from where the keys are: a right-click menu says what its items' keys are.
    pub fn keyed(
        label: impl Into<SharedString>,
        action: &dyn Action,
        cx: &App,
        handler: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
    ) -> Self {
        Self {
            key: shortcut(action, cx),
            ..Self::new(label, handler)
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

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// The short name a branch of `project` named `name` gets, as the backend normalizes it, or
/// why it can't have that name. `current` is the short name it has now, empty for a new one.
pub(crate) fn branch_name(
    store: &Store,
    project: &ProjectId,
    name: &str,
    current: &str,
) -> Result<String, SharedString> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a name for the branch".into());
    }
    let Some(short) = backend::branch_name(name) else {
        return Err("Git doesn't allow that as a branch name".into());
    };
    // Linked worktrees' branches too, which the backend would refuse the name for.
    let taken = short != current
        && (store.workspace(project)).is_some_and(|ws| {
            (ws.all_branches()).any(|b| b.ref_name.as_ref().is_some_and(|r| short_ref(r) == short))
        });
    match taken {
        true => Err(format!("{short} already exists").into()),
        false => Ok(short),
    }
}

/// Run `op` with `name`, typed for a branch, and hand `done` the full ref name of the branch
/// it returns. With `renaming`, a full ref, that branch is being renamed; without, a branch
/// is made or one without a name yet is named.
pub(crate) fn run_named<T: Send + 'static>(
    store: &mut Store,
    project: &ProjectId,
    renaming: Option<FullName>,
    name: String,
    op: impl FnOnce(&Path, &Remap, &str) -> Result<T> + Send + 'static,
    done: impl FnOnce(&mut Store, T, &mut Context<Store>) + 'static,
    cx: &mut Context<Store>,
) {
    let name = name.trim().to_owned();
    let short = backend::branch_name(&name).unwrap_or_else(|| name.clone());
    // Git's rules changed the name, as they do spaces and double dots.
    if short != name {
        store.snack(
            format!("Named it {short}, as Git doesn't allow \"{name}\""),
            cx,
        );
    }
    let label = match renaming {
        Some(_) => "Rename branch",
        None => "Create branch",
    };
    // Making or naming a branch only writes refs. An unnamed branch has no ref to tell it
    // from another unnamed one, so its new name waits for the reload.
    let overlay = Overlay {
        renamed: renaming.map(|from| (from, short)),
        ..Overlay::default()
    };
    let op = move |path: &Path, remap: &Remap| op(path, remap, &name);
    store.run(project, label, overlay, op, done, cx)
}

/// A small modal asking for one line of text: a branch name, a repository to clone, a pull
/// request number, a comment, or credentials for Git.
pub struct Prompt {
    /// Where it acts, as [`Store::place`] says, above the title.
    place: Option<SharedString>,
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
    /// What had the keys before, as a message being written, to give them back to.
    restore_focus: Option<FocusHandle>,
    /// Its own, around its field and buttons, which Tab keeps to.
    pub(crate) focus: FocusHandle,
    _subscription: Subscription,
}

impl AppView {
    /// The menu of `items`, open at `position`, unless there are none.
    pub(crate) fn open_menu(
        &mut self,
        position: Point<Pixels>,
        items: Vec<MenuItem>,
        cx: &mut Context<Self>,
    ) {
        if items.is_empty() {
            return;
        }
        self.menu = Some(ContextMenu {
            position,
            items,
            acting_on: self.store.read(cx).acting_on(),
        });
        cx.notify();
    }

    /// Ask for a new branch's name, then run `op` with it and hand `done` the full ref name of
    /// the branch it returns. `at` is what it acts on, the project or a branch or commit in it,
    /// for the prompt to say where.
    pub(crate) fn prompt_branch<T: Send + 'static>(
        &mut self,
        at: Selection,
        title: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        op: impl FnOnce(&Path, &Remap, &str) -> Result<T> + Send + 'static,
        done: impl FnOnce(&mut Store, T, &mut Context<Store>) + 'static,
    ) {
        let project = at.project().clone();
        let place = self.store.read(cx).place(&at);
        let input = cx.new(|cx| TextInput::new("Branch name", false, cx));
        let checked = project.clone();
        let check = move |this: &AppView, name: &str, cx: &App| {
            branch_name(this.store.read(cx), &checked, name, "").err()
        };
        self.open_prompt(
            Some(place),
            title,
            None,
            "Create",
            input,
            None,
            Some(Box::new(check)),
            window,
            cx,
            move |this, name, _, cx| {
                (this.store).update(cx, |store, cx| {
                    run_named(store, &project, None, name, op, done, cx)
                })
            },
        );
    }

    #[expect(clippy::too_many_arguments)]
    pub(crate) fn open_prompt(
        &mut self,
        place: Option<SharedString>,
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
        // Opened from a picker or over another prompt, as Git asking for a passphrase, go back
        // to where that one came from.
        let restore_focus = match (&self.prompt, &self.palette) {
            (Some(open), _) => open.restore_focus.clone(),
            (None, Some(open)) => open.restore_focus.clone(),
            (None, None) => window.focused(cx),
        };
        // It takes the focus, so nothing else that would take the keys stays over it.
        self.palette = None;
        self.menu = None;
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
                        let focus = this.prompt.take().and_then(|prompt| {
                            (prompt.on_submit)(this, text, window, cx);
                            prompt.restore_focus
                        });
                        this.give_back_focus(focus, window, cx);
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
            place,
            title: title.to_owned().into(),
            detail: detail.map(Into::into),
            confirm: confirm.to_owned().into(),
            input,
            on_submit: Box::new(on_submit),
            on_cancel,
            check,
            hint: None,
            restore_focus,
            focus: cx.focus_handle(),
            _subscription: subscription,
        });
        cx.notify();
    }

    pub(crate) fn cancel_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self.prompt.take().and_then(|prompt| {
            if let Some(on_cancel) = prompt.on_cancel {
                on_cancel(self);
            }
            prompt.restore_focus
        });
        self.give_back_focus(focus, window, cx);
        cx.notify();
    }

    /// Focus what had the keys before a prompt, or what's left on top without one.
    fn give_back_focus(&self, focus: Option<FocusHandle>, window: &mut Window, cx: &mut App) {
        match focus {
            Some(focus) => window.focus(&focus, cx),
            None => self.restore_focus(window, cx),
        }
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
                // Git asks for an HTTPS username before its password, and the username shows.
                let username = prompt.text.starts_with("Username for ");
                let result = this.update_in(cx, |this, window, cx| {
                    let input = cx.new(|cx| match username {
                        true => TextInput::new("Username", false, cx),
                        false => TextInput::new("Passphrase or password", false, cx).masked(),
                    });
                    this.open_prompt(
                        None,
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
        let (focus, returns_to) = (view.focus_handle(cx), window.focused(cx));
        window.focus(&focus, cx);
        let card = cx.focus_handle();
        self.modal = Some((view.into(), focus, card, subscription, returns_to));
        cx.notify();
    }

    /// Close the modal, and a menu open from it, whose items would act on it.
    pub(crate) fn close_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((.., returns_to)) = self.modal.take() {
            self.menu = None;
            self.give_back_focus(returns_to, window, cx);
            cx.notify();
        }
    }

    pub(crate) fn render_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (view, _, focus, ..) = self.modal.as_ref()?;
        // At most most of the window: the modal inside can't say so itself, as this box is
        // only as wide as the modal.
        let card = (div().flex().max_w(relative(0.92)).max_h(relative(0.88))).child(view.clone());
        let card = dialog(card, focus);
        let backdrop = backdrop("modal-backdrop", card, cx, Self::close_modal);
        Some(backdrop.items_center().into_any_element())
    }

    pub(crate) fn render_menu(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let menu = self.menu.as_ref()?;
        let theme = Theme::get(cx);
        let items = menu.items.iter().enumerate().flat_map(|(ix, item)| {
            let handler = item.handler.clone();
            let line = (item.separated && ix > 0).then(|| {
                div()
                    .h(fixed(1.))
                    .mx(dp(10.))
                    .my(dp(4.))
                    .bg(theme.border_3)
                    .into_any_element()
            });
            // As ui-react's menu sections head theirs.
            let heading = item.heading.clone().map(|heading| {
                div()
                    .px(dp(14.))
                    .pt(dp(4.))
                    .pb(dp(2.))
                    .text_size(dp(12.))
                    .text_color(theme.text_3)
                    .child(heading)
                    .into_any_element()
            });
            let row = list_row(("menu-item", ix), false, theme)
                .mx(dp(4.))
                .gap(dp(16.))
                .text_size(dp(13.))
                .text_color(if item.danger {
                    theme.danger
                } else {
                    theme.text_1
                })
                .child(div().flex_1().child(item.label.clone()))
                .when_some(item.detail.clone(), |el, detail| {
                    el.child(div().text_color(theme.text_3).child(detail))
                })
                .when_some(item.key.clone(), |el, key| el.child(keycap(&key, theme)))
                // A row with a detail keeps the tick's place, so the details line up.
                .when(item.ticked || item.detail.is_some(), |el| {
                    el.child(small_icon(Icon::Tick).text_color(match item.ticked {
                        true => theme.text_1,
                        false => gpui::transparent_black(),
                    }))
                })
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.menu = None;
                    handler(this, window, cx);
                    cx.notify();
                }))
                .into_any_element();
            line.into_iter().chain(heading).chain([row])
        });
        // A menu taller than the window, as one of many projects is, scrolls inside the margins
        // `floating` keeps it in.
        let card = (popover(theme).id("context-menu").min_w(dp(180.)))
            .max_h(window.viewport_size().height - fixed(16.))
            .overflow_y_scroll()
            .child(v_flex().py(dp(4.)).children(items));
        Some(floating(menu.position, card, cx, |this, _, cx| {
            this.menu = None;
            cx.notify();
        }))
    }

    pub(crate) fn render_prompt(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let prompt = self.prompt.as_ref()?;
        let theme = Theme::get(cx);
        let input = prompt.input.clone();
        let focused = input.focus_handle(cx).is_focused(window);
        let card = dialog(popover(theme).id("prompt"), &prompt.focus)
            // Escape from its buttons, which the field's own Escape doesn't reach.
            .on_action(cx.listener(|this, _: &Cancel, window, cx| this.cancel_prompt(window, cx)))
            .w(dp(380.))
            .h_auto()
            .p(dp(16.))
            .gap(dp(12.))
            .rounded(dp(10.))
            .child(
                v_flex()
                    .gap(dp(2.))
                    .children(prompt.place.clone().map(|place| muted(place, theme)))
                    .child(
                        div()
                            .text_size(dp(14.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(prompt.title.clone()),
                    ),
            )
            .when_some(prompt.detail.clone(), |el, detail| {
                el.child(muted(detail, theme))
            })
            .child(field(&input, focused, theme).font_family(theme.mono_font.clone()))
            .when_some(prompt.hint.clone(), |el, hint| {
                el.child(
                    div()
                        .text_size(dp(12.))
                        .text_color(theme.danger)
                        .child(hint),
                )
            })
            .child(
                h_flex()
                    .justify_end()
                    .gap(dp(8.))
                    .child(
                        button("prompt-cancel", "Cancel")
                            .style(ButtonStyle::Ghost)
                            .on_click({
                                let input = input.clone();
                                move |_, window, cx| {
                                    input.update(cx, |_, cx| cx.emit(InputEvent::Cancel));
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
            );
        let backdrop = backdrop("prompt-backdrop", card, cx, Self::cancel_prompt);
        Some(backdrop.items_start().pt(dp(120.)).into_any_element())
    }

    /// Snackbars at the bottom centre, error cards at the bottom right, the update card under
    /// them.
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
                            .h(dp(32.))
                            .pl(dp(12.))
                            .pr(dp(if undo.is_some() { 4. } else { 12. }))
                            .gap(dp(8.))
                            .rounded(dp(8.))
                            .bg(theme.snackbar_bg)
                            .text_color(theme.snackbar_text)
                            .text_size(dp(12.))
                            .shadow_lg()
                            // Over a modal's backdrop, which a click would reach and close it.
                            .occlude()
                            .child(message.clone())
                            .when_some(undo.clone(), |el, (project, snapshot)| {
                                el.child(
                                    div()
                                        .w(fixed(1.))
                                        .h(dp(16.))
                                        .bg(theme.snackbar_text.opacity(0.2)),
                                )
                                .child(
                                    h_flex()
                                        .id(("undo", id))
                                        .h(dp(24.))
                                        .px(dp(8.))
                                        .rounded(dp(5.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .hover(|el| el.bg(theme.snackbar_text.opacity(0.12)))
                                        .child("Undo")
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.store.update(cx, |store, cx| {
                                                store.undo_snapshot(
                                                    &project, snapshot, None, None, cx,
                                                )
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
                    ..
                } => {
                    let copied = detail.to_string();
                    let retry = retry.clone().map(|(label, run)| {
                        button(("retry", id), label).small().on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                this.store.update(cx, |store, cx| {
                                    if store.take_toast(id, cx) {
                                        run(store, cx)
                                    }
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
                            .w(dp(320.))
                            .p(dp(14.))
                            .gap(dp(8.))
                            .occlude()
                            .child(
                                h_flex()
                                    .items_start()
                                    .gap(dp(8.))
                                    .child(icon(Icon::Warning).text_color(theme.danger).mt(dp(2.)))
                                    .child(
                                        // A long title wraps rather than running off the card.
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(title.clone()),
                                    ),
                            )
                            // Room for what a server or hook says to do, under what failed.
                            .child(
                                muted(error_summary(detail), theme)
                                    .line_clamp(8)
                                    .text_ellipsis(),
                            )
                            .child(
                                h_flex()
                                    .gap(dp(6.))
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
                    .bottom(dp(STATUS_BAR_HEIGHT + 12.))
                    .right(dp(16.))
                    .gap(dp(8.))
                    .children(errors)
                    .children(self.render_update_card(cx)),
            )
            .children(snackbar.map(|snackbar| {
                // Lifted above the commit footer's buttons, as the design system's hosts do.
                let footer = match self.detail.read(cx).shows_footer(cx) {
                    true => crate::detail::FOOTER_HEIGHT,
                    false => 0.,
                };
                h_flex()
                    .absolute()
                    .bottom(dp(STATUS_BAR_HEIGHT + footer + 16.))
                    .left_0()
                    .right_0()
                    .justify_center()
                    .child(snackbar)
            }))
    }

    /// The update card, while the desktop app's update feed offers a newer version and Later
    /// hasn't put it off. Not over a modal, as What's new, which it opens, is one.
    fn render_update_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = Theme::get(cx);
        let store = self.store.read(cx);
        let version = store.newer_version.clone()?;
        if store.update_put_off.is_some() || self.modal.is_some() {
            return None;
        }
        let progress = store.update_progress();
        let installing = progress.is_some().then(|| format!("Installing {version}"));
        let status = progress.unwrap_or_else(|| "The app restarts once it's installed.".into());
        let title = format!("GitButler {version} is here");
        // Lite's update icon on a tile of the accent, the card's one touch of colour beside its
        // Update button.
        let tile = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(dp(32.))
            .rounded(dp(8.))
            .bg(theme.pop.opacity(0.12))
            .child(icon(Icon::ArrowInBox).text_color(theme.pop));
        Some(
            popover(theme)
                .id("update-card")
                .role(Role::Alert)
                .aria_label(title.clone())
                .w(dp(320.))
                .p(dp(16.))
                .gap(dp(16.))
                .occlude()
                .child(
                    h_flex().items_start().gap(dp(12.)).child(tile).child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(dp(2.))
                            .child(
                                div()
                                    .text_size(dp(14.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(muted(status, theme)),
                    ),
                )
                .child(
                    h_flex()
                        .gap(dp(6.))
                        .child(button("update-notes", "Release notes").small().on_click(
                            cx.listener(|this, _, window, cx| {
                                this.open_whats_new(&OpenReleaseNotes, window, cx)
                            }),
                        ))
                        .child(div().flex_1())
                        .child(
                            button("update-later", "Later")
                                .small()
                                .style(ButtonStyle::Ghost)
                                .tooltip("Hide for 4 hours")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.store.update(cx, |store, cx| store.put_off_update(cx))
                                })),
                        )
                        .child(
                            button("update-app", "Update")
                                .small()
                                .style(ButtonStyle::Pop)
                                .disabled_because(installing)
                                .on_click(cx.listener(|this, _, _, cx| this.install_update(cx))),
                        ),
                )
                .into_any_element(),
        )
    }
}

/// A dimmed layer over the window that takes the pointer, with `card` across its middle. A
/// press outside the card runs `dismiss`.
pub(crate) fn backdrop(
    id: &'static str,
    card: impl InteractiveElement + IntoElement + 'static,
    cx: &mut Context<AppView>,
    dismiss: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .inset_0()
        .bg(gpui::black().opacity(0.25))
        .occlude()
        .flex()
        .justify_center()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, window, cx| dismiss(this, window, cx)),
        )
        .child(card.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()))
}

/// `el`, a prompt or a modal around `focus`. Tab and ⇧Tab go round its fields and buttons,
/// as in a macOS sheet, and never to the window behind it.
fn dialog<E: InteractiveElement>(el: E, focus: &FocusHandle) -> E {
    let (next, previous) = (focus.clone(), focus.clone());
    el.key_context("Dialog")
        .track_focus(focus)
        .on_action(move |_: &FocusNextPane, window, cx| tab_within(&next, false, window, cx))
        .on_action(move |_: &FocusPreviousPane, window, cx| tab_within(&previous, true, window, cx))
}

/// Give the keys to the control in `dialog` after the one that has them, or before it when
/// going `back`. The window's tab order runs through what's behind the dialog too, so this
/// steps on past those. Focusing them on the way tells them nothing, as GPUI only reports
/// focus changes when it draws.
fn tab_within(dialog: &FocusHandle, back: bool, window: &mut Window, cx: &mut App) {
    let start = window.focused(cx);
    let mut first = None;
    loop {
        match back {
            true => window.focus_prev(cx),
            false => window.focus_next(cx),
        }
        if dialog.contains_focused(window, cx) {
            // A list scrolled past it brings it into view. One left from a control outside any
            // list mustn't scroll this one's.
            if cx.has_global::<FocusedAt>() {
                cx.remove_global::<FocusedAt>();
            }
            return window.dispatch_action(Box::new(ShowFocused), cx);
        }
        let now = window.focused(cx);
        // Round the whole window without reaching the dialog.
        if now.is_none() || now == first {
            break;
        }
        first = first.or(now);
    }
    if let Some(start) = start {
        window.focus(&start, cx);
    }
}

/// `card` at `position`, kept inside the window, over everything as a native menu is: what's
/// under it neither hovers nor shows a tooltip, and a press outside it only runs `close`.
pub(crate) fn floating(
    position: Point<Pixels>,
    card: Stateful<Div>,
    cx: &mut Context<AppView>,
    close: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> AnyElement {
    let close = cx.listener(move |this, _, window, cx| close(this, window, cx));
    deferred(
        div().absolute().inset_0().occlude().child(
            anchored()
                .position(position)
                .snap_to_window_with_margin(fixed(8.))
                .child(card.on_mouse_down_out(close)),
        ),
    )
    .with_priority(2)
    .into_any_element()
}
