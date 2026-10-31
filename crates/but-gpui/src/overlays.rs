//! What opens above the window: right-click menus, one-line prompts, modals, and the
//! snackbars and error cards.

use std::rc::Rc;

use anyhow::Result;
use gpui::{
    Action, AnyElement, App, Bounds, ClickEvent, Context, DismissEvent, Div, Entity, FocusHandle,
    Focusable as _, ManagedView, MouseButton, MouseMoveEvent, Pixels, Point, Role, SharedString,
    Stateful, Subscription, Window, anchored, deferred, div, point, prelude::*, relative,
};

use crate::app::{
    AppView, Cancel, Confirm, FocusNextPane, FocusPreviousPane, SelectNext, SelectPrevious,
    ShowContextMenu,
};
use crate::backend::{self, FullName, short_ref};
use crate::commands::Command;
use crate::help::OpenReleaseNotes;
use crate::state::{Notice, Overlay, ProjectId, Selection, Store};
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, FocusedAt, Icon, STATUS_BAR_HEIGHT, ShowFocused, button, dp, error_summary, field,
    fixed, h_flex, icon, keycap, list_row, muted, popover, shortcut, small_icon, v_flex,
};

/// A right-click menu. It takes the keys while it's open, as ⌘K does, so ↑ and ↓ go through
/// its items and Return runs one.
pub struct ContextMenu {
    pub position: Point<Pixels>,
    pub items: Vec<MenuItem>,
    /// What it opened on: see [`Store::acting_on`].
    pub acting_on: (Option<Selection>, Vec<backend::CommitId>),
    /// The item the keys or the pointer are on, which Return runs: none until either moves.
    highlighted: Option<usize>,
    /// What had the keys before it took them, which its items run from.
    restore_focus: Option<FocusHandle>,
}

/// What a list draws as having the keys: what has the focus, or what the open context menu
/// took it from and gives it back to, so what the menu acts on stays solid under it.
pub(crate) fn keys_at(window: &Window, cx: &App) -> Option<FocusHandle> {
    let focused = window.focused(cx)?;
    let lent = window.root::<AppView>().flatten().and_then(|app| {
        let app = app.read(cx);
        let menu = (app.menu.as_ref()).filter(|_| focused == app.menu_focus)?;
        menu.restore_focus.clone()
    });
    Some(lent.unwrap_or(focused))
}

/// Whether a list drawing with `focus` draws its selection solid, as [`keys_at`] says.
pub(crate) fn has_keys(focus: &FocusHandle, window: &Window, cx: &App) -> bool {
    keys_at(window, cx).is_some_and(|keys| keys == *focus)
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
    /// Shown as destructive, in red.
    pub danger: bool,
    /// What [`Command::risky`] says of the command it runs, so a list typed into finds it by
    /// its words, not by letters scattered through it.
    pub risky: bool,
    /// Starts a new group, with a line above it.
    pub separated: bool,
    /// Starts a new group under this heading, as a choice of one does: "Sort by".
    pub heading: Option<SharedString>,
    /// The choice its group stands at, which carries a tick.
    pub ticked: bool,
    /// The command it runs, which says when it's offered and how it reads for the selection
    /// once the menu opens.
    pub command: Option<&'static dyn Command>,
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
            risky: false,
            separated: false,
            heading: None,
            ticked: false,
            command: None,
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

    /// An item that dispatches `action` where focus is, showing the keys bound to it, and risky
    /// when the command it is says so, whatever the item calls it.
    pub fn action(label: impl Into<SharedString>, action: &dyn Action, cx: &App) -> Self {
        let key = shortcut(action, cx);
        let risky =
            (crate::commands::COMMANDS.iter()).any(|c| c.name() == action.name() && c.risky());
        let action = action.boxed_clone();
        Self {
            key,
            risky,
            ..Self::new(label, move |_, window, cx| {
                window.dispatch_action(action.boxed_clone(), cx)
            })
        }
    }

    /// An item that dispatches `command` where focus is, showing the keys bound to it. It's
    /// there when the menu opens only where the command is offered, worded as the command says
    /// for the selection then, or by its title.
    pub fn command(command: &'static dyn Command, cx: &App) -> Self {
        Self {
            command: Some(command),
            ..Self::action(command.title(), command, cx)
        }
    }

    /// Shows the keys of `action` instead, which does the same here: ⌘C beside the one copy
    /// it makes.
    pub fn keys(mut self, action: &dyn Action, cx: &App) -> Self {
        self.key = shortcut(action, cx);
        self
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

/// Apply the intent `op` makes of `name`, typed for a branch, and hand `done` what it returns.
/// With `renaming`, a full ref, that branch is being renamed; without, a branch is made or
/// one without a name yet is named.
pub(crate) fn run_named<I: backend::Intent>(
    store: &mut Store,
    project: &ProjectId,
    renaming: Option<FullName>,
    name: String,
    op: impl FnOnce(String) -> I,
    done: impl FnOnce(&mut Store, I::Done, &mut Context<Store>) + 'static,
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
    store.apply(project, label, overlay, op(name), done, cx)
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
        // Which commands are offered shows once the selection a right-click just moved has
        // registered its actions, when the frame it changed is drawn.
        self.pending_menu = Some(ContextMenu {
            position,
            items,
            acting_on: self.store.read(cx).acting_on(),
            highlighted: None,
            restore_focus: None,
        });
        cx.notify();
    }

    /// Open the menu waiting on the frame drawn since, with only the commands offered now,
    /// and give it the keys.
    pub(crate) fn open_pending_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut menu) = self.pending_menu.take() else {
            return;
        };
        menu.items = self.offered(menu.items, window, cx);
        if menu.items.is_empty() {
            return;
        }
        // In place of one open still, back to where that one came from.
        menu.restore_focus = match self.menu_focus.is_focused(window) {
            true => self.menu.take().and_then(|open| open.restore_focus),
            false => window.focused(cx),
        };
        window.focus(&self.menu_focus, cx);
        self.menu = Some(menu);
        cx.notify();
    }

    /// Close the menu, giving the keys back to what had them while it still has them.
    pub(crate) fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else {
            return;
        };
        if self.menu_focus.is_focused(window) {
            self.give_back_focus(menu.restore_focus, window, cx);
        }
        cx.notify();
    }

    /// Run the menu's item `ix` from where the keys were before it opened, closing it.
    fn run_menu_item(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let handler = (self.menu.as_ref())
            .and_then(|menu| menu.items.get(ix))
            .map(|item| item.handler.clone());
        self.close_menu(window, cx);
        if let Some(handler) = handler {
            Self::once_drawn(window, cx, move |this, window, cx| {
                handler(this, window, cx)
            });
        }
    }

    /// Run `run` once the window has drawn since now. An action goes to the handlers of the
    /// frame drawn last, so a command run as the keys come back from a menu or ⌘K would miss
    /// those registered only while what gets them has them, as copying the cursor's path.
    pub(crate) fn once_drawn(
        window: &Window,
        cx: &Context<Self>,
        run: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let this = cx.entity().downgrade();
        // A frame's callbacks run before it draws, so the next frame's, after the one between.
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, cx| {
                (this.update(cx, |this, cx| run(this, window, cx))).ok();
            })
        });
    }

    /// Move the menu's highlight by `delta` items, from the first or the last when none is.
    fn step_menu(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(menu) = &mut self.menu else {
            return;
        };
        let last = menu.items.len().saturating_sub(1);
        menu.highlighted = Some(match (menu.highlighted, delta < 0) {
            (None, false) => 0,
            (None, true) => last,
            (Some(at), _) => at.saturating_add_signed(delta).min(last),
        });
        cx.notify();
    }

    /// ⇧F10: what right-clicking the selected row offers, open at that row, or in the middle
    /// of the window while it isn't on screen.
    pub(crate) fn show_context_menu(
        &mut self,
        _: &ShowContextMenu,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (items, row) = self.selection_menu(window, cx);
        let position = match row {
            Some(row) => point(row.left() + fixed(16.), row.bottom()),
            None => {
                let size = window.viewport_size();
                point(size.width / 2., size.height / 3.)
            }
        };
        self.open_menu(position, items, cx);
    }

    /// What right-clicking the selection offers, as ⌘K starts with and ⇧F10 opens, and where
    /// its row is while it's on screen: in the detail with the keys, the file under the
    /// cursor, or what its page's list has selected; else the navigator's row.
    pub(crate) fn selection_menu(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> (Vec<MenuItem>, Option<Bounds<Pixels>>) {
        let detail = (self.detail.read(cx).is_focused(window, cx))
            .then(|| (self.detail).update(cx, |detail, cx| detail.selected_menu(window, cx)))
            .flatten();
        if let Some(menu) = detail {
            return menu;
        }
        let Some(selection) = self.store.read(cx).selection.clone() else {
            return (Vec::new(), None);
        };
        let navigator = self.navigator.read(cx);
        (
            navigator.menu_items(&selection, cx),
            navigator.row_bounds(&selection),
        )
    }

    /// `items` without the commands not offered where the keys are, and the rest worded for the
    /// selection, keeping a line above where a group starts.
    pub(crate) fn offered(&self, items: Vec<MenuItem>, window: &Window, cx: &App) -> Vec<MenuItem> {
        let mut separated = false;
        (items.into_iter())
            .filter_map(|mut item| {
                if let Some(command) = item.command {
                    if !self.offers(command, window, cx) {
                        separated |= item.separated;
                        return None;
                    }
                    if let Some(phrase) = command.phrase(self, cx) {
                        item.label = phrase.into();
                    }
                }
                item.separated |= std::mem::take(&mut separated);
                Some(item)
            })
            .collect()
    }

    /// Whether ⌘K and the menus offer `command` where the keys are.
    pub(crate) fn offers(&self, command: &dyn Command, window: &Window, cx: &App) -> bool {
        window.is_action_available(command, cx) && command.listed(self, cx)
    }

    /// Ask for a new branch's name, then apply the intent `op` makes of it and hand `done` what
    /// it returns. `at` is what it acts on, the project or a branch or commit in it, for the
    /// prompt to say where.
    pub(crate) fn prompt_branch<I: backend::Intent>(
        &mut self,
        at: Selection,
        title: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        op: impl FnOnce(String) -> I + 'static,
        done: impl FnOnce(&mut Store, I::Done, &mut Context<Store>) + 'static,
    ) {
        let project = at.project().clone();
        let place = self.store.read(cx).place(&at);
        let input = cx.new(|cx| TextInput::new("Branch name", false, cx).spaces_as_dashes());
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
        self.close_menu(window, cx);
        // Opened from a picker or over another prompt, as Git asking for a passphrase, go back
        // to where that one came from.
        let restore_focus = match (&self.prompt, &self.palette) {
            (Some(open), _) => open.restore_focus.clone(),
            (None, Some(open)) => open.restore_focus.clone(),
            (None, None) => window.focused(cx),
        };
        // It takes the focus, so nothing else that would take the keys stays over it.
        if self.palette.take().is_some() {
            (self.navigator).update(cx, |navigator, cx| navigator.stop_going(true, cx));
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
        self.close_menu(window, cx);
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
            let row = list_row(("menu-item", ix), menu.highlighted == Some(ix), theme)
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
                // The pointer moves the highlight too, so only one item is lit.
                .on_mouse_move(cx.listener(move |this, _: &MouseMoveEvent, _, cx| {
                    if let Some(menu) = this.menu.as_mut().filter(|m| m.highlighted != Some(ix)) {
                        menu.highlighted = Some(ix);
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.run_menu_item(ix, window, cx)
                }))
                .into_any_element();
            line.into_iter().chain(heading).chain([row])
        });
        // A menu taller than the window, as one of many projects is, scrolls inside the margins
        // `floating` keeps it in.
        let card = (popover(theme).id("context-menu").min_w(dp(180.)))
            .track_focus(&self.menu_focus)
            .key_context("ContextMenu")
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.step_menu(1, cx)))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.step_menu(-1, cx)))
            .on_action(cx.listener(|this, _: &Confirm, window, cx| {
                if let Some(ix) = this.menu.as_ref().and_then(|menu| menu.highlighted) {
                    this.run_menu_item(ix, window, cx)
                }
            }))
            .max_h(window.viewport_size().height - fixed(16.))
            .overflow_y_scroll()
            .child(v_flex().py(dp(4.)).children(items));
        Some(floating(menu.position, card, cx, Self::close_menu))
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
