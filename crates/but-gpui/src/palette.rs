//! The command palette (⌘K): every action that applies where focus was, with its keys,
//! filtered as you type. It doubles as the list of shortcuts, as in Lite and Zed.

use gpui::{
    Action, AnyElement, ClickEvent, Context, Entity, FocusHandle, Focusable as _, KeyBinding,
    ScrollHandle, SharedString, Subscription, Window, actions, div, prelude::*, px,
};

use crate::app::{
    AddProject, AppView, CopySelection, CutSelection, DeleteSelection, Fetch, FocusChanges,
    FocusDiff, FocusNavigator, NewBranch, NewBranchOnTop, OpenPullRequest, PasteHere, Push, Quit,
    Redo, RenameSelection, StartCommit, ToggleFileTree, ToggleNavigator, UncommitSelection, Undo,
    UpdateFromTarget,
};
use crate::detail::{
    DiscardFile, DiscardHunk, NextFile, NextHunk, PreviousFile, PreviousHunk, ToggleFold,
};
use crate::edits::{
    AddEmptyCommitAbove, AddEmptyCommitBelow, CherryPick, CopyChangeId, CopyMessage,
    NewBranchBelow, NewBranchHere, RewrapMessage, SquashAll, UnapplyStack, Unstack,
};
use crate::overlays::backdrop;
use crate::state::{Notice, Selection};
use crate::theme::Theme;
use crate::ui::scrollbar::{Scroll, scrollbar};
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{h_flex, keycap, list_row, muted, popover, shortcut, v_flex};

actions!(palette, [PaletteUp, PaletteDown, GoToBranch, ShowActions]);

pub fn bind_keys(cx: &mut gpui::App) {
    cx.bind_keys([
        KeyBinding::new("cmd-p", GoToBranch, None),
        KeyBinding::new("m", ShowActions, Some("Workspace && !TextInput")),
    ]);
    // Registered after the text field's own keys, so these win inside the palette.
    let c = Some("Palette > TextInput");
    cx.bind_keys([
        KeyBinding::new("up", PaletteUp, c),
        KeyBinding::new("down", PaletteDown, c),
        KeyBinding::new("ctrl-p", PaletteUp, c),
        KeyBinding::new("ctrl-n", PaletteDown, c),
    ]);
}

/// Every command, in the order they're listed before anything is typed.
const COMMANDS: &[(&str, &dyn Action)] = &[
    ("Next hunk", &NextHunk),
    ("Previous hunk", &PreviousHunk),
    ("Next file", &NextFile),
    ("Previous file", &PreviousFile),
    ("Fold or unfold file", &ToggleFold),
    ("Show files as a tree or a list", &ToggleFileTree),
    ("Discard hunk", &DiscardHunk),
    ("Discard file", &DiscardFile),
    ("Commit…", &StartCommit),
    ("Undo", &Undo),
    ("Redo", &Redo),
    ("Push branch", &Push),
    ("Open pull request", &OpenPullRequest),
    ("Fetch", &Fetch),
    ("Update from target", &UpdateFromTarget),
    ("New branch", &NewBranch),
    ("New branch on top", &NewBranchOnTop),
    ("Rename branch or edit message", &RenameSelection),
    ("Uncommit", &UncommitSelection),
    ("Cherry-pick commit", &CherryPick),
    ("Add empty commit above", &AddEmptyCommitAbove),
    ("Add empty commit below", &AddEmptyCommitBelow),
    ("Squash all commits", &SquashAll),
    ("Squash the picked commits", &crate::edits::SquashPicked),
    (
        "Move commit to a new branch",
        &crate::edits::MoveToNewBranch,
    ),
    ("Pull from the remote", &crate::edits::PullRemote),
    (
        "Replace with the remote's version",
        &crate::edits::TakeRemote,
    ),
    ("New branch below", &NewBranchBelow),
    ("New branch ending here", &NewBranchHere),
    ("Unstack branch", &Unstack),
    ("Unapply branch or stack", &UnapplyStack),
    ("Land on target", &crate::edits::LandBranch),
    ("Rewrap commit message", &RewrapMessage),
    ("Copy change ID", &CopyChangeId),
    ("Copy commit message", &CopyMessage),
    ("Discard or delete selection", &DeleteSelection),
    ("Copy", &CopySelection),
    ("Cut", &CutSelection),
    ("Paste here", &PasteHere),
    ("Toggle navigator", &ToggleNavigator),
    ("Go to navigator", &FocusNavigator),
    ("Go to files", &FocusChanges),
    ("Go to diff", &FocusDiff),
    ("Add project…", &AddProject),
    ("Clone a repository…", &crate::projects::CloneRepository),
    ("Settings", &crate::settings::OpenSettings),
    ("Go to branch", &GoToBranch),
    ("Find in diff", &crate::detail::FindInDiff),
    ("Filter files", &crate::detail::FilterFiles),
    ("Wrap long lines", &crate::detail::ToggleWrap),
    ("Toggle diff backgrounds", &crate::detail::TogglePlainLines),
    ("Side-by-side diff", &crate::detail::ToggleSplit),
    ("Show or hide the files", &crate::detail::ToggleFiles),
    ("Create pull request", &crate::pr::CreatePullRequest),
    ("Merge pull request", &crate::pr::MergePullRequest),
    ("Resolve conflicts", &crate::detail::ResolveConflicts),
    ("Open in editor", &crate::open_in::OpenInEditor),
    ("Open in terminal", &crate::open_in::OpenInTerminal),
    ("Quit", &Quit),
];

const RISKY: &[&str] = &[
    "Discard hunk",
    "Discard file",
    "Discard or delete selection",
    "Uncommit",
    "Squash all commits",
    "Squash the picked commits",
    "Replace with the remote's version",
    "Unapply branch or stack",
    "Land on target",
    "Push branch",
    "Merge pull request",
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
    /// Where its choices act, as [`Store::place`] says, beside the input.
    place: Option<SharedString>,
    choices: Vec<Choice>,
    /// What Enter does, for the hint at the bottom.
    verb: &'static str,
    selected: usize,
    /// The rows' scroll, which follows the selection.
    scroll: ScrollHandle,
    pub(crate) restore_focus: Option<FocusHandle>,
    /// What it opened on: see [`Store::acting_on`].
    pub acting_on: (Option<Selection>, usize),
    /// Its choices hold what they act on, as a menu's do, rather than read it when run.
    pub holds_ids: bool,
    _subscription: Subscription,
}

impl Palette {
    /// Select the row at `row`, scrolling it into view.
    fn select(&mut self, row: usize) {
        self.selected = row;
        self.scroll.scroll_to_item(row);
    }
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
        let mut commands: Vec<(&'static str, &dyn Action)> = (COMMANDS.iter().copied())
            .filter(|(_, action)| window.is_action_available(*action, cx))
            .collect();
        // Recently run commands first, as in Zed.
        commands.sort_by_key(|(label, _)| {
            self.recent_commands
                .iter()
                .position(|recent| recent == label)
                .unwrap_or(usize::MAX)
        });
        let mut choices: Vec<Choice> = commands
            .into_iter()
            .map(|(label, action)| {
                let key = shortcut(action, cx);
                let action = action.boxed_clone();
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
        // What an error card offers to try instead, as Update anyway, found by typing: last,
        // so ⌘K then Enter never runs one.
        choices.extend((self.store.read(cx).toasts.iter()).filter_map(
            |toast| match &toast.notice {
                Notice::Error {
                    title,
                    retry: Some((label, run)),
                    ..
                } => {
                    let (id, run) = (toast.id, run.clone());
                    let choice = Choice::new(label.clone(), move |this, _, cx| {
                        this.store.update(cx, |store, cx| {
                            if store.take_toast(id, cx) {
                                run(store, cx)
                            }
                        })
                    });
                    Some(choice.detail(title.clone()))
                }
                _ => None,
            },
        ));
        self.open_picker(None, "Search commands", "Run", choices, window, cx);
    }

    /// Pick a branch of any loaded project by name or pull request title, and select it.
    pub fn go_to_branch(&mut self, _: &GoToBranch, window: &mut Window, cx: &mut Context<Self>) {
        let store = self.store.read(cx);
        // Only expanded projects: a collapsed one's rows aren't drawn, or kept current.
        let open = || {
            (store.projects.iter())
                .filter(|entry| entry.expanded)
                .filter_map(|entry| Some((entry, entry.workspace.as_ref()?)))
        };
        let several = open().count() > 1;
        let choices = open()
            .flat_map(|(entry, workspace)| {
                workspace
                    .branches()
                    .filter(|branch| !branch.ref_name.is_empty())
                    .map(move |branch| {
                        let (id, ref_name) = (entry.id.clone(), branch.ref_name.clone());
                        // As the navigator names it: a pull request by its title, its
                        // branch's name beside it, where typing either finds it.
                        let label = store.label_of(&entry.id, branch).to_owned();
                        let name = (label != branch.name).then_some(branch.name.as_str());
                        let project = several.then_some(entry.project.name.as_str());
                        let detail = name.into_iter().chain(project).collect::<Vec<_>>();
                        let detail = detail.join(" · ");
                        let gone = format!("{} was renamed or deleted meanwhile", branch.name);
                        let choice = Choice::new(label, move |this, window, cx| {
                            // Gone since the picker opened, as an agent renamed it.
                            if (this.store.read(cx).workspace(&id))
                                .is_none_or(|ws| ws.find_branch(&ref_name).is_none())
                            {
                                return this.store.update(cx, |store, cx| store.snack(gone, cx));
                            }
                            this.select(Selection::Branch(id, ref_name), window, cx)
                        });
                        match detail.is_empty() {
                            true => choice,
                            false => choice.detail(detail),
                        }
                    })
            })
            .collect();
        self.open_picker(None, "Go to branch", "Go", choices, window, cx);
    }

    /// What right-clicking the selected row offers, to pick from with the keys.
    pub fn show_actions(&mut self, _: &ShowActions, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.store.read(cx).selection.clone() else {
            return;
        };
        // On the Branches and Operations history pages, what's opened there.
        let detail = self.detail.read(cx);
        let items = match &selection {
            Selection::Branches(project) => (detail.selected_other_branch(project))
                .map(|branch| detail.branch_menu_items(project, &branch.ref_name, cx)),
            Selection::History(project) => (detail.selected_operation(project)).map(|operation| {
                crate::detail::Detail::operation_menu_items(project, operation.id)
            }),
            _ => None,
        };
        let items = items.unwrap_or_else(|| self.navigator.read(cx).menu_items(&selection, cx));
        let choices: Vec<Choice> = (items.into_iter())
            .map(|item| Choice {
                label: item.label,
                key: item.key,
                detail: item.detail,
                run: Box::new(move |this, window, cx| (item.handler)(this, window, cx)),
            })
            .collect();
        if choices.is_empty() {
            return;
        }
        let place = self.store.read(cx).place(&selection);
        self.open_picker(Some(place), "Actions", "Run", choices, window, cx);
        if let Some(palette) = &mut self.palette {
            palette.holds_ids = true;
        }
    }

    /// Show `choices` to filter by typing and pick one with Enter or a click.
    pub fn open_picker(
        &mut self,
        place: Option<SharedString>,
        placeholder: &'static str,
        verb: &'static str,
        choices: Vec<Choice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Opened from another picker, go back to where that one came from.
        let restore_focus = match &self.palette {
            Some(open) => open.restore_focus.clone(),
            None => window.focused(cx),
        };
        let input = cx.new(|cx| TextInput::new(placeholder, false, cx));
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Changed => {
                    if let Some(palette) = &mut this.palette {
                        palette.select(0);
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
            place,
            choices,
            verb,
            selected: 0,
            scroll: ScrollHandle::new(),
            restore_focus,
            acting_on: self.store.read(cx).acting_on(),
            holds_ids: false,
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
            .filter_map(|(ix, choice)| {
                let detail = (choice.detail.as_ref()).and_then(|detail| score(detail, &query));
                let score = score(&choice.label, &query).max(detail)?;
                // One that throws work away shows for its words, not for letters scattered
                // through it, so typing a command missing here doesn't land on it.
                let scattered = score == 0 && !query.is_empty();
                (!scattered || !RISKY.contains(&&*choice.label)).then_some((score, ix))
            })
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
            palette.select(palette.selected.saturating_sub(1));
            cx.notify();
        }
    }

    fn palette_down(&mut self, _: &PaletteDown, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.matches(cx).len();
        if let Some(palette) = &mut self.palette {
            palette.select((palette.selected + 1).min(count.saturating_sub(1)));
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
                        el.child(muted(detail, &theme))
                    })
                    .when_some(choice.key.clone(), |el, key| el.child(keycap(&key, &theme)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.run_command(ix, window, cx)
                    }))
                    .into_any_element()
            })
            .collect();
        let empty = rows.is_empty();
        let card = popover(&theme)
            .id("palette")
            .key_context("Palette")
            .on_action(cx.listener(Self::palette_up))
            .on_action(cx.listener(Self::palette_down))
            .w(px(520.))
            .max_h(px(440.))
            .overflow_hidden()
            .child(
                div()
                    .flex_none()
                    .h(px(36.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(theme.border_3)
                    .child(div().flex_1().min_w(px(0.)).child(palette.input.clone()))
                    .children((palette.place.clone()).map(|place| muted(place, &theme))),
            )
            .child(
                v_flex()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .child(
                        v_flex()
                            .id("palette-rows")
                            .flex_1()
                            .min_h(px(0.))
                            .py(px(4.))
                            .overflow_y_scroll()
                            .track_scroll(&palette.scroll)
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
                    .children(scrollbar(
                        "palette-scrollbar",
                        Scroll::Div(palette.scroll.clone()),
                        &theme,
                    )),
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
                    .children(
                        [("↑ ↓", "Move"), ("↩", palette.verb), ("esc", "Close")].map(
                            |(key, hint)| {
                                h_flex().gap(px(4.)).child(keycap(key, &theme)).child(hint)
                            },
                        ),
                    ),
            );
        // Over a modal, a click outside closes only the palette.
        let backdrop = backdrop("palette-backdrop", card, cx, Self::close_palette);
        Some(backdrop.items_start().pt(px(72.)).into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::{COMMANDS, RISKY};

    #[test]
    fn every_risky_name_is_a_command() {
        for risky in RISKY {
            assert!(
                COMMANDS.iter().any(|(label, _)| label == risky),
                "{risky:?} must match a command's label, or ⌘K then Enter could repeat it"
            );
        }
    }
}
