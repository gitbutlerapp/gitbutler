//! What opens above the window: right-click menus, one-line prompts, modals, and the
//! snackbars and error cards.

use std::path::Path;
use std::rc::Rc;

use anyhow::Result;
use gpui::{
    Action, AnyElement, App, ClickEvent, Context, DismissEvent, Div, Entity, FocusHandle,
    Focusable as _, ManagedView, MouseButton, Pixels, Point, Role, SharedString, Stateful,
    Subscription, Window, anchored, deferred, div, prelude::*, px, relative,
};

use crate::app::AppView;
use crate::backend::{self, short_ref};
use crate::state::{Notice, Overlay, ProjectId, Remap, Selection, Store};
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, Icon, STATUS_BAR_HEIGHT, button, error_summary, field, h_flex, icon, keycap,
    list_row, muted, popover, shortcut, small_icon, v_flex,
};

/// A right-click menu.
pub struct ContextMenu {
    pub position: Point<Pixels>,
    pub items: Vec<MenuItem>,
    /// What it opened on: see [`Store::acting_on`].
    pub acting_on: (Option<Selection>, usize),
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

/// The short name a branch named `name` gets, as the backend normalizes it, or why it
/// can't have that name.
fn branch_name(name: &str) -> Result<String, SharedString> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Enter a name for the branch".into());
    }
    backend::branch_name(name).ok_or_else(|| "Git doesn't allow that as a branch name".into())
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
    /// The branch the prompt renames or makes one on top of. Once it's gone, as when an
    /// agent renamed it, the prompt goes too, as a menu does, rather than fail.
    pub(crate) on_branch: Option<(ProjectId, String)>,
    /// What had the keys before, as a message being written, to give them back to.
    restore_focus: Option<FocusHandle>,
    _subscription: Subscription,
}

impl AppView {
    pub fn open_menu(
        &mut self,
        position: Point<Pixels>,
        items: Vec<MenuItem>,
        cx: &mut Context<Self>,
    ) {
        self.menu = Some(ContextMenu {
            position,
            items,
            acting_on: self.store.read(cx).acting_on(),
        });
        cx.notify();
    }

    /// Ask for a branch name, then run `op` with it and hand `done` the full ref name of the
    /// branch it returns. `at` is what it acts on, the project or a branch or commit in it, for
    /// the prompt to say where. With the branch it's `renaming`, a full ref, that branch is
    /// being renamed; without one, or an empty one for a branch without a name yet, it asks
    /// as for a new branch.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prompt_branch(
        &mut self,
        at: Selection,
        title: &str,
        renaming: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
        op: impl FnOnce(&Path, &Remap, &str) -> Result<String> + Send + 'static,
        done: impl FnOnce(&mut Store, String, &mut Context<Store>) + 'static,
    ) {
        let project = at.project().clone();
        let place = self.store.read(cx).place(&at);
        let renaming = renaming.filter(|from| !from.is_empty());
        let initial = renaming.as_deref().map_or("", short_ref);
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
            // Linked worktrees' branches too, which the backend would refuse the name for.
            let taken = short != current
                && (this.store.read(cx).workspace(&checked)).is_some_and(|ws| {
                    (ws.lanes().flatten()).any(|b| short_ref(&b.ref_name) == short)
                });
            taken.then(|| format!("{short} already exists").into())
        };
        let current = initial.to_owned();
        // As the desktop app says: a rename doesn't reach the remote.
        let pushed = (renaming.as_deref())
            .and_then(|from| self.store.read(cx).workspace(&project)?.find_branch(from))
            .is_some_and(|branch| branch.push != backend::PushState::NeverPushed);
        let detail = pushed.then(|| {
            "It's been pushed. The remote keeps it under its old name, and pushing again \
             makes a new remote branch, apart from any pull request of the old one."
                .to_owned()
        });
        let renamed = renaming.clone().map(|from| (project.clone(), from));
        self.open_prompt(
            Some(place),
            title,
            detail,
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
                // Making or naming a branch only writes refs. An unnamed branch has no ref
                // to tell it from another unnamed one, so its new name waits for the reload.
                let overlay = Overlay {
                    renamed: (renaming.clone())
                        .map(|from| (from, branch_name(&name).unwrap_or(name.clone()))),
                    ..Overlay::default()
                };
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        label,
                        overlay,
                        move |path, remap| op(path, remap, &name),
                        done,
                        cx,
                    )
                });
            },
        );
        if let Some(from) = renamed {
            self.prompt_on_branch(from);
        }
    }

    /// Tie the prompt just opened to the branch it acts on, so it goes once the branch does.
    pub(crate) fn prompt_on_branch(&mut self, branch: (ProjectId, String)) {
        // Not to a prompt Git is waiting on, which one asked for meanwhile left open.
        if let Some(prompt) = self.prompt.as_mut().filter(|p| p.on_cancel.is_none()) {
            prompt.on_branch = Some(branch);
        }
    }

    #[allow(clippy::too_many_arguments)]
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
            on_branch: None,
            restore_focus,
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
        let focus = view.focus_handle(cx);
        window.focus(&focus, cx);
        self.modal = Some((view.into(), focus, subscription));
        cx.notify();
    }

    /// Close the modal, and a menu open from it, whose items would act on it.
    pub(crate) fn close_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.modal.take().is_some() {
            self.menu = None;
            self.focus_main(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn render_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (view, ..) = self.modal.as_ref()?;
        // At most most of the window: the modal inside can't say so itself, as this box is
        // only as wide as the modal.
        let card = (div().flex().max_w(relative(0.92)).max_h(relative(0.88))).child(view.clone());
        let backdrop = backdrop("modal-backdrop", card, cx, Self::close_modal);
        Some(backdrop.items_center().into_any_element())
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
        let card = (popover(theme).id("context-menu").min_w(px(180.)).py(px(4.))).children(items);
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
        let card = popover(theme)
            .id("prompt")
            .w(px(380.))
            .h_auto()
            .p(px(16.))
            .gap(px(12.))
            .rounded(px(10.))
            .child(
                v_flex()
                    .gap(px(2.))
                    .children(prompt.place.clone().map(|place| muted(place, theme)))
                    .child(
                        div()
                            .text_size(px(14.))
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
        Some(backdrop.items_start().pt(px(120.)).into_any_element())
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
                            // Over a modal's backdrop, which a click would reach and close it.
                            .occlude()
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
                            .w(px(320.))
                            .p(px(14.))
                            .gap(px(8.))
                            .occlude()
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
                .snap_to_window_with_margin(px(8.))
                .child(card.on_mouse_down_out(close)),
        ),
    )
    .with_priority(2)
    .into_any_element()
}
