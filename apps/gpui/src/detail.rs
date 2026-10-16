//! The right side: whatever is selected in the navigator, with its files and diff.
//! Uncommitted changes get the commit composer; a commit gets its message.

use std::collections::HashSet;

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, FocusHandle, Focusable, ListAlignment,
    ListOffset, ListState, MouseButton, MouseDownEvent, SharedString, Subscription, Task,
    WeakEntity, Window, div, list, prelude::*, px,
};

use crate::app::{AppView, MenuItem};
use crate::backend::{
    self, CommitState, Destination, DiffBody, DiffSource, FileChange, FileDiff, HunkKey, PushState,
    Selected,
};
use crate::state::{Cut, ProjectId, Selection, Store, StoreEvent, ToastKind};
use crate::theme::Theme;
use crate::transfer::short_ref;
use crate::ui::diff::{self, DiffRow};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{
    ButtonStyle, HEADER_HEIGHT, Icon, button, empty_state, h_flex, icon, relative_time, small_icon,
    v_flex,
};

const FILE_LIST_WIDTH: f32 = 300.;

#[derive(Debug, Clone, PartialEq)]
struct DiffKey {
    project: ProjectId,
    source: DiffSource,
}

struct LoadedDiff {
    key: DiffKey,
    files: Vec<FileDiff>,
    rows: Vec<DiffRow>,
}

pub struct Detail {
    store: Entity<Store>,
    /// For app-level actions like pushing, which live on the root view.
    app: WeakEntity<AppView>,
    focus: FocusHandle,
    diff: Option<LoadedDiff>,
    diff_error: Option<SharedString>,
    loading: Option<DiffKey>,
    /// The worktree changed while its diff was loading; load again when done.
    reload_queued: bool,
    _load: Task<()>,
    list: ListState,
    message: Entity<TextInput>,
    /// The branch (full ref) new commits go to; `None` creates a branch.
    commit_target: Option<String>,
    /// Uncommitted paths left out of the next commit.
    unchecked: HashSet<String>,
    /// Hunks left out of the next commit, in files that are otherwise in.
    unchecked_hunks: HashSet<(String, HunkKey)>,
    /// The commit message being edited, with the subscription that saves it.
    rewording: Option<(Entity<TextInput>, Subscription)>,
    /// The project the checkboxes and commit target above belong to.
    checks_project: Option<ProjectId>,
    _subscriptions: Vec<Subscription>,
}

impl Detail {
    pub fn new(
        store: Entity<Store>,
        app: WeakEntity<AppView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let message = cx.new(|cx| TextInput::new("Commit message", true, cx));
        let subscriptions = vec![
            cx.subscribe(&store, |this, _, event, cx| match event {
                StoreEvent::SelectionChanged => {
                    this.rewording = None;
                    let project = this.selected_project(cx);
                    if this.checks_project != project {
                        this.checks_project = project;
                        this.unchecked.clear();
                        this.unchecked_hunks.clear();
                        this.commit_target = None;
                    }
                    this.refresh_diff(true, cx);
                }
                StoreEvent::Loaded(project) => {
                    if this.selected_project(cx).as_ref() == Some(project) {
                        this.refresh_diff(false, cx);
                    }
                }
            }),
            cx.subscribe_in(&message, window, |this, _, event, window, cx| match event {
                InputEvent::Submit => this.commit(window, cx),
                InputEvent::Cancel => {
                    window.blur(cx);
                    cx.notify();
                }
                InputEvent::Changed => cx.notify(),
            }),
            cx.observe(&store, |_, _, cx| cx.notify()),
        ];
        let mut detail = Self {
            store,
            app,
            focus: cx.focus_handle(),
            diff: None,
            diff_error: None,
            loading: None,
            reload_queued: false,
            _load: Task::ready(()),
            list: ListState::new(0, ListAlignment::Top, px(600.)),
            message,
            commit_target: None,
            unchecked: HashSet::new(),
            unchecked_hunks: HashSet::new(),
            rewording: None,
            checks_project: None,
            _subscriptions: subscriptions,
        };
        detail.refresh_diff(true, cx);
        detail
    }

    fn selection(&self, cx: &App) -> Option<Selection> {
        self.store.read(cx).selection.clone()
    }

    fn selected_project(&self, cx: &App) -> Option<ProjectId> {
        self.selection(cx).map(|s| s.project().clone())
    }

    fn key_for_selection(&self, cx: &App) -> Option<DiffKey> {
        let selection = self.selection(cx)?;
        let store = self.store.read(cx);
        let source = match &selection {
            Selection::Project(_) => return None,
            Selection::Uncommitted(_) => DiffSource::Worktree,
            Selection::Commit(_, id) => DiffSource::Commit(*id),
            Selection::Branch(project, ref_name) => {
                let branch = store.workspace(project)?.find_branch(ref_name)?;
                let newest = branch.commits.first()?.id;
                let oldest = branch.commits.last()?.id;
                DiffSource::Range { oldest, newest }
            }
        };
        Some(DiffKey {
            project: selection.project().clone(),
            source,
        })
    }

    /// Load the diff for the current selection. `selection_changed` resets the scroll;
    /// otherwise only a worktree diff, which can change under us, is reloaded.
    fn refresh_diff(&mut self, selection_changed: bool, cx: &mut Context<Self>) {
        let key = self.key_for_selection(cx);
        let current = self.diff.as_ref().map(|diff| &diff.key);
        let stale = match &key {
            None => true,
            Some(key) => current != Some(key) || key.source == DiffSource::Worktree,
        };
        if !stale && !selection_changed {
            return;
        }
        let Some(key) = key else {
            self.diff = None;
            self.loading = None;
            cx.notify();
            return;
        };
        if !selection_changed && self.loading.as_ref() == Some(&key) {
            self.reload_queued = true;
            return;
        }
        self.reload_queued = false;
        if current != Some(&key) {
            self.diff_error = None;
            if selection_changed {
                // Don't leave the previous selection's diff on screen while loading.
                self.diff = None;
                self.list.reset(0);
            }
        }
        let Some(path) = self.store.read(cx).path(&key.project) else {
            return;
        };
        self.loading = Some(key.clone());
        let source = key.source;
        self._load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || backend::load_diff(&path, source)).await;
            this.update(cx, |this, cx| {
                if this.loading.as_ref() != Some(&key) {
                    return;
                }
                this.loading = None;
                match result {
                    Ok(files) => {
                        let rows = diff::rows(&files);
                        let same = this.diff.as_ref().is_some_and(|diff| diff.key == key);
                        if same {
                            let top = this.list.logical_scroll_top();
                            this.list.reset(rows.len());
                            this.list.scroll_to(top);
                        } else {
                            this.list.reset(rows.len());
                        }
                        if key.source == DiffSource::Worktree {
                            let present: HashSet<&str> =
                                files.iter().map(|f| f.file.path.as_str()).collect();
                            this.unchecked
                                .retain(|path| present.contains(path.as_str()));
                            let hunks: HashSet<(&str, HunkKey)> = files
                                .iter()
                                .flat_map(|f| {
                                    hunk_keys(f)
                                        .into_iter()
                                        .map(|key| (f.file.path.as_str(), key))
                                })
                                .collect();
                            this.unchecked_hunks
                                .retain(|(path, key)| hunks.contains(&(path.as_str(), *key)));
                        }
                        this.diff = Some(LoadedDiff { key, files, rows });
                        this.diff_error = None;
                    }
                    Err(err) => this.diff_error = Some(format!("{err:#}").into()),
                }
                if std::mem::take(&mut this.reload_queued) {
                    this.refresh_diff(false, cx);
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// What dragging `project`'s uncommitted changes carries: the checked files when
    /// they're shown here, otherwise all of them.
    pub fn files_to_move(&self, project: &ProjectId, cx: &App) -> Vec<FileChange> {
        if self.selection(cx) == Some(Selection::Uncommitted(project.clone())) {
            return self.checked_uncommitted(cx);
        }
        self.store
            .read(cx)
            .workspace(project)
            .map(|ws| ws.uncommitted.clone())
            .unwrap_or_default()
    }

    /// Uncommitted files that are checked for the next commit.
    pub fn checked_uncommitted(&self, cx: &App) -> Vec<FileChange> {
        let Some(Selection::Uncommitted(project)) = self.selection(cx) else {
            return Vec::new();
        };
        let Some(workspace) = self.store.read(cx).workspace(&project) else {
            return Vec::new();
        };
        workspace
            .uncommitted
            .iter()
            .filter(|file| !self.unchecked.contains(&file.path))
            .cloned()
            .collect()
    }

    /// The checked uncommitted changes: checked files, narrowed to their checked hunks.
    pub fn selected_changes(&self, cx: &App) -> Vec<Selected> {
        self.checked_uncommitted(cx)
            .into_iter()
            .filter_map(|file| {
                let hunks = self.worktree_hunks(&file.path);
                let picked: Vec<HunkKey> = hunks
                    .iter()
                    .filter(|key| !self.unchecked_hunks.contains(&(file.path.clone(), **key)))
                    .copied()
                    .collect();
                if picked.is_empty() && !hunks.is_empty() {
                    return None;
                }
                let partial = picked.len() < hunks.len();
                Some(Selected {
                    file,
                    hunks: partial.then_some(picked),
                })
            })
            .collect()
    }

    fn worktree_hunks(&self, path: &str) -> Vec<HunkKey> {
        self.diff
            .as_ref()
            .filter(|diff| diff.key.source == DiffSource::Worktree)
            .and_then(|diff| diff.files.iter().find(|f| f.file.path == path))
            .map(hunk_keys)
            .unwrap_or_default()
    }

    fn file_check(&self, path: &str) -> Check {
        if self.unchecked.contains(path) {
            return Check::None;
        }
        let excluded = self
            .unchecked_hunks
            .iter()
            .filter(|(hunk_path, _)| hunk_path == path)
            .count();
        if excluded == 0 {
            Check::All
        } else {
            Check::Some
        }
    }

    fn hunk_checked(&self, path: &str, key: HunkKey) -> bool {
        !self.unchecked.contains(path) && !self.unchecked_hunks.contains(&(path.to_owned(), key))
    }

    fn toggle_hunk(&mut self, path: &str, key: HunkKey, cx: &mut Context<Self>) {
        let entry = (path.to_owned(), key);
        if self.unchecked.remove(path) {
            // The whole file was out; bring back only this hunk.
            for other in self.worktree_hunks(path) {
                if other != key {
                    self.unchecked_hunks.insert((path.to_owned(), other));
                }
            }
        } else if !self.unchecked_hunks.remove(&entry) {
            self.unchecked_hunks.insert(entry);
        }
        let hunks = self.worktree_hunks(path);
        if hunks
            .iter()
            .all(|key| self.unchecked_hunks.contains(&(path.to_owned(), *key)))
        {
            self.unchecked_hunks
                .retain(|(hunk_path, _)| hunk_path != path);
            self.unchecked.insert(path.to_owned());
        }
        cx.notify();
    }

    pub fn focus_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.message.focus_handle(cx), cx);
        cx.notify();
    }

    /// Get `files` ready to commit to `branch`, after they were dropped on it.
    pub fn stage(
        &mut self,
        branch: String,
        files: &[FileChange],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_target = Some(branch);
        if let Some(project) = self.selected_project(cx)
            && let Some(workspace) = self.store.read(cx).workspace(&project)
        {
            let keep: HashSet<&str> = files.iter().map(|f| f.path.as_str()).collect();
            self.unchecked = workspace
                .uncommitted
                .iter()
                .filter(|f| !keep.contains(f.path.as_str()))
                .map(|f| f.path.clone())
                .collect();
        }
        self.focus_message(window, cx);
    }

    pub fn start_reword(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Commit(project, id)) = self.selection(cx) else {
            return;
        };
        let Some(message) = self
            .store
            .read(cx)
            .workspace(&project)
            .and_then(|ws| ws.find_commit(id))
            .map(|(_, commit)| commit.message.clone())
        else {
            return;
        };
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Commit message", true, cx);
            input.set_text(message, cx);
            input
        });
        let subscription = cx.subscribe_in(
            &input,
            window,
            move |this, input, event, window, cx| match event {
                InputEvent::Submit => {
                    let message = input.read(cx).text().trim().to_owned();
                    this.rewording = None;
                    let project = project.clone();
                    this.store.update(cx, |store, cx| {
                        store.run(
                            &project,
                            "Edit message",
                            move |path| backend::reword(path, id, &message),
                            |store, rewritten, cx| {
                                store.follow(&rewritten, cx);
                                store.toast(ToastKind::Info, "Message updated", cx)
                            },
                            cx,
                        )
                    });
                    window.focus(&this.focus, cx);
                }
                InputEvent::Cancel => {
                    this.rewording = None;
                    window.focus(&this.focus, cx);
                    cx.notify();
                }
                InputEvent::Changed => {}
            },
        );
        window.focus(&input.focus_handle(cx), cx);
        self.rewording = Some((input, subscription));
        cx.notify();
    }

    fn effective_target(&self, project: &ProjectId, cx: &App) -> Option<String> {
        let workspace = self.store.read(cx).workspace(project)?;
        if let Some(target) = &self.commit_target
            && workspace.find_branch(target).is_some()
        {
            return Some(target.clone());
        }
        workspace
            .stacks
            .first()
            .and_then(|stack| stack.branches.first())
            .filter(|branch| !branch.ref_name.is_empty())
            .map(|branch| branch.ref_name.clone())
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Selection::Uncommitted(project)) = self.selection(cx) else {
            return;
        };
        let message = self.message.read(cx).text().trim().to_owned();
        if message.is_empty() {
            self.store.update(cx, |store, cx| {
                store.toast(ToastKind::Info, "Write a commit message first", cx)
            });
            self.focus_message(window, cx);
            return;
        }
        let diff_shown = self.diff.as_ref().is_some_and(|diff| {
            diff.key.source == DiffSource::Worktree && diff.key.project == project
        });
        if !self.unchecked_hunks.is_empty() && !diff_shown {
            // Hunk choices are only known against a loaded diff.
            self.store.update(cx, |store, cx| {
                store.toast(
                    ToastKind::Info,
                    "Still loading the diff, try again in a moment",
                    cx,
                )
            });
            return;
        }
        let files = self.selected_changes(cx);
        if files.is_empty() {
            self.store.update(cx, |store, cx| {
                store.toast(ToastKind::Info, "Nothing checked to commit", cx)
            });
            return;
        }
        let target = self.effective_target(&project, cx);
        let message_input = self.message.clone();
        self.store.update(cx, |store, cx| {
            store.run(
                &project,
                "Commit",
                move |path| {
                    let branch = match target {
                        Some(branch) => branch,
                        None => backend::create_branch(path, None)?,
                    };
                    let id = backend::commit(
                        path,
                        &Destination::Branch(branch.clone()),
                        &files,
                        &message,
                    )?;
                    Ok((branch, id))
                },
                move |store, (branch, (_id, left_out)), cx| {
                    message_input.update(cx, |input, cx| input.set_text("", cx));
                    store.report_left_out(
                        &format!("Committed to {}", short_ref(&branch)),
                        &left_out,
                        cx,
                    );
                },
                cx,
            )
        });
    }

    fn toggle_checked(&mut self, path: &str, cx: &mut Context<Self>) {
        let check = self.file_check(path);
        self.unchecked_hunks
            .retain(|(hunk_path, _)| hunk_path != path);
        match check {
            Check::All | Check::Some => {
                self.unchecked.insert(path.to_owned());
            }
            Check::None => {
                self.unchecked.remove(path);
            }
        }
        cx.notify();
    }

    fn scroll_to_file(&mut self, file_ix: usize, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            return;
        };
        if let Some(row) = diff
            .rows
            .iter()
            .position(|row| matches!(row, DiffRow::File(ix) if *ix == file_ix))
        {
            self.list.scroll_to(ListOffset {
                item_ix: row,
                offset_in_item: px(0.),
            });
            cx.notify();
        }
    }
}

impl Focusable for Detail {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Detail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let selection = self.selection(cx);
        let body: AnyElement = match &selection {
            None => {
                let has_projects = !self.store.read(cx).projects.is_empty();
                empty_state(
                    if has_projects {
                        "Nothing selected"
                    } else {
                        "Welcome to GitButler"
                    },
                    if has_projects {
                        "Pick a project, branch or commit on the left."
                    } else {
                        "Add a repository with ⌘O to get started."
                    },
                    &theme,
                )
            }
            Some(Selection::Project(project)) => self.render_project(project, &theme, cx),
            Some(selection) => {
                let side = match selection {
                    Selection::Uncommitted(project) => {
                        self.render_uncommitted_side(project, &theme, window, cx)
                    }
                    Selection::Commit(project, id) => {
                        self.render_commit_side(project, *id, &theme, cx)
                    }
                    Selection::Branch(project, ref_name) => {
                        self.render_branch_side(project, ref_name, &theme, cx)
                    }
                    Selection::Project(_) => unreachable!("handled above"),
                };
                h_flex()
                    .size_full()
                    .items_start()
                    .child(
                        v_flex()
                            .w(px(FILE_LIST_WIDTH))
                            .flex_none()
                            .h_full()
                            .border_r_1()
                            .border_color(theme.border_3)
                            .child(side),
                    )
                    .child(self.render_diff(&theme, cx))
                    .into_any_element()
            }
        };
        v_flex()
            .id("detail")
            .key_context("Detail")
            .track_focus(&self.focus)
            .size_full()
            .bg(theme.bg_1)
            .child(self.render_header(selection.as_ref(), &theme, cx))
            .child(div().flex_1().min_h(px(0.)).child(body))
    }
}

impl Detail {
    fn render_header(
        &self,
        selection: Option<&Selection>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let store = self.store.read(cx);
        let mut crumbs: Vec<SharedString> = Vec::new();
        let mut actions: Vec<AnyElement> = Vec::new();
        if let Some(selection) = selection
            && let Some(entry) = store.project(selection.project())
        {
            crumbs.push(entry.project.name.clone().into());
            let workspace = entry.workspace.as_ref();
            match selection {
                Selection::Project(_) => {}
                Selection::Uncommitted(_) => crumbs.push("Uncommitted changes".into()),
                Selection::Branch(project, ref_name) => {
                    crumbs.push(short_ref(ref_name).to_owned().into());
                    if let Some(branch) = workspace.and_then(|ws| ws.find_branch(ref_name)) {
                        let (project, ref_name) = (project.clone(), ref_name.clone());
                        if branch.push.can_push() {
                            let label = if branch.push == PushState::NeedsForce {
                                "Force push"
                            } else {
                                "Push"
                            };
                            let app = self.app.clone();
                            actions.push(
                                button("push", label)
                                    .icon(Icon::ArrowUp)
                                    .style(ButtonStyle::Pop)
                                    .disabled(store.busy.is_some())
                                    .tooltip("Push this branch and the ones below it  ⌘⇧P")
                                    .on_click(move |_, _, cx| {
                                        app.update(cx, |app, cx| {
                                            app.push(project.clone(), ref_name.clone(), cx)
                                        })
                                        .ok();
                                    })
                                    .into_any_element(),
                            );
                        }
                    }
                }
                Selection::Commit(project, id) => {
                    if let Some((branch, commit)) = workspace.and_then(|ws| ws.find_commit(*id)) {
                        crumbs.push(branch.name.clone().into());
                        crumbs.push(commit.id.short().into());
                        let (p, id) = (project.clone(), *id);
                        let app = self.app.clone();
                        actions.push(
                            button("uncommit", "Uncommit")
                                .style(ButtonStyle::Ghost)
                                .disabled(store.busy.is_some())
                                .tooltip("Turn this commit back into uncommitted changes")
                                .on_click(move |_, _, cx| {
                                    app.update(cx, |app, cx| app.uncommit(p.clone(), id, cx))
                                        .ok();
                                })
                                .into_any_element(),
                        );
                        actions.push(
                            button("reword", "Edit message")
                                .style(ButtonStyle::Ghost)
                                .on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.start_reword(window, cx)
                                    }),
                                )
                                .into_any_element(),
                        );
                    }
                }
            }
        }
        let last = crumbs.len().saturating_sub(1);
        h_flex()
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .px(px(16.))
            .gap(px(6.))
            .border_b_1()
            .border_color(theme.border_3)
            .children(crumbs.into_iter().enumerate().flat_map(|(ix, crumb)| {
                let sep =
                    (ix > 0).then(|| div().text_color(theme.text_3).child("/").into_any_element());
                let el = div()
                    .when(ix == last, |el| {
                        el.font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.text_1)
                    })
                    .when(ix != last, |el| el.text_color(theme.text_2))
                    .truncate()
                    .child(crumb)
                    .into_any_element();
                sep.into_iter().chain([el])
            }))
            .child(div().flex_1())
            .children(actions)
    }

    fn render_project(&self, project: &ProjectId, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let Some(entry) = store.project(project) else {
            return empty_state("Project not found", "", theme);
        };
        let workspace = entry.workspace.as_ref();
        let expanded = entry.expanded;
        let id = entry.id.clone();
        let stat = |label: &str, value: String, color| {
            v_flex()
                .gap(px(2.))
                .px(px(14.))
                .py(px(10.))
                .min_w(px(120.))
                .rounded(px(6.))
                .border_1()
                .border_color(theme.border_3)
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(theme.text_2)
                        .child(label.to_owned()),
                )
                .child(div().text_size(px(20.)).text_color(color).child(value))
        };
        let mut stats = h_flex().gap(px(10.));
        if let Some(ws) = workspace {
            let branches = ws.branches().count();
            stats = stats
                .child(stat("Branches", branches.to_string(), theme.text_1))
                .child(stat(
                    "Uncommitted",
                    ws.uncommitted.len().to_string(),
                    if ws.uncommitted.is_empty() {
                        theme.text_1
                    } else {
                        theme.warn
                    },
                ))
                .child(stat(
                    "Unpushed",
                    ws.unpushed_commits().to_string(),
                    if ws.unpushed_commits() == 0 {
                        theme.text_1
                    } else {
                        theme.blue
                    },
                ));
            if let Some(target) = &ws.target {
                stats = stats.child(stat(
                    &format!("Behind {}", target.name),
                    target.behind.to_string(),
                    if target.behind == 0 {
                        theme.text_1
                    } else {
                        theme.warn
                    },
                ));
            }
        }
        v_flex()
            .p(px(24.))
            .gap(px(16.))
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(px(20.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(entry.project.name.clone()),
                    )
                    .child(
                        div()
                            .font_family(theme.mono_font.clone())
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child(entry.project.path.display().to_string()),
                    ),
            )
            .when_some(entry.error.clone(), |el, err| {
                el.child(
                    h_flex()
                        .gap(px(8.))
                        .p(px(10.))
                        .rounded(px(6.))
                        .bg(theme.danger.opacity(0.08))
                        .text_color(theme.danger)
                        .child(icon(Icon::Warning))
                        .child(div().flex_1().child(err)),
                )
            })
            .child(stats)
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(
                        button(
                            "toggle-project",
                            if expanded {
                                "Collapse"
                            } else {
                                "Show branches"
                            },
                        )
                        .style(if expanded {
                            ButtonStyle::Outline
                        } else {
                            ButtonStyle::Pop
                        })
                        .on_click(cx.listener({
                            let id = id.clone();
                            move |this, _, _, cx| {
                                this.store
                                    .update(cx, |store, cx| store.set_expanded(&id, !expanded, cx))
                            }
                        })),
                    )
                    .when(expanded, |el| {
                        let app = self.app.clone();
                        let fetch_id = id.clone();
                        el.child(
                            button("fetch", "Fetch")
                                .icon(Icon::Refresh)
                                .disabled(store.busy.is_some())
                                .on_click(move |_, _, cx| {
                                    app.update(cx, |app, cx| {
                                        app.fetch_project(fetch_id.clone(), cx)
                                    })
                                    .ok();
                                }),
                        )
                    })
                    .when_some(
                        workspace
                            .and_then(|ws| ws.target.as_ref())
                            .filter(|target| expanded && target.needs_update)
                            .map(|target| target.name.clone()),
                        |el, target| {
                            let app = self.app.clone();
                            let update_id = id.clone();
                            el.child(
                                button("update", format!("Update from {target}"))
                                    .icon(Icon::ArrowDown)
                                    .style(ButtonStyle::Pop)
                                    .disabled(store.busy.is_some())
                                    .on_click(move |_, _, cx| {
                                        app.update(cx, |app, cx| {
                                            app.update_project(update_id.clone(), cx)
                                        })
                                        .ok();
                                    }),
                            )
                        },
                    ),
            )
            .into_any_element()
    }

    #[expect(clippy::too_many_arguments)]
    fn render_file_row(
        &self,
        ix: usize,
        file: &FileChange,
        stats: Option<(u32, u32)>,
        checkbox: Option<Check>,
        drag_from: Option<(ProjectId, Option<backend::CommitId>)>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (name, dir) = diff::split_path(&file.path);
        let path = file.path.clone();
        let mut row = h_flex()
            .id(("file", ix))
            .h(px(28.))
            .px(px(10.))
            .mx(px(6.))
            .gap(px(8.))
            .rounded(px(4.))
            .hover(|el| el.bg(theme.hover()))
            .when_some(checkbox, |el, check| {
                let path = path.clone();
                el.child(
                    checkbox_box(("check", ix), check, theme).on_click(cx.listener(
                        move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.toggle_checked(&path, cx)
                        },
                    )),
                )
            })
            .child(
                div()
                    .flex_none()
                    .w(px(10.))
                    .text_size(px(11.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(diff::change_color(file.kind, theme))
                    .child(file.kind.letter()),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(6.))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_none()
                            .max_w(px(180.))
                            .truncate()
                            .child(name.to_owned()),
                    )
                    .child(
                        div()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(12.))
                            .text_color(theme.text_3)
                            .child(dir.to_owned()),
                    ),
            )
            .when_some(stats, |el, (added, removed)| {
                el.child(diff::stats(added, removed, theme))
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.scroll_to_file(ix, cx)));
        if let Some((project, from)) = drag_from {
            let payload = Cut::Files {
                project: project.clone(),
                from,
                files: vec![file.clone()],
            };
            let label: SharedString = file.path.clone().into();
            let menu_payload = payload.clone();
            row = row
                .on_drag(payload, move |payload, _, _, cx| {
                    let label = label.clone();
                    cx.new(|_| crate::navigator::DragPreview {
                        label,
                        payload: payload.clone(),
                    })
                })
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        this.file_menu(menu_payload.clone(), event, window, cx)
                    }),
                );
        }
        row.into_any_element()
    }

    fn file_menu(
        &mut self,
        payload: Cut,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let app = self.app.clone();
        let Cut::Files {
            project,
            from,
            files,
        } = payload.clone()
        else {
            return;
        };
        let mut items = vec![MenuItem::new("Cut", {
            let (project, files) = (project.clone(), files.clone());
            move |this: &mut AppView, _, cx| {
                this.cut_files(project.clone(), from, files.clone(), cx)
            }
        })];
        match from {
            None => items.push(
                MenuItem::new("Discard changes…", {
                    let (project, files) = (project.clone(), files.clone());
                    move |this, window, cx| {
                        let changes = files
                            .iter()
                            .map(|file| Selected {
                                file: file.clone(),
                                hunks: None,
                            })
                            .collect();
                        this.discard(project.clone(), changes, window, cx)
                    }
                })
                .danger(),
            ),
            Some(_) => items.push(MenuItem::new("Uncommit file", move |this, window, cx| {
                this.transfer(
                    &payload,
                    &Selection::Uncommitted(project.clone()),
                    false,
                    window,
                    cx,
                );
            })),
        }
        let position = event.position;
        app.update(cx, |app, cx| app.open_menu(position, items, cx))
            .ok();
    }

    fn render_uncommitted_side(
        &self,
        project: &ProjectId,
        theme: &Theme,
        window: &mut Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(workspace) = store.workspace(project) else {
            return empty_state("Loading…", "", theme);
        };
        let stats_by_path = |path: &str| {
            self.diff
                .as_ref()
                .filter(|d| d.key.source == DiffSource::Worktree)
                .and_then(|d| d.files.iter().find(|f| f.file.path == path))
                .map(FileDiff::stats)
        };
        let files: Vec<AnyElement> = workspace
            .uncommitted
            .iter()
            .enumerate()
            .map(|(ix, file)| {
                self.render_file_row(
                    ix,
                    file,
                    stats_by_path(&file.path),
                    Some(self.file_check(&file.path)),
                    Some((project.clone(), None)),
                    theme,
                    cx,
                )
            })
            .collect();
        let checked = workspace
            .uncommitted
            .iter()
            .filter(|f| self.file_check(&f.path) != Check::None)
            .count();
        let all_checked = self.unchecked.is_empty() && self.unchecked_hunks.is_empty();
        let target = self.effective_target(project, cx);
        let busy = store.busy.is_some();
        let message_focused = self.message.focus_handle(cx).is_focused(window);
        let branches: Vec<(String, String)> = workspace
            .branches()
            .filter(|b| !b.ref_name.is_empty())
            .map(|b| (b.ref_name.clone(), b.name.clone()))
            .collect();

        let list_header = h_flex()
            .h(px(36.))
            .px(px(16.))
            .gap(px(8.))
            .flex_none()
            .child(
                div()
                    .text_size(px(12.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(match workspace.uncommitted.len() {
                        0 => "No changes".to_owned(),
                        1 => "1 file changed".to_owned(),
                        n => format!("{n} files changed"),
                    }),
            )
            .child(div().flex_1())
            .when(!workspace.uncommitted.is_empty(), |el| {
                el.child(
                    button(
                        "toggle-all",
                        if all_checked {
                            "Uncheck all"
                        } else {
                            "Check all"
                        },
                    )
                    .small()
                    .style(ButtonStyle::Ghost)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if all_checked {
                            if let Some(Selection::Uncommitted(project)) = this.selection(cx)
                                && let Some(ws) = this.store.read(cx).workspace(&project)
                            {
                                this.unchecked =
                                    ws.uncommitted.iter().map(|f| f.path.clone()).collect();
                            }
                        } else {
                            this.unchecked.clear();
                        }
                        this.unchecked_hunks.clear();
                        cx.notify();
                    })),
                )
            });

        let composer = v_flex()
            .flex_none()
            .p(px(12.))
            .gap(px(8.))
            .border_t_1()
            .border_color(theme.border_3)
            .bg(theme.bg_1)
            .child(
                h_flex()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .text_color(theme.text_2)
                    .child("Commit to")
                    .child(self.render_target_picker(
                        project.clone(),
                        target.clone(),
                        branches,
                        theme,
                        cx,
                    )),
            )
            .child(
                div()
                    .id("message-box")
                    .min_h(px(84.))
                    .max_h(px(260.))
                    .overflow_y_scroll()
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(if message_focused {
                        theme.pop
                    } else {
                        theme.border_2
                    })
                    .bg(theme.bg_1)
                    .text_size(px(13.))
                    .line_height(px(19.))
                    .cursor_text()
                    .on_click(cx.listener(|this, _, window, cx| this.focus_message(window, cx)))
                    .child(self.message.clone()),
            )
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(11.))
                            .text_color(theme.text_3)
                            .child(if checked == 0 {
                                "Check files to commit".to_owned()
                            } else {
                                format!("{checked} of {} files", workspace.uncommitted.len())
                            }),
                    )
                    .child(
                        button("commit", "Commit")
                            .style(ButtonStyle::Pop)
                            .disabled(busy || checked == 0)
                            .tooltip("⌘↩")
                            .on_click(cx.listener(|this, _, window, cx| this.commit(window, cx))),
                    ),
            );

        v_flex()
            .size_full()
            .child(list_header)
            .child(
                div()
                    .id("uncommitted-files")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(files)
                    .when(workspace.uncommitted.is_empty(), |el| {
                        el.child(
                            div()
                                .px(px(16.))
                                .py(px(8.))
                                .text_size(px(12.))
                                .text_color(theme.text_2)
                                .child("Your working directory matches the last commit."),
                        )
                    }),
            )
            .when(!workspace.uncommitted.is_empty(), |el| el.child(composer))
            .into_any_element()
    }

    fn render_target_picker(
        &self,
        project: ProjectId,
        target: Option<String>,
        branches: Vec<(String, String)>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let label = target
            .as_deref()
            .map(short_ref)
            .map(str::to_owned)
            .unwrap_or_else(|| "a new branch".to_owned());
        let detail = cx.entity().downgrade();
        let app = self.app.clone();
        h_flex()
            .id("commit-target")
            .h(px(22.))
            .px(px(8.))
            .gap(px(4.))
            .rounded(px(4.))
            .bg(theme.fill_gray.opacity(0.08))
            .hover(|el| el.bg(theme.fill_gray.opacity(0.14)))
            .font_family(theme.mono_font.clone())
            .text_color(theme.blue)
            .child(small_icon(Icon::Branch))
            .child(label)
            .child(small_icon(Icon::ChevronDown).text_color(theme.text_2))
            .on_click(move |event, _, cx| {
                let mut items: Vec<MenuItem> = branches
                    .iter()
                    .map(|(ref_name, name)| {
                        let detail = detail.clone();
                        let ref_name = ref_name.clone();
                        MenuItem::new(name.clone(), move |_, _, cx| {
                            detail
                                .update(cx, |detail, cx| {
                                    detail.commit_target = Some(ref_name.clone());
                                    cx.notify();
                                })
                                .ok();
                        })
                    })
                    .collect();
                let project = project.clone();
                items.push(MenuItem::new("New branch…", move |this, window, cx| {
                    let project = project.clone();
                    this.store.update(cx, |store, cx| {
                        store.select(Some(Selection::Uncommitted(project)), cx)
                    });
                    this.new_branch(&crate::app::NewBranch, window, cx);
                }));
                let position = event.position();
                app.update(cx, |app, cx| app.open_menu(position, items, cx))
                    .ok();
            })
    }

    fn render_commit_side(
        &self,
        project: &ProjectId,
        id: backend::CommitId,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some((branch, commit)) = store.workspace(project).and_then(|ws| ws.find_commit(id))
        else {
            return empty_state("Commit not found", "", theme);
        };
        let body = commit
            .message
            .split_once('\n')
            .map(|(_, body)| body.trim().to_owned())
            .unwrap_or_default();
        let state = match commit.state {
            CommitState::Local => ("Not pushed", theme.blue),
            CommitState::Pushed => ("Pushed", theme.text_2),
            CommitState::Integrated => ("Integrated", theme.safe),
        };
        let files = self.render_diff_file_list(project, Some(id), theme, cx);
        let message: AnyElement = match &self.rewording {
            Some((input, _)) => v_flex()
                .gap(px(8.))
                .child(
                    div()
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(6.))
                        .border_1()
                        .border_color(theme.pop)
                        .line_height(px(19.))
                        .child(input.clone()),
                )
                .child(
                    h_flex()
                        .gap(px(8.))
                        .justify_end()
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(11.))
                                .text_color(theme.text_3)
                                .child("⌘↩ to save, esc to cancel"),
                        )
                        .child(
                            button("cancel-reword", "Cancel")
                                .small()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.rewording = None;
                                    cx.notify();
                                })),
                        )
                        .child({
                            let input = input.clone();
                            button("save-reword", "Save")
                                .small()
                                .style(ButtonStyle::Pop)
                                .on_click(move |_, _, cx| {
                                    input.update(cx, |_, cx| cx.emit(InputEvent::Submit))
                                })
                        }),
                )
                .into_any_element(),
            None => v_flex()
                .id("commit-message")
                .gap(px(6.))
                .child(
                    div()
                        .text_size(px(15.))
                        .line_height(px(21.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(if commit.title.is_empty() {
                            "No message".to_owned()
                        } else {
                            commit.title.clone()
                        }),
                )
                .when(!body.is_empty(), |el| {
                    el.child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .text_color(theme.text_2)
                            .child(body),
                    )
                })
                .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                    if event.click_count() == 2 {
                        this.start_reword(window, cx)
                    }
                }))
                .into_any_element(),
        };
        v_flex()
            .size_full()
            .child(
                v_flex()
                    .flex_none()
                    .p(px(16.))
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(theme.border_3)
                    .child(message)
                    .when(commit.conflicted, |el| {
                        el.child(
                            h_flex()
                                .gap(px(8.))
                                .px(px(10.))
                                .py(px(8.))
                                .rounded(px(6.))
                                .bg(theme.danger.opacity(0.08))
                                .text_size(px(12.))
                                .text_color(theme.danger)
                                .child(small_icon(Icon::Warning))
                                .child(div().flex_1().child(
                                    "This commit has conflicts. Resolve them with `but resolve` or the GitButler app, or undo with ⌘Z.",
                                )),
                        )
                    })
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .flex_wrap()
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child(commit.author.clone())
                            .child("·")
                            .child(relative_time(commit.time))
                            .child("·")
                            .child(div().text_color(state.1).child(state.0))
                            .when(commit.conflicted, |el| {
                                el.child("·")
                                    .child(div().text_color(theme.danger).child("Conflicted"))
                            }),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .text_size(px(12.))
                            .child(small_icon(Icon::Branch).text_color(theme.text_2))
                            .child(
                                div()
                                    .font_family(theme.mono_font.clone())
                                    .text_color(theme.blue)
                                    .child(branch.name.clone()),
                            )
                            .child(div().flex_1())
                            .child(
                                div()
                                    .font_family(theme.mono_font.clone())
                                    .text_color(theme.text_3)
                                    .child(commit.id.short()),
                            ),
                    ),
            )
            .child(files)
            .into_any_element()
    }

    fn render_branch_side(
        &self,
        project: &ProjectId,
        ref_name: &str,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let store = self.store.read(cx);
        let Some(branch) = store
            .workspace(project)
            .and_then(|ws| ws.find_branch(ref_name))
        else {
            return empty_state("Branch not found", "", theme);
        };
        let push = match branch.push {
            PushState::NothingToPush => "Up to date with its remote",
            PushState::Unpushed => "Has unpushed commits",
            PushState::NeedsForce => "Rewritten, needs a force push",
            PushState::NeverPushed => "Not pushed yet",
            PushState::Integrated => "Integrated",
        };
        let commits: Vec<AnyElement> = branch
            .commits
            .iter()
            .enumerate()
            .map(|(ix, commit)| {
                let selection = Selection::Commit(project.clone(), commit.id);
                let store = self.store.clone();
                h_flex()
                    .id(("branch-commit", ix))
                    .h(px(28.))
                    .px(px(10.))
                    .mx(px(6.))
                    .gap(px(8.))
                    .rounded(px(4.))
                    .hover(|el| el.bg(theme.hover()))
                    .child(
                        small_icon(match commit.state {
                            CommitState::Local => Icon::Commit,
                            CommitState::Pushed => Icon::CommitFill,
                            CommitState::Integrated => Icon::Tick,
                        })
                        .text_color(
                            if commit.state == CommitState::Local {
                                theme.blue
                            } else {
                                theme.text_3
                            },
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .child(commit.title.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(11.))
                            .text_color(theme.text_3)
                            .child(relative_time(commit.time)),
                    )
                    .on_click(move |_, _, cx| {
                        store.update(cx, |store, cx| store.select(Some(selection.clone()), cx))
                    })
                    .into_any_element()
            })
            .collect();
        let files = self.render_diff_file_list(project, None, theme, cx);
        v_flex()
            .size_full()
            .child(
                v_flex()
                    .flex_none()
                    .p(px(16.))
                    .gap(px(6.))
                    .border_b_1()
                    .border_color(theme.border_3)
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .child(icon(Icon::Branch).text_color(theme.text_2))
                            .child(
                                div()
                                    .font_family(theme.mono_font.clone())
                                    .text_size(px(14.))
                                    .text_color(theme.blue)
                                    .truncate()
                                    .child(branch.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.text_2)
                            .child(format!("{} commits · {push}", branch.commits.len())),
                    ),
            )
            .child(
                div()
                    .id("branch-commits")
                    .flex_none()
                    .max_h(px(260.))
                    .overflow_y_scroll()
                    .py(px(4.))
                    .border_b_1()
                    .border_color(theme.border_3)
                    .children(commits)
                    .when(branch.commits.is_empty(), |el| {
                        el.child(div().px(px(16.)).py(px(6.)).text_size(px(12.)).text_color(theme.text_2).child(
                            "No commits yet. Drop uncommitted files here to commit to this branch.",
                        ))
                    }),
            )
            .child(files)
            .into_any_element()
    }

    /// The files of the loaded commit or branch diff, each a jump target and draggable
    /// when it belongs to a single commit.
    fn render_diff_file_list(
        &self,
        project: &ProjectId,
        commit: Option<backend::CommitId>,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> AnyElement {
        let Some(diff) = &self.diff else {
            return div()
                .px(px(16.))
                .py(px(10.))
                .text_size(px(12.))
                .text_color(theme.text_2)
                .child(if self.loading.is_some() {
                    "Loading…"
                } else {
                    ""
                })
                .into_any_element();
        };
        let rows: Vec<AnyElement> = diff
            .files
            .iter()
            .enumerate()
            .map(|(ix, file)| {
                self.render_file_row(
                    ix,
                    &file.file,
                    Some(file.stats()),
                    None,
                    commit.map(|id| (project.clone(), Some(id))),
                    theme,
                    cx,
                )
            })
            .collect();
        v_flex()
            .flex_1()
            .min_h(px(0.))
            .child(
                h_flex()
                    .h(px(36.))
                    .px(px(16.))
                    .flex_none()
                    .text_size(px(12.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(match diff.files.len() {
                        1 => "1 file changed".to_owned(),
                        n => format!("{n} files changed"),
                    }),
            )
            .child(
                div()
                    .id("diff-files")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .children(rows),
            )
            .into_any_element()
    }

    fn render_diff(&self, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        if let Some(err) = &self.diff_error {
            return div()
                .flex_1()
                .p(px(24.))
                .text_color(theme.danger)
                .child(err.clone())
                .into_any_element();
        }
        let Some(diff) = &self.diff else {
            return div()
                .flex_1()
                .h_full()
                .bg(theme.bg_2)
                .child(empty_state(
                    if self.loading.is_some() {
                        "Loading diff…"
                    } else {
                        ""
                    },
                    "",
                    theme,
                ))
                .into_any_element();
        };
        if diff.files.is_empty() {
            let (title, detail) = match diff.key.source {
                DiffSource::Worktree => (
                    "Nothing to commit",
                    "Edits show up here as soon as they're saved.",
                ),
                _ => ("No changes", ""),
            };
            return div()
                .flex_1()
                .h_full()
                .bg(theme.bg_2)
                .child(empty_state(title, detail, theme))
                .into_any_element();
        }
        let entity = cx.entity().downgrade();
        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .bg(theme.bg_2)
            .pt(px(12.))
            .child(
                list(self.list.clone(), move |ix, _window, cx| {
                    let Some(detail) = entity.upgrade() else {
                        return div().into_any_element();
                    };
                    let entity = detail.downgrade();
                    let detail = detail.read(cx);
                    let theme = Theme::get(cx);
                    let Some(diff) = detail.diff.as_ref().filter(|diff| ix < diff.rows.len())
                    else {
                        return div().into_any_element();
                    };
                    let row = diff.rows[ix];
                    let selectable = diff.key.source == DiffSource::Worktree;
                    let hunk_of = |file: usize, hunk: usize| match &diff.files[file].body {
                        DiffBody::Hunks { hunks, .. } => {
                            Some((diff.files[file].file.path.clone(), hunks[hunk].key))
                        }
                        _ => None,
                    };
                    let (leading, excluded) = match row {
                        DiffRow::Hunk { file, hunk } if selectable => match hunk_of(file, hunk) {
                            Some((path, key)) => {
                                let checked = detail.hunk_checked(&path, key);
                                let check = if checked { Check::All } else { Check::None };
                                let leading = checkbox_box(("hunk", ix), check, theme)
                                    .on_click(move |_, _, cx| {
                                        entity
                                            .update(cx, |detail, cx| {
                                                detail.toggle_hunk(&path, key, cx)
                                            })
                                            .ok();
                                    })
                                    .into_any_element();
                                (Some(leading), !checked)
                            }
                            None => (None, false),
                        },
                        DiffRow::Line { file, hunk, .. } if selectable => (
                            None,
                            hunk_of(file, hunk)
                                .is_some_and(|(path, key)| !detail.hunk_checked(&path, key)),
                        ),
                        _ => (None, false),
                    };
                    let element = diff::render_row(row, &diff.files, theme, leading, excluded);
                    match (row, &detail.diff) {
                        (DiffRow::Hunk { file, hunk }, Some(diff)) if selectable => hunk_menu(
                            element,
                            ix,
                            &diff.files[file],
                            hunk,
                            diff.key.project.clone(),
                            detail.app.clone(),
                        ),
                        _ => element,
                    }
                })
                .size_full(),
            )
            .into_any_element()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Check {
    All,
    Some,
    None,
}

fn hunk_keys(file: &FileDiff) -> Vec<HunkKey> {
    match &file.body {
        DiffBody::Hunks { hunks, .. } => hunks.iter().map(|hunk| hunk.key).collect(),
        _ => Vec::new(),
    }
}

fn checkbox_box(
    id: impl Into<gpui::ElementId>,
    check: Check,
    theme: &Theme,
) -> gpui::Stateful<gpui::Div> {
    let on = check != Check::None;
    div()
        .id(id)
        .flex_none()
        .size(px(14.))
        .rounded(px(3.))
        .border_1()
        .border_color(if on { theme.pop } else { theme.border_2 })
        .when(on, |el| el.bg(theme.pop))
        .flex()
        .items_center()
        .justify_center()
        .map(|el| match check {
            Check::All => el.child(
                small_icon(Icon::Tick)
                    .size(px(10.))
                    .text_color(gpui::white()),
            ),
            Check::Some => el.child(div().w(px(8.)).h(px(2.)).rounded(px(1.)).bg(gpui::white())),
            Check::None => el,
        })
}

/// Right-click on a hunk header offers to discard just that hunk.
fn hunk_menu(
    element: AnyElement,
    ix: usize,
    file: &FileDiff,
    hunk: usize,
    project: ProjectId,
    app: WeakEntity<AppView>,
) -> AnyElement {
    let DiffBody::Hunks { hunks, .. } = &file.body else {
        return element;
    };
    let change = Selected {
        file: file.file.clone(),
        hunks: Some(vec![hunks[hunk].key]),
    };
    div()
        .id(("hunk-menu", ix))
        .child(element)
        .on_mouse_down(MouseButton::Right, move |event, _, cx| {
            let (project, change) = (project.clone(), change.clone());
            let item = MenuItem::new("Discard hunk…", move |this, window, cx| {
                this.discard(project.clone(), vec![change.clone()], window, cx)
            })
            .danger();
            app.update(cx, |app, cx| app.open_menu(event.position, vec![item], cx))
                .ok();
        })
        .into_any_element()
}
