//! The window's root: the project navigator on the left, details on the right, and the
//! overlays (menus, prompts, toasts) above both.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, FocusHandle, Focusable, KeyBinding, MouseButton,
    PathPromptOptions, Pixels, Point, PromptLevel, ScrollHandle, SharedString, Subscription,
    Window, actions, anchored, deferred, div, prelude::*, px,
};

use crate::backend::{self, CommitId};
use crate::detail::Detail;
use crate::state::{Cut, ProjectId, Selection, Store, StoreEvent, ToastKind};
use crate::theme::Theme;
use crate::transfer::{Transfer, short_ref};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{ButtonStyle, Icon, button, h_flex, icon, v_flex};

actions!(
    gitbutler,
    [
        Quit,
        AddProject,
        Refresh,
        Undo,
        Fetch,
        NewBranch,
        SelectPrevious,
        SelectNext,
        Expand,
        Collapse,
        Confirm,
        CutSelection,
        PasteHere,
        Cancel,
        DeleteSelection,
        Push,
        FocusNavigator,
        FocusFilter,
        FocusMessage,
    ]
);

pub fn bind_keys(cx: &mut App) {
    let nav = Some("Navigator");
    let workspace = Some("Workspace && !TextInput");
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-o", AddProject, None),
        KeyBinding::new("cmd-r", Refresh, None),
        KeyBinding::new("cmd-shift-r", Fetch, None),
        KeyBinding::new("cmd-shift-n", NewBranch, None),
        KeyBinding::new("cmd-shift-p", Push, None),
        KeyBinding::new("cmd-1", FocusNavigator, None),
        KeyBinding::new("cmd-f", FocusFilter, None),
        KeyBinding::new("cmd-2", FocusMessage, None),
        KeyBinding::new("cmd-z", Undo, workspace),
        KeyBinding::new("cmd-x", CutSelection, workspace),
        KeyBinding::new("cmd-v", PasteHere, workspace),
        KeyBinding::new("escape", Cancel, workspace),
        KeyBinding::new("up", SelectPrevious, nav),
        KeyBinding::new("k", SelectPrevious, nav),
        KeyBinding::new("down", SelectNext, nav),
        KeyBinding::new("j", SelectNext, nav),
        KeyBinding::new("right", Expand, nav),
        KeyBinding::new("l", Expand, nav),
        KeyBinding::new("left", Collapse, nav),
        KeyBinding::new("h", Collapse, nav),
        KeyBinding::new("enter", Confirm, nav),
        KeyBinding::new("space", Confirm, nav),
        KeyBinding::new("cmd-backspace", DeleteSelection, nav),
    ]);
}

/// A right-click menu.
pub struct ContextMenu {
    pub position: Point<Pixels>,
    pub items: Vec<MenuItem>,
}

type MenuHandler = Rc<dyn Fn(&mut AppView, &mut Window, &mut Context<AppView>)>;
type SubmitHandler = Rc<dyn Fn(&mut AppView, String, &mut Window, &mut Context<AppView>)>;
type CancelHandler = Box<dyn FnOnce(&mut AppView)>;

pub struct MenuItem {
    pub label: SharedString,
    pub danger: bool,
    pub handler: MenuHandler,
}

impl MenuItem {
    pub fn new(
        label: impl Into<SharedString>,
        handler: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            danger: false,
            handler: Rc::new(handler),
        }
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
}

/// A small modal asking for one line of text: a branch name or a credential.
struct Prompt {
    title: SharedString,
    detail: Option<SharedString>,
    confirm: SharedString,
    input: Entity<TextInput>,
    on_submit: SubmitHandler,
    on_cancel: Option<CancelHandler>,
    _subscription: Subscription,
}

pub struct AppView {
    pub store: Entity<Store>,
    pub detail: Entity<Detail>,
    pub nav_focus: FocusHandle,
    pub nav_scroll: ScrollHandle,
    /// Branches showing all their commits instead of the first few.
    pub unfolded_branches: HashSet<(ProjectId, String)>,
    /// Narrows the navigator to projects whose name contains this text.
    pub filter: Entity<TextInput>,
    pub menu: Option<ContextMenu>,
    prompt: Option<Prompt>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = cx.new(Store::new);
        let app = cx.entity().downgrade();
        let detail = cx.new(|cx| Detail::new(store.clone(), app, window, cx));
        let subscriptions = vec![
            cx.observe(&store, |_, _, cx| cx.notify()),
            cx.subscribe(&store, |this, _, event, cx| {
                if let StoreEvent::SelectionChanged = event {
                    this.scroll_selection_into_view(cx);
                }
            }),
            cx.observe_window_appearance(window, |_, window, cx| {
                cx.set_global(Theme::for_appearance(window.appearance(), cx));
                cx.refresh_windows();
            }),
        ];
        let nav_focus = cx.focus_handle();
        window.focus(&nav_focus, cx);
        let filter = cx.new(|cx| TextInput::new("Filter projects", false, cx));
        let filter_subscription = cx.subscribe_in(
            &filter,
            window,
            |this, filter, event, window, cx| match event {
                InputEvent::Changed => cx.notify(),
                InputEvent::Submit => {
                    // Jump into the first match.
                    let query = filter.read(cx).text().to_lowercase();
                    let first = this
                        .store
                        .read(cx)
                        .projects
                        .iter()
                        .find(|entry| entry.project.name.to_lowercase().contains(&query))
                        .map(|entry| entry.id.clone());
                    if let Some(id) = first {
                        this.select(Selection::Project(id), window, cx);
                    }
                }
                InputEvent::Cancel => {
                    filter.update(cx, |filter, cx| filter.set_text("", cx));
                    window.focus(&this.nav_focus, cx);
                }
            },
        );
        let mut subscriptions = subscriptions;
        subscriptions.push(filter_subscription);
        Self::listen_for_credential_prompts(window, cx);
        Self {
            store,
            detail,
            nav_focus,
            nav_scroll: ScrollHandle::new(),
            unfolded_branches: HashSet::new(),
            filter,
            menu: None,
            prompt: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn select(&mut self, selection: Selection, window: &mut Window, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.select(Some(selection), cx));
        window.focus(&self.nav_focus, cx);
    }

    fn selection(&self, cx: &App) -> Option<Selection> {
        self.store.read(cx).selection.clone()
    }

    fn quit(&mut self, _: &Quit, _: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    pub fn add_project(&mut self, _: &AddProject, _: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add project".into()),
        });
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.into_iter().next()
            {
                store.update(cx, |store, cx| store.add_project(path, cx));
            }
        })
        .detach();
    }

    pub fn refresh(&mut self, _: &Refresh, _: &mut Window, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.load_projects(cx);
            store.reload_all(cx);
        });
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.selection(cx).map(|s| s.project().clone()) else {
            return;
        };
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Undo",
                backend::undo,
                |store, undone, cx| {
                    let message = if undone { "Undone" } else { "Nothing to undo" };
                    store.toast(ToastKind::Info, message, cx);
                },
                cx,
            )
        });
    }

    fn fetch(&mut self, _: &Fetch, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.selection(cx).map(|s| s.project().clone()) {
            self.fetch_project(project, cx);
        }
    }

    pub fn new_branch(&mut self, _: &NewBranch, window: &mut Window, cx: &mut Context<Self>) {
        let Some(project) = self.selection(cx).map(|s| s.project().clone()).or_else(|| {
            self.store
                .read(cx)
                .projects
                .iter()
                .find(|p| p.expanded)
                .map(|p| p.id.clone())
        }) else {
            return;
        };
        self.prompt_name(
            "New branch",
            "Create",
            "",
            window,
            cx,
            move |this, name, _, cx| {
                let project = project.clone();
                let selected = project.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Create branch",
                        move |path| backend::create_branch(path, Some(&name)),
                        move |store, ref_name, cx| {
                            store.select(Some(Selection::Branch(selected, ref_name)), cx)
                        },
                        cx,
                    )
                });
            },
        );
    }

    pub fn fetch_project(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Fetch",
                backend::fetch,
                |store, (), cx| store.toast(ToastKind::Info, "Fetched", cx),
                cx,
            )
        });
    }

    /// Rebase the project's branches onto its target's latest commits.
    pub fn update_project(&mut self, project: ProjectId, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Update",
                backend::update_from_target,
                |store, (), cx| store.toast(ToastKind::Info, "Updated from target", cx),
                cx,
            )
        });
    }

    pub fn push(&mut self, project: ProjectId, ref_name: String, cx: &mut Context<Self>) {
        let force = self
            .store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_branch(&ref_name))
            .is_some_and(|branch| branch.push == backend::PushState::NeedsForce);
        let name = short_ref(&ref_name).to_owned();
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                format!("Push {name}"),
                move |path| backend::push(path, &ref_name, force),
                move |store, (), cx| store.toast(ToastKind::Info, format!("Pushed {name}"), cx),
                cx,
            )
        });
    }

    fn push_selected(&mut self, _: &Push, _: &mut Window, cx: &mut Context<Self>) {
        let branch = match self.selection(cx) {
            Some(Selection::Branch(project, ref_name)) => Some((project, ref_name)),
            Some(Selection::Commit(project, id)) => self
                .store
                .read(cx)
                .workspace(&project)
                .and_then(|ws| ws.find_commit(id))
                .map(|(branch, _)| (project.clone(), branch.ref_name.clone())),
            _ => None,
        };
        if let Some((project, ref_name)) = branch {
            self.push(project, ref_name, cx);
        }
    }

    pub fn rename_branch(
        &mut self,
        project: ProjectId,
        ref_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = short_ref(&ref_name).to_owned();
        self.prompt_name(
            "Rename branch",
            "Rename",
            &current,
            window,
            cx,
            move |this, name, _, cx| {
                let project = project.clone();
                let ref_name = ref_name.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Rename branch",
                        move |path| backend::rename_branch(path, &ref_name, &name),
                        {
                            let project = project.clone();
                            move |store, renamed, cx| {
                                store.select(Some(Selection::Branch(project, renamed)), cx)
                            }
                        },
                        cx,
                    )
                });
            },
        );
    }

    pub fn add_branch_on_top(
        &mut self,
        project: ProjectId,
        below: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prompt_name(
            "New branch on top",
            "Create",
            "",
            window,
            cx,
            move |this, name, _, cx| {
                let project = project.clone();
                let below = below.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Create branch",
                        move |path| backend::create_dependent_branch(path, &below, &name),
                        {
                            let project = project.clone();
                            move |store, ref_name, cx| {
                                store.select(Some(Selection::Branch(project, ref_name)), cx)
                            }
                        },
                        cx,
                    )
                });
            },
        );
    }

    pub fn remove_branch(
        &mut self,
        project: ProjectId,
        ref_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = short_ref(&ref_name).to_owned();
        self.confirm_then(
            format!("Delete branch {name}?"),
            "Its commits go too. You can undo this with ⌘Z.",
            "Delete",
            window,
            cx,
            move |this, cx| {
                let project = project.clone();
                let ref_name = ref_name.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Delete branch",
                        move |path| backend::remove_branch(path, &ref_name),
                        |store, (), cx| store.toast(ToastKind::Info, "Branch deleted", cx),
                        cx,
                    )
                });
            },
        );
    }

    pub fn uncommit(&mut self, project: ProjectId, id: CommitId, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Uncommit",
                move |path| backend::uncommit(path, &[id]),
                |store, (), cx| store.toast(ToastKind::Info, "Uncommitted", cx),
                cx,
            )
        });
    }

    pub fn discard_commit(
        &mut self,
        project: ProjectId,
        id: CommitId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_then(
            format!("Discard commit {}?", id.short()),
            "Its changes are dropped. You can undo this with ⌘Z.",
            "Discard",
            window,
            cx,
            move |this, cx| {
                let project = project.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Discard commit",
                        move |path| backend::discard_commit(path, id),
                        |store, (), cx| store.toast(ToastKind::Info, "Commit discarded", cx),
                        cx,
                    )
                });
            },
        );
    }

    pub fn discard(
        &mut self,
        project: ProjectId,
        changes: Vec<backend::Selected>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let what = match changes.as_slice() {
            [] => return,
            [one] => match &one.hunks {
                Some(hunks) if hunks.len() == 1 => format!("a hunk of {}", one.file.path),
                Some(hunks) => format!("{} hunks of {}", hunks.len(), one.file.path),
                None => one.file.path.clone(),
            },
            many => format!("{} files", many.len()),
        };
        self.confirm_then(
            format!("Discard changes to {what}?"),
            "You can undo this with ⌘Z.",
            "Discard",
            window,
            cx,
            move |this, cx| {
                let project = project.clone();
                let changes = changes.clone();
                this.store.update(cx, |store, cx| {
                    store.run(
                        &project,
                        "Discard",
                        move |path| backend::discard(path, &changes),
                        |store, (), cx| store.toast(ToastKind::Info, "Changes discarded", cx),
                        cx,
                    )
                });
            },
        );
    }

    /// Ask with a native dialog before doing something destructive.
    fn confirm_then(
        &mut self,
        message: String,
        detail: &str,
        confirm: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut AppView, &mut Context<AppView>) + 'static,
    ) {
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            Some(detail),
            &[confirm, "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                this.update(cx, |this, cx| then(this, cx)).ok();
            }
        })
        .detach();
    }

    fn prompt_name(
        &mut self,
        title: &str,
        confirm: &str,
        initial: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_submit: impl Fn(&mut AppView, String, &mut Window, &mut Context<AppView>) + 'static,
    ) {
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Branch name", false, cx);
            input.set_text(initial, cx);
            input.select_all_text(cx);
            input
        });
        self.open_prompt(
            title,
            None,
            confirm,
            input,
            None,
            window,
            cx,
            move |this, name, window, cx| {
                let name = name.trim().to_owned();
                if !name.is_empty() {
                    on_submit(this, name, window, cx);
                }
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn open_prompt(
        &mut self,
        title: &str,
        detail: Option<String>,
        confirm: &str,
        input: Entity<TextInput>,
        on_cancel: Option<CancelHandler>,
        window: &mut Window,
        cx: &mut Context<Self>,
        on_submit: impl Fn(&mut AppView, String, &mut Window, &mut Context<AppView>) + 'static,
    ) {
        let subscription =
            cx.subscribe_in(
                &input,
                window,
                |this, input, event, window, cx| match event {
                    InputEvent::Submit => {
                        let text = input.read(cx).text().to_owned();
                        if let Some(prompt) = this.prompt.take() {
                            (prompt.on_submit)(this, text, window, cx);
                        }
                        window.focus(&this.nav_focus, cx);
                        cx.notify();
                    }
                    InputEvent::Cancel => {
                        this.cancel_prompt(window, cx);
                    }
                    InputEvent::Changed => {}
                },
            );
        window.focus(&input.focus_handle(cx), cx);
        // A prompt that's replaced still needs its answer.
        if let Some(previous) = self.prompt.take()
            && let Some(on_cancel) = previous.on_cancel
        {
            on_cancel(self);
        }
        self.prompt = Some(Prompt {
            title: title.to_owned().into(),
            detail: detail.map(Into::into),
            confirm: confirm.to_owned().into(),
            input,
            on_submit: Rc::new(on_submit),
            on_cancel,
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
        window.focus(&self.nav_focus, cx);
        cx.notify();
    }

    /// Git asks for passphrases and passwords through GitButler; show those as a prompt.
    fn listen_for_credential_prompts(window: &mut Window, cx: &mut Context<Self>) {
        let (sender, mut prompts) = futures::channel::mpsc::unbounded();
        backend::askpass::init(move |prompt| {
            sender.unbounded_send(prompt).ok();
        });
        cx.spawn_in(window, async move |this, cx| {
            use futures::StreamExt as _;
            while let Some(prompt) = prompts.next().await {
                let id = std::cell::Cell::new(Some(prompt.id));
                let id = Rc::new(id);
                let result = this.update_in(cx, |this, window, cx| {
                    let input =
                        cx.new(|cx| TextInput::new("Passphrase or password", false, cx).masked());
                    let cancel_id = id.clone();
                    this.open_prompt(
                        "Git needs your credentials",
                        Some(prompt.text),
                        "Continue",
                        input,
                        Some(Box::new(move |_| {
                            if let Some(id) = cancel_id.take() {
                                backend::askpass::respond(id, None);
                            }
                        })),
                        window,
                        cx,
                        move |_, secret, _, _| {
                            if let Some(id) = id.take() {
                                backend::askpass::respond(id, Some(secret));
                            }
                        },
                    );
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn cut_selection(&mut self, _: &CutSelection, _: &mut Window, cx: &mut Context<Self>) {
        let cut = match self.selection(cx) {
            Some(Selection::Commit(project, id)) => Some(Cut::Commit(project, id)),
            Some(Selection::Uncommitted(project)) => {
                let files = self.detail.read(cx).checked_uncommitted(cx);
                (!files.is_empty()).then_some(Cut::Files {
                    project,
                    from: None,
                    files,
                })
            }
            _ => None,
        };
        if let Some(cut) = cut {
            let message = format!("Cut {}. Pick a spot and press ⌘V.", cut.describe());
            self.store.update(cx, |store, cx| {
                store.set_cut(Some(cut), cx);
                store.toast(ToastKind::Info, message, cx);
            });
        }
    }

    fn paste_here(&mut self, _: &PasteHere, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(cut), Some(target)) = (self.store.read(cx).cut.clone(), self.selection(cx))
        else {
            return;
        };
        self.transfer(&cut, &target, window.modifiers().alt, window, cx);
    }

    pub fn cut_files(
        &mut self,
        project: ProjectId,
        from: Option<CommitId>,
        files: Vec<backend::FileChange>,
        cx: &mut Context<Self>,
    ) {
        let cut = Cut::Files {
            project,
            from,
            files,
        };
        let message = format!("Cut {}. Pick a spot and press ⌘V.", cut.describe());
        self.store.update(cx, |store, cx| {
            store.set_cut(Some(cut), cx);
            store.toast(ToastKind::Info, message, cx);
        });
    }

    /// Land `payload` on `target`, from a drop or a paste.
    pub fn transfer(
        &mut self,
        payload: &Cut,
        target: &Selection,
        alternate: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(transfer) = Transfer::resolve(payload, target, alternate, self.store.read(cx))
        else {
            self.store.update(cx, |store, cx| {
                store.toast(ToastKind::Info, "That can't go there", cx)
            });
            return;
        };
        let project = target.project().clone();
        let staged = self.store.update(cx, |store, cx| {
            store.set_cut(None, cx);
            transfer.perform(&project, store, cx)
        });
        if let Some((branch, files)) = staged {
            self.store.update(cx, |store, cx| {
                store.select(Some(Selection::Uncommitted(project.clone())), cx)
            });
            self.detail
                .update(cx, |detail, cx| detail.stage(branch, &files, window, cx));
        }
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            cx.notify();
            return;
        }
        if self.store.read(cx).cut.is_some() {
            self.store.update(cx, |store, cx| store.set_cut(None, cx));
        }
    }

    fn focus_filter(&mut self, _: &FocusFilter, window: &mut Window, cx: &mut Context<Self>) {
        self.filter
            .update(cx, |filter, cx| filter.select_all_text(cx));
        window.focus(&self.filter.focus_handle(cx), cx);
    }

    fn focus_navigator(&mut self, _: &FocusNavigator, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.nav_focus, cx);
    }

    fn focus_message(&mut self, _: &FocusMessage, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(project) = self.selection(cx).map(|s| s.project().clone()) {
            self.store.update(cx, |store, cx| {
                store.select(Some(Selection::Uncommitted(project)), cx)
            });
            self.detail
                .update(cx, |detail, cx| detail.focus_message(window, cx));
        }
    }

    pub fn open_menu(
        &mut self,
        position: Point<Pixels>,
        items: Vec<MenuItem>,
        cx: &mut Context<Self>,
    ) {
        self.menu = Some(ContextMenu { position, items });
        cx.notify();
    }

    fn render_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.menu.as_ref()?;
        let theme = Theme::get(cx);
        let items = menu.items.iter().enumerate().map(|(ix, item)| {
            let handler = item.handler.clone();
            h_flex()
                .id(("menu-item", ix))
                .h(px(26.))
                .px(px(10.))
                .mx(px(4.))
                .rounded(px(4.))
                .text_size(px(13.))
                .text_color(if item.danger {
                    theme.danger
                } else {
                    theme.text_1
                })
                .hover(|el| el.bg(theme.hover()))
                .child(item.label.clone())
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.menu = None;
                    handler(this, window, cx);
                    cx.notify();
                }))
        });
        Some(
            deferred(
                anchored()
                    .position(menu.position)
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        v_flex()
                            .id("context-menu")
                            .min_w(px(180.))
                            .py(px(4.))
                            .rounded(px(8.))
                            .bg(theme.bg_1)
                            .border_1()
                            .border_color(theme.border_2)
                            .shadow_lg()
                            .children(items)
                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                this.menu = None;
                                cx.notify();
                            })),
                    ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    fn render_prompt(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
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
                    v_flex()
                        .id("prompt")
                        .w(px(380.))
                        .h_auto()
                        .p(px(16.))
                        .gap(px(12.))
                        .rounded(px(10.))
                        .bg(theme.bg_1)
                        .border_1()
                        .border_color(theme.border_2)
                        .shadow_lg()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child(prompt.title.clone()),
                        )
                        .when_some(prompt.detail.clone(), |el, detail| {
                            el.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(theme.text_2)
                                    .child(detail),
                            )
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
                        .child(
                            h_flex()
                                .justify_end()
                                .gap(px(8.))
                                .child(button("prompt-cancel", "Cancel").on_click({
                                    let input = input.clone();
                                    move |_, window, cx| {
                                        input.update(cx, |_, cx| cx.emit(InputEvent::Cancel));
                                        window.refresh();
                                    }
                                }))
                                .child(
                                    button("prompt-confirm", prompt.confirm.clone())
                                        .style(ButtonStyle::Pop)
                                        .on_click(move |_, _, cx| {
                                            input.update(cx, |_, cx| cx.emit(InputEvent::Submit))
                                        }),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_toasts(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let store = self.store.read(cx);
        v_flex()
            .absolute()
            .bottom(px(16.))
            .right(px(16.))
            .gap(px(8.))
            .items_end()
            .children(store.toasts.iter().map(|toast| {
                let id = toast.id;
                let error = toast.kind == ToastKind::Error;
                h_flex()
                    .id(("toast", id))
                    .max_w(px(460.))
                    .gap(px(8.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .bg(theme.bg_1)
                    .border_1()
                    .border_color(if error {
                        theme.danger.opacity(0.5)
                    } else {
                        theme.border_2
                    })
                    .shadow_lg()
                    .text_size(px(13.))
                    .when(error, |el| {
                        el.child(icon(Icon::Warning).text_color(theme.danger))
                    })
                    .child(div().flex_1().child(toast.message.clone()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.store
                            .update(cx, |store, cx| store.dismiss_toast(id, cx))
                    }))
            }))
    }
}

impl Focusable for AppView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.nav_focus.clone()
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        div()
            .id("root")
            .key_context("Workspace")
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::add_project))
            .on_action(cx.listener(Self::refresh))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::fetch))
            .on_action(cx.listener(Self::new_branch))
            .on_action(cx.listener(Self::push_selected))
            .on_action(cx.listener(Self::cut_selection))
            .on_action(cx.listener(Self::paste_here))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::focus_navigator))
            .on_action(cx.listener(Self::focus_filter))
            .on_action(cx.listener(Self::focus_message))
            .size_full()
            .flex()
            .flex_row()
            .bg(theme.bg_1)
            .text_color(theme.text_1)
            .font_family(theme.ui_font.clone())
            .text_size(px(13.))
            .line_height(px(18.))
            .child(self.render_navigator(window, cx))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .child(self.detail.clone()),
            )
            .child(self.render_toasts(cx))
            .children(self.render_prompt(cx))
            .children(self.render_menu(cx))
    }
}
