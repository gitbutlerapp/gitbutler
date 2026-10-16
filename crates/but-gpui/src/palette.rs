//! The command palette (⌘K): every action that applies where focus was, with its keys,
//! filtered as you type. It doubles as the list of shortcuts, as in Lite and Zed.

use gpui::{
    Action, AnyElement, ClickEvent, Context, Entity, FocusHandle, Focusable as _, KeyBinding,
    MouseButton, SharedString, Subscription, Window, actions, div, prelude::*, px,
};

use crate::app::{
    AddProject, AppView, CopySelection, CutSelection, DeleteSelection, Fetch, FocusChanges,
    FocusNavigator, NewBranch, NewBranchOnTop, OpenPullRequest, PasteHere, Push, Quit, Redo,
    Refresh, RenameSelection, StartCommit, ToggleFileTree, ToggleNavigator, ToggleZoom,
    UncommitSelection, Undo, UpdateFromTarget,
};
use crate::detail::{
    DiscardFile, DiscardHunk, NextFile, NextHunk, PreviousFile, PreviousHunk, ToggleFold,
    ToggleReviewed,
};
use crate::edits::{
    AddEmptyCommitAbove, AddEmptyCommitBelow, CherryPick, CopyChangeId, CopyMessage, MoveDown,
    MoveUp, NewBranchBelow, NewBranchHere, RewrapMessage, SquashAll, UnapplyStack, Unstack,
    UpdateStack,
};
use crate::state::Selection;
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{h_flex, keycap, list_row, popover, shortcut, v_flex};

actions!(palette, [PaletteUp, PaletteDown, GoToBranch]);

pub fn bind_keys(cx: &mut gpui::App) {
    cx.bind_keys([KeyBinding::new("cmd-p", GoToBranch, None)]);
    // Registered after the text field's own keys, so these win inside the palette.
    let c = Some("Palette > TextInput");
    cx.bind_keys([
        KeyBinding::new("up", PaletteUp, c),
        KeyBinding::new("down", PaletteDown, c),
    ]);
}

/// Every command, in the order they're listed before anything is typed.
fn commands() -> Vec<(&'static str, Box<dyn Action>)> {
    vec![
        ("Mark file reviewed", Box::new(ToggleReviewed)),
        ("Next hunk", Box::new(NextHunk)),
        ("Previous hunk", Box::new(PreviousHunk)),
        ("Next file", Box::new(NextFile)),
        ("Previous file", Box::new(PreviousFile)),
        ("Fold file", Box::new(ToggleFold)),
        ("Show files as a tree or a list", Box::new(ToggleFileTree)),
        ("Discard hunk", Box::new(DiscardHunk)),
        ("Discard file", Box::new(DiscardFile)),
        ("Commit…", Box::new(StartCommit)),
        ("Undo", Box::new(Undo)),
        ("Redo", Box::new(Redo)),
        ("Push branch", Box::new(Push)),
        ("Open pull request", Box::new(OpenPullRequest)),
        ("Fetch", Box::new(Fetch)),
        ("Update from target", Box::new(UpdateFromTarget)),
        ("New branch", Box::new(NewBranch)),
        ("New branch on top", Box::new(NewBranchOnTop)),
        ("Rename", Box::new(RenameSelection)),
        ("Uncommit", Box::new(UncommitSelection)),
        ("Move commit up", Box::new(MoveUp)),
        ("Move commit down", Box::new(MoveDown)),
        ("Cherry-pick commit", Box::new(CherryPick)),
        ("Add empty commit above", Box::new(AddEmptyCommitAbove)),
        ("Add empty commit below", Box::new(AddEmptyCommitBelow)),
        ("Squash all commits", Box::new(SquashAll)),
        ("New branch below", Box::new(NewBranchBelow)),
        ("New branch ending here", Box::new(NewBranchHere)),
        ("Unstack branch", Box::new(Unstack)),
        ("Unapply stack", Box::new(UnapplyStack)),
        ("Land branch on target", Box::new(crate::edits::LandBranch)),
        ("Update stack from target", Box::new(UpdateStack)),
        ("Rewrap commit message", Box::new(RewrapMessage)),
        ("Copy change ID", Box::new(CopyChangeId)),
        ("Copy commit message", Box::new(CopyMessage)),
        ("Discard or delete selection", Box::new(DeleteSelection)),
        ("Copy", Box::new(CopySelection)),
        ("Cut", Box::new(CutSelection)),
        ("Paste here", Box::new(PasteHere)),
        ("Toggle navigator", Box::new(ToggleNavigator)),
        ("Zoom diff", Box::new(ToggleZoom)),
        ("Go to navigator", Box::new(FocusNavigator)),
        ("Go to changes", Box::new(FocusChanges)),
        ("Add project…", Box::new(AddProject)),
        (
            "Clone a repository…",
            Box::new(crate::projects::CloneRepository),
        ),
        ("Settings", Box::new(crate::settings::OpenSettings)),
        ("Go to branch", Box::new(GoToBranch)),
        ("Find in diff", Box::new(crate::detail::FindInDiff)),
        ("Filter files", Box::new(crate::detail::FilterFiles)),
        ("Wrap long lines", Box::new(crate::detail::ToggleWrap)),
        ("Side-by-side diff", Box::new(crate::detail::ToggleSplit)),
        (
            "Show or hide the files",
            Box::new(crate::detail::ToggleFiles),
        ),
        (
            "Create pull request",
            Box::new(crate::pr::CreatePullRequest),
        ),
        ("Edit pull request", Box::new(crate::pr::EditPullRequest)),
        ("Merge pull request", Box::new(crate::pr::MergePullRequest)),
        (
            "Resolve conflicts",
            Box::new(crate::resolve::ResolveConflicts),
        ),
        (
            "Resolve conflicts by hand",
            Box::new(crate::resolve::EditByHand),
        ),
        ("Open in editor", Box::new(crate::open_in::OpenInEditor)),
        ("Open in terminal", Box::new(crate::open_in::OpenInTerminal)),
        ("Reload", Box::new(Refresh)),
        ("Quit", Box::new(Quit)),
    ]
}

const RISKY: &[&str] = &[
    "Discard hunk",
    "Discard file",
    "Discard or delete selection",
    "Uncommit",
    "Squash all commits",
    "Unapply stack",
    "Land branch on target",
    "Push branch",
];

type Run = Box<dyn FnOnce(&mut AppView, &mut Window, &mut Context<AppView>)>;

/// A row of the palette: what it's called, what it says at its end (keys, or a detail like
/// the project a branch is in), and what picking it does.
pub struct Choice {
    label: SharedString,
    key: Option<SharedString>,
    detail: Option<SharedString>,
    run: Run,
}

impl Choice {
    pub fn new(
        label: impl Into<SharedString>,
        run: impl FnOnce(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            key: None,
            detail: None,
            run: Box::new(run),
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// The command palette, or another list to pick from the same way.
pub struct Palette {
    input: Entity<TextInput>,
    choices: Vec<Choice>,
    /// What Enter does, for the hint at the bottom.
    verb: &'static str,
    selected: usize,
    restore_focus: Option<FocusHandle>,
    _subscription: Subscription,
}

/// How well `query` matches `label`: the start of a word beats the middle of one, which
/// beats a scattered match, and earlier beats later. `None` when the letters don't appear
/// in order.
fn score(label: &str, query: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let label = label.to_lowercase();
    let query = query.to_lowercase();
    if let Some((at, _)) = label
        .match_indices(&query)
        .find(|(at, _)| *at == 0 || label[..*at].ends_with(' '))
    {
        return Some(2000 - at as i32);
    }
    if let Some(at) = label.find(&query) {
        return Some(1000 - at as i32);
    }
    let mut chars = label.chars();
    query
        .chars()
        .all(|wanted| chars.any(|c| c == wanted))
        .then_some(0)
}

impl AppView {
    pub fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut commands: Vec<(&'static str, Box<dyn Action>)> = commands()
            .into_iter()
            .filter(|(_, action)| window.is_action_available(action.as_ref(), cx))
            .collect();
        // Recently run commands first, as in Zed.
        commands.sort_by_key(|(label, _)| {
            self.recent_commands
                .iter()
                .position(|recent| recent == label)
                .unwrap_or(usize::MAX)
        });
        let choices = commands
            .into_iter()
            .map(|(label, action)| {
                let key = shortcut(action.as_ref(), cx);
                Choice {
                    key,
                    ..Choice::new(label, move |this, window, cx| {
                        // Recent commands come first, so ⌘K then Enter repeats one; never
                        // one that throws work away.
                        if !RISKY.contains(&label) {
                            this.recent_commands.retain(|recent| *recent != label);
                            this.recent_commands.insert(0, label);
                        }
                        window.dispatch_action(action, cx);
                    })
                }
            })
            .collect();
        self.open_picker("Search commands", "Run", choices, window, cx);
    }

    /// Pick a branch of any loaded project by name, and select it.
    pub fn go_to_branch(&mut self, _: &GoToBranch, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        let several = store
            .projects
            .iter()
            .filter(|p| p.workspace.is_some())
            .count()
            > 1;
        let choices = store
            .projects
            .iter()
            .filter_map(|entry| Some((entry, entry.workspace.as_ref()?)))
            .flat_map(|(entry, workspace)| {
                workspace
                    .branches()
                    .filter(|branch| !branch.ref_name.is_empty())
                    .map(move |branch| {
                        let selection =
                            Selection::Branch(entry.id.clone(), branch.ref_name.clone());
                        let choice = Choice::new(branch.name.clone(), move |this, window, cx| {
                            this.select(selection, window, cx)
                        });
                        if several {
                            choice.detail(entry.project.name.clone())
                        } else {
                            choice
                        }
                    })
            })
            .collect();
        self.open_picker("Go to branch", "Go", choices, window, cx);
    }

    /// Show `choices` to filter by typing and pick one with Enter or a click.
    pub fn open_picker(
        &mut self,
        placeholder: &'static str,
        verb: &'static str,
        choices: Vec<Choice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let restore_focus = window.focused(cx);
        let input = cx.new(|cx| TextInput::new(placeholder, false, cx));
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Changed => {
                    if let Some(palette) = &mut this.palette {
                        palette.selected = 0;
                    }
                    cx.notify();
                }
                InputEvent::Submit => this.run_selected(window, cx),
                InputEvent::Cancel => this.close_palette(window, cx),
            });
        window.focus(&input.focus_handle(cx), cx);
        self.menu = None;
        self.palette = Some(Palette {
            input,
            choices,
            verb,
            selected: 0,
            restore_focus,
            _subscription: subscription,
        });
        cx.notify();
    }

    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(palette) = self.palette.take()
            && let Some(focus) = palette.restore_focus
        {
            window.focus(&focus, cx);
        }
        cx.notify();
    }

    fn matches(&self, cx: &gpui::App) -> Vec<usize> {
        let Some(palette) = &self.palette else {
            return Vec::new();
        };
        let query = palette.input.read(cx).text().trim().to_owned();
        let mut scored: Vec<(i32, usize)> = palette
            .choices
            .iter()
            .enumerate()
            .filter_map(|(ix, choice)| score(&choice.label, &query).map(|score| (score, ix)))
            .collect();
        // Stable, so equal scores keep the recent-first order.
        scored.sort_by_key(|(score, _)| -score);
        scored.into_iter().map(|(_, ix)| ix).collect()
    }

    fn run_command(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(palette) = self.palette.take() else {
            return;
        };
        let Some(choice) = palette.choices.into_iter().nth(ix) else {
            return;
        };
        // Run it where it was offered.
        if let Some(focus) = palette.restore_focus {
            window.focus(&focus, cx);
        }
        (choice.run)(self, window, cx);
        cx.notify();
    }

    fn run_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.palette.as_ref().map_or(0, |p| p.selected);
        if let Some(&ix) = self.matches(cx).get(selected) {
            self.run_command(ix, window, cx);
        }
    }

    fn palette_up(&mut self, _: &PaletteUp, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(palette) = &mut self.palette {
            palette.selected = palette.selected.saturating_sub(1);
            cx.notify();
        }
    }

    fn palette_down(&mut self, _: &PaletteDown, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.matches(cx).len();
        if let Some(palette) = &mut self.palette {
            palette.selected = (palette.selected + 1).min(count.saturating_sub(1));
            cx.notify();
        }
    }

    /// Lite's picker dialog: near the top, a search row, 28px rows, and a footer of hints.
    pub fn render_palette(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let palette = self.palette.as_ref()?;
        let theme = Theme::get(cx).clone();
        let matches = self.matches(cx);
        let rows: Vec<AnyElement> = matches
            .iter()
            .enumerate()
            .map(|(row, &ix)| {
                let choice = &palette.choices[ix];
                let selected = row == palette.selected;
                list_row(("command", ix), selected, &theme)
                    .gap(px(12.))
                    .child(div().flex_1().truncate().child(choice.label.clone()))
                    .when_some(choice.detail.clone(), |el, detail| {
                        el.child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.text_2)
                                .child(detail),
                        )
                    })
                    .when_some(choice.key.clone(), |el, key| el.child(keycap(&key, &theme)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.run_command(ix, window, cx)
                    }))
                    .into_any_element()
            })
            .collect();
        let empty = rows.is_empty();
        Some(
            div()
                .id("palette-backdrop")
                .absolute()
                .inset_0()
                // Over a modal, a click outside closes only the palette.
                .occlude()
                .flex()
                .justify_center()
                .items_start()
                .pt(px(72.))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| this.close_palette(window, cx)),
                )
                .child(
                    popover(&theme)
                        .id("palette")
                        .key_context("Palette")
                        .on_action(cx.listener(Self::palette_up))
                        .on_action(cx.listener(Self::palette_down))
                        .w(px(520.))
                        .max_h(px(440.))
                        .overflow_hidden()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .flex_none()
                                .h(px(36.))
                                .px(px(12.))
                                .flex()
                                .items_center()
                                .border_b_1()
                                .border_color(theme.border_3)
                                .child(palette.input.clone()),
                        )
                        .child(
                            v_flex()
                                .id("palette-rows")
                                .flex_1()
                                .min_h(px(0.))
                                .py(px(4.))
                                .overflow_y_scroll()
                                .children(rows)
                                .when(empty, |el| {
                                    el.child(
                                        div()
                                            .px(px(12.))
                                            .py(px(6.))
                                            .text_color(theme.text_2)
                                            .child("Nothing matches"),
                                    )
                                }),
                        )
                        .child(
                            h_flex()
                                .flex_none()
                                .h(px(36.))
                                .px(px(12.))
                                .gap(px(12.))
                                .border_t_1()
                                .border_color(theme.border_3)
                                .bg(theme.bg_2)
                                .text_size(px(12.))
                                .text_color(theme.text_2)
                                .child(
                                    h_flex()
                                        .gap(px(4.))
                                        .child(keycap("↑ ↓", &theme))
                                        .child("Move"),
                                )
                                .child(
                                    h_flex()
                                        .gap(px(4.))
                                        .child(keycap("↩", &theme))
                                        .child(palette.verb),
                                )
                                .child(
                                    h_flex()
                                        .gap(px(4.))
                                        .child(keycap("esc", &theme))
                                        .child("Close"),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
