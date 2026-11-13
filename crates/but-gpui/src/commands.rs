//! What the app can do, as ⌘K and the right-click menus offer it: each command's one title,
//! whether it throws work away, how a menu words it for the selection, and when it's offered.

use gpui::{Action, App, Window};

use crate::app::{
    ActualSize, AddProject, AppView, ChangeTheme, CheckForUpdates, CopySelection, Fetch,
    FocusChanges, FocusDiff, FocusNavigator, FocusTerminals, NewBranch, NewBranchOnTop,
    OpenPullRequest, Pull, Push, Quit, Redo, StartAmend, StartCommit, ToggleDetail, ToggleFileTree,
    ToggleNavigator, ToggleNavigatorPlace, ToggleTerminalOverview, Undo, UpdateFromTarget,
    UpdateWorkspace, UpdateWorktrees, ZoomIn, ZoomOut,
};
use crate::backend::{FullName, Workspace};
use crate::detail::{FoldFile, NextFile, NextHunk, PreviousFile, PreviousHunk, UnfoldFile};
use crate::edits::{
    AddEmptyCommitAbove, AddEmptyCommitBelow, BranchEdits, CherryPick, CopyChangeId, CopyMessage,
    Discard, NameBranch, NewBranchBelow, NewBranchHere, RenameBranch, RenameTerminal, Reword,
    RewrapMessage, SquashAll, StackOn, UnapplyStack, Uncommit, Unstack,
};
use crate::palette::{CopyMore, GoTo, GoToTerminal};
use crate::state::Selection;
use crate::ui::{commit_title, plural, quoted};

/// What the palette calls the action named `name`, or its name in words: `navigator::SelectNext`
/// is "Select next".
pub(crate) fn title_of(name: &str) -> String {
    if let Some(command) = COMMANDS.iter().find(|c| c.name() == name) {
        return command.title().to_owned();
    }
    let last = name.rsplit("::").next().unwrap_or(name);
    let mut words = String::new();
    for (ix, c) in last.char_indices() {
        if c.is_uppercase() && ix > 0 {
            words.push(' ');
            words.extend(c.to_lowercase());
        } else {
            words.push(c);
        }
    }
    words
}

/// An action the palette and the menus offer, by the one title they all call it.
pub(crate) trait Command: Action {
    fn title(&self) -> &'static str;

    /// It throws work away, so ⌘K then Enter never repeats it, and letters scattered through
    /// its title don't find it.
    fn risky(&self) -> bool {
        false
    }

    /// How a right-click menu says it for the selection, with what it acts on in the sentence:
    /// "Pull 3 commits from the remote" for Pull from the remote. It keeps the title's verb, so
    /// what a menu says is what ⌘K finds. The title when `None`.
    fn phrase(&self, _app: &AppView, _window: &Window, _cx: &App) -> Option<String> {
        None
    }

    /// Whether ⌘K and the menus offer it where its handler is registered: not where it could
    /// only say why it can't run, as Push with nothing to push, whose key still says so.
    fn listed(&self, _app: &AppView, _cx: &App) -> bool {
        true
    }

    /// Other names typing finds it by in ⌘K and Settings › Keyboard, as other commands word the
    /// same kind of thing: Remove branch for Delete branch. Never shown.
    fn aliases(&self) -> &'static [&'static str] {
        &[]
    }
}

/// Gives each action its title, `risky` after the ones that throw work away, and between braces
/// the functions of the app that word it for a menu (`phrase`, or `phrase_at` where that
/// depends on what has the keys) and say whether it's offered
/// (`listed`), and the other names it's found by (`aliases`), and lists them as [`COMMANDS`], in
/// that order.
macro_rules! commands {
    ($(
        $action:path => $title:tt $($risky:ident)? $({ $($option:ident: $method:expr),* $(,)? })?
    ),* $(,)?) => {
        $(impl Command for $action {
            fn title(&self) -> &'static str {
                commands!(@title $title)
            }
            $(fn $risky(&self) -> bool {
                true
            })?
            $($(commands!(@option $option $method);)*)?
        })*

        /// Every command, in the order they're listed before anything is typed.
        pub(crate) const COMMANDS: &[&dyn Command] = &[$(&$action),*];
    };
    // A title is a string, or a constant between parentheses, as one that differs by platform.
    (@title ($title:expr)) => {
        $title
    };
    (@title $title:literal) => {
        $title
    };
    (@option phrase $method:expr) => {
        fn phrase(&self, app: &AppView, _window: &Window, cx: &App) -> Option<String> {
            let phrase: fn(&AppView, &App) -> Option<String> = $method;
            phrase(app, cx)
        }
    };
    // A phrase that depends on where the keys are.
    (@option phrase_at $method:expr) => {
        fn phrase(&self, app: &AppView, window: &Window, cx: &App) -> Option<String> {
            let phrase: fn(&AppView, &Window, &App) -> Option<String> = $method;
            phrase(app, window, cx)
        }
    };
    (@option listed $method:expr) => {
        fn listed(&self, app: &AppView, cx: &App) -> bool {
            let listed: fn(&AppView, &App) -> bool = $method;
            listed(app, cx)
        }
    };
    (@option aliases $aliases:expr) => {
        fn aliases(&self) -> &'static [&'static str] {
            $aliases
        }
    };
}

commands! {
    NextHunk => "Next hunk",
    PreviousHunk => "Previous hunk",
    NextFile => "Next file",
    PreviousFile => "Previous file",
    FoldFile => "Fold file",
    UnfoldFile => "Unfold file",
    ToggleFileTree => "Show files as a tree or a list",
    Discard => "Discard" risky {
        aliases: &[
            "Discard commit",
            "Remove commit",
            "Delete commit",
            "Discard changes",
            "Discard hunk",
            "Discard lines",
            "Discard file",
            "Delete branch",
            "Remove branch",
            "Discard branch",
            "Close terminal",
            "Remove terminal",
            "Delete terminal",
            "Discard terminal",
            "Delete worktree",
            "Remove worktree",
            "Discard worktree",
        ],
        listed: deletable,
    },
    crate::detail::CheckAll => "Check all files",
    crate::detail::UncheckAll => "Uncheck all files",
    crate::detail::MarkResolved => "Mark resolved",
    crate::detail::UncommitChanges => "Uncommit changes" risky {
        phrase: |app, cx| Some(format!("Uncommit {}", app.detail.read(cx).changes_kind(cx)?)),
    },
    crate::detail::SplitIntoNewCommit => "Split into a new commit",
    StartCommit => "Commit",
    StartAmend => "Amend",
    Undo => "Undo",
    Redo => "Redo",
    Push => "Push" risky { phrase: push_phrase, listed: pushable },
    OpenPullRequest => "Open pull request" { listed: has_pull_request },
    Fetch => "Fetch",
    Pull => "Pull",
    UpdateFromTarget => "Update workspace and worktrees" { listed: target_moved },
    UpdateWorkspace => "Update workspace" { listed: has_target },
    UpdateWorktrees => "Update worktrees" { listed: has_target },
    NewBranch => "New branch",
    NewBranchOnTop => "New branch above",
    crate::app::NewWorktree => "New worktree" { listed: has_target },
    crate::app::NewWorktreeWithName => "New worktree with name" { listed: has_target },
    crate::app::RecycleWorktree => "Recycle worktree" {
        listed: has_target,
        aliases: &["Reuse worktree"],
    },
    Reword => "Reword" {
        aliases: &["Reword commit", "Rename commit", "Edit commit message"],
    },
    RenameBranch => "Rename branch" {
        aliases: &["Reword branch", "Edit branch name"],
    },
    NameBranch => "Name branch",
    // What a terminal's menu called Reword before it had a command of its own.
    RenameTerminal => "Rename terminal" { aliases: &["Reword terminal"] },
    Uncommit => "Uncommit" risky {
        phrase: |app, cx| picks(app, cx).map(|n| format!("Uncommit {n} commits")),
    },
    CherryPick => "Cherry-pick" {
        // The workspace's commits picked, or the Branches page's.
        phrase: |app, cx| {
            let n = picks(app, cx).or_else(|| app.detail.read(cx).branch_cherry_picks())?;
            (n > 1).then(|| format!("Cherry-pick {n} commits"))
        },
    },
    AddEmptyCommitAbove => "Add empty commit above" {
        // On a branch, above is on top: it's the only place there.
        phrase: |app, cx| on_branch(app, cx).then(|| "Add empty commit".to_owned()),
    },
    AddEmptyCommitBelow => "Add empty commit below",
    SquashAll => "Squash all commits" risky,
    crate::edits::SquashPicked => "Squash picked commits" risky { phrase: squash_picked_phrase },
    crate::edits::SquashIntoSelected => "Squash picked commits into this one" risky {
        phrase: |app, cx| picks(app, cx).map(|n| format!("Squash {n} commits into this one")),
    },
    crate::edits::MoveToNewBranch => "Move to a new branch" {
        phrase: |app, cx| picks(app, cx).map(|n| format!("Move {n} commits to a new branch")),
    },
    crate::edits::PullRemote => "Pull from the remote" { phrase: pull_phrase },
    crate::edits::TakeRemote => "Replace with the remote's version" risky,
    NewBranchBelow => "New branch below",
    NewBranchHere => "New branch ending here",
    StackOn => "Stack on",
    Unstack => "Unstack",
    UnapplyStack => "Unapply stack" risky { phrase: unapply_phrase },
    crate::edits::MergeBranch => "Merge into target" risky { phrase: merge_phrase },
    RewrapMessage => "Rewrap message",
    CopyChangeId => "Copy change ID",
    CopyMessage => "Copy message",
    CopySelection => "Copy",
    crate::app::CopyCommitId => "Copy commit ID",
    crate::app::CopyBranchName => "Copy branch name" {
        phrase: |_, _| Some("Copy name".to_owned()),
    },
    crate::app::CopyPullRequestLink => "Copy pull request link",
    crate::app::CopySnapshotId => "Copy snapshot ID",
    crate::app::CopyPath => "Copy path",
    crate::app::CopyRelativePath => "Copy relative path",
    crate::app::CopyAbsolutePath => "Copy absolute path",
    crate::app::CopyErrorMessage => "Copy error message",
    CopyMore => "Copy more",
    crate::app::MoveSelection => "Move" {
        phrase: |app, cx| picks(app, cx).map(|n| format!("Move {n} commits")),
    },
    crate::app::SquashSelection => "Squash" {
        phrase_at: squash_phrase,
        // What's uncommitted is amended instead.
        listed: |app, cx| !matches!(app.store.read(cx).selection, Some(Selection::Uncommitted(..))),
    },
    crate::app::SquashIntoSelection => "Squash uncommitted changes into this",
    ToggleNavigator => "Toggle navigator",
    ToggleNavigatorPlace => "Move navigator",
    ToggleDetail => "Toggle details",
    ToggleTerminalOverview => "Toggle terminal overview",
    ZoomIn => "Zoom in",
    ZoomOut => "Zoom out",
    ActualSize => "Actual size",
    FocusNavigator => "Go to navigator",
    FocusChanges => "Go to files",
    FocusDiff => "Go to diff",
    FocusTerminals => "Go to terminal overview",
    AddProject => "Add project",
    crate::projects::CloneRepository => "Clone a repository",
    crate::settings::OpenSettings => "Settings",
    ChangeTheme => "Change theme" { aliases: &["Colour scheme", "Color scheme"] },
    crate::settings::OpenBriefsSettings => "Briefs settings" { listed: briefs_off },
    crate::settings::OpenKeyboardShortcuts => "Keyboard shortcuts",
    crate::app::OperationsHistory => "Operations history",
    crate::app::NextProjectPage => "Next project tab" { aliases: &["Next project page"] },
    crate::app::PreviousProjectPage => "Previous project tab" {
        aliases: &["Previous project page"]
    },
    crate::app::NextNavigatorTab => "Next navigator tab" {
        aliases: &["Next worktree"],
        listed: navigator_on_top
    },
    crate::app::PreviousNavigatorTab => "Previous navigator tab" {
        aliases: &["Previous worktree"],
        listed: navigator_on_top
    },
    crate::app::FocusNextPane => "Next pane",
    crate::app::FocusPreviousPane => "Previous pane",
    crate::app::FocusPaneRight => "Pane to the right",
    crate::app::FocusPaneLeft => "Pane to the left",
    crate::app::FocusTileLeft => "Tile to the left" { aliases: &["Left split"] },
    crate::app::FocusTileRight => "Tile to the right" { aliases: &["Right split"] },
    crate::app::FocusTileUp => "Tile above" { aliases: &["Upper split"] },
    crate::app::FocusTileDown => "Tile below" { aliases: &["Lower split"] },
    crate::detail::SwitchTab => "Switch tab" {
        aliases: &["Switch between Changes and Pull request"]
    },
    crate::detail::PreviousTab => "Previous tab",
    crate::detail::ShowBrief => "Show brief" { listed: brief_here },
    crate::detail::ShowBranchesWithBriefs => "Show branches with a brief" {
        aliases: &["Find a brief"],
        listed: briefs_on,
    },
    crate::detail::ChangesBeside => "Show changes beside the brief" { listed: changes_can_go_beside },
    crate::detail::WriteNote => "Write a comment on this brief" { listed: brief_shown },
    crate::detail::SendNotes => "Send comments on this brief" { listed: brief_shown },
    crate::detail::NextBrief => "Next brief" { listed: brief_shown },
    crate::detail::PreviousBrief => "Previous brief" { listed: brief_shown },
    crate::app::CloseWindow => "Close window",
    GoTo => "Go to anything",
    GoToTerminal => "Go to a terminal",
    crate::detail::FindInDiff => "Find" { aliases: &["Find in diff", "Find in the brief"] },
    crate::detail::FilterFiles => "Filter files",
    crate::detail::ToggleWrap => "Wrap long lines",
    crate::detail::TogglePlainLines => "Toggle diff backgrounds",
    crate::detail::ToggleSplit => "Side-by-side diff",
    crate::detail::ToggleFiles => "Toggle files" {
        // In the single file diff the files stay beside the diff, and nothing hides them.
        listed: |app, cx| !app.detail.read(cx).single_file_diff(cx),
    },
    crate::pr::CreatePullRequest => "Create pull request" { listed: pull_request_creatable },
    crate::pr::MergePullRequest => "Merge pull request" risky {
        // As the forge's button says it, for how the project merges.
        phrase: |app, cx| {
            let store = app.store.read(cx);
            let method = crate::pr::merge_method(store, store.selection.as_ref()?.project());
            Some(crate::pr::Offer::Merge.label(method).to_owned())
        },
    },
    crate::pr::EnableAutoMerge => "Enable auto-merge",
    crate::pr::DisableAutoMerge => "Disable auto-merge",
    crate::pr::ConvertToDraft => "Convert to draft",
    crate::pr::MarkReadyForReview => "Ready for review",
    crate::detail::ResolveConflicts => "Resolve conflicts" {
        // A linked worktree's conflicts are resolved there.
        listed: |app, cx| {
            let store = app.store.read(cx);
            (store.selection.as_ref()).is_none_or(|s| !store.in_linked(s))
        },
    },
    crate::detail::ApplyBranch => "Apply branch",
    crate::detail::NewWorktreeFromBranch => "New worktree from branch" {
        // Of the branch the menu's on.
        phrase: |_, _| Some("New worktree".to_owned()),
    },
    crate::detail::EditDescription => "Edit branch description" {
        phrase: |_, _| Some("Edit description".to_owned()),
    },
    crate::detail::RestoreOperation => "Restore to before this operation" risky {
        phrase: |_, _| Some("Restore to before this".to_owned()),
    },
    crate::open_in::OpenInEditor => "Open in editor" {
        // The one chosen.
        phrase: |app, cx| Some(crate::open_in::editor_label(app.store.read(cx))),
    },
    crate::open_in::OpenInTerminal => "Open in terminal",
    crate::terminal::NewTerminal => "New terminal",
    crate::terminal::NewTerminalWith => "New terminal with command" {
        aliases: &["Open terminal with", "Run in new terminal", "New agent terminal"],
        // Short in a menu, beside "Open a file…".
        phrase: |_, _| Some("New terminal with…".to_owned()),
    },
    crate::app::SplitRight => "Split right" { aliases: &["Split vertically", "Pin to the right"] },
    crate::app::SplitBelow => "Split below" {
        aliases: &["Split down", "Split horizontally", "Pin below"],
    },
    crate::app::NewTerminalRight => "New terminal to the right" {
        aliases: &["Split terminal right", "Terminal to the right"],
    },
    crate::app::NewTerminalBelow => "New terminal below" {
        aliases: &["Split terminal below", "Split terminal down", "Terminal below"],
    },
    crate::app::NewTerminalWithRight => "New terminal with command to the right" {
        aliases: &["Run in new terminal to the right", "New agent terminal to the right"],
    },
    crate::app::NewTerminalWithBelow => "New terminal with command below" {
        aliases: &["Run in new terminal below", "New agent terminal below"],
    },
    crate::app::EqualizeTiles => "Equalize tiles" {
        aliases: &["Equalize splits", "Even out tiles", "Balance tiles", "Balance splits"],
    },
    crate::app::ZoomTile => "Zoom tile" {
        aliases: &["Zoom split", "Maximize tile", "Maximize split"],
    },
    crate::app::ArrangeTiles => "Arrange tiles" {
        aliases: &["Move tile", "Move split", "Rearrange tiles", "Rearrange splits"],
    },
    crate::app::SwapTiles => "Swap tiles" {
        aliases: &["Swap splits", "Exchange tiles", "Exchange splits"],
    },
    crate::app::NextTile => "Next tile" { aliases: &["Next split"] },
    crate::app::PreviousTile => "Previous tile" { aliases: &["Previous split"] },
    crate::app::CopyTileDiagram => "Copy tile layout as a diagram" {
        aliases: &["Copy splits as ASCII", "Copy tiling diagram"],
    },
    crate::navigator::Watch => "Watch",
    crate::navigator::StopWatching => "Stop watching" { aliases: &["Unwatch"] },
    crate::navigator::UnarchiveWorktree => "Unarchive worktree",
    crate::navigator::Reveal => (crate::ui::REVEAL),
    crate::navigator::OpenProjectSettings => "Project settings",
    crate::navigator::ArchiveWorktree => "Archive worktree" {
        phrase: |_, _| Some("Archive".to_owned()),
    },
    // Under their headings in the view menus, they're the choices.
    crate::navigator::SortProjectsByActivity => "Sort projects by last activity" {
        phrase: |_, _| Some("Last activity".to_owned()),
    },
    crate::navigator::SortProjectsByName => "Sort projects by name" {
        phrase: |_, _| Some("Name".to_owned()),
    },
    crate::navigator::ShowAllProjects => "Show all projects" {
        phrase: |_, _| Some("All".to_owned()),
    },
    crate::navigator::ShowActiveProjects => "Show active projects" {
        phrase: |_, _| Some("Active in the last 7 days".to_owned()),
    },
    crate::detail::ShowOnlyLocalBranches => "Show only local branches" {
        phrase: |_, _| Some("Local branches".to_owned()),
    },
    crate::detail::ShowRemoteBranches => "Show remote branches" {
        phrase: |_, _| Some("Remote branches".to_owned()),
        aliases: &["Show all remote branches"],
    },
    crate::detail::ShowBranchesWithoutCommits => "Show branches without unique commits" {
        aliases: &["Include empty branches"],
    },
    // The menu bar's, for Windows, which has none.
    CheckForUpdates => "Check for updates",
    crate::app::BuildAndRestart => "Build and restart",
    crate::app::BuildAndRestartFromHere => "Build and restart from here",
    crate::command_editor::OpenCommandEditor => "Edit commands" {
        aliases: &["Keybindings", "Keymap editor", "Edit keys", "Edit shortcuts"],
    },
    crate::help::OpenDocs => "GitButler docs",
    crate::help::OpenReleaseNotes => "What's new (release notes)",
    crate::help::ReportIssue => "Report an issue",
    crate::help::OpenDiscord => "GitButler Discord",
    crate::help::ShowConfigFolder => "Show settings folder",
    crate::help::ShowLogsFolder => "Show logs folder",
    crate::help::About => "About GitButler",
    Quit => "Quit",
}

/// How many commits the selected commit acts on with those picked with it, when that's more
/// than one.
fn picks(app: &AppView, cx: &App) -> Option<usize> {
    let store = app.store.read(cx);
    let Some(Selection::Commit(project, id)) = &store.selection else {
        return None;
    };
    let n = store.with_picks(project, *id).len();
    (n > 1).then_some(n)
}

/// Whether the navigator's tabs are across the top, the only place its tabs are.
fn navigator_on_top(app: &AppView, cx: &App) -> bool {
    app.store.read(cx).prefs.navigator_place == crate::state::NavigatorPlace::Top
}

fn on_branch(app: &AppView, cx: &App) -> bool {
    matches!(app.store.read(cx).selection, Some(Selection::Branch(..)))
}

/// The branch the selection pushes, merges or unapplies, as Push takes it: a branch, a commit's
/// branch or a stack's top, and the workspace it's in.
fn selected_branch<'a>(app: &AppView, cx: &'a App) -> Option<(&'a Workspace, FullName)> {
    let (project, ref_name) = app.branch_of_selection(cx)?;
    Some((app.store.read(cx).workspace(&project)?, ref_name?))
}

fn selected_workspace<'a>(app: &AppView, cx: &'a App) -> Option<&'a Workspace> {
    let store = app.store.read(cx);
    store.workspace(store.selection.as_ref()?.project())
}

fn push_phrase(app: &AppView, cx: &App) -> Option<String> {
    let (workspace, ref_name) = selected_branch(app, cx)?;
    let push = workspace.push_label(&ref_name);
    Some(match app.store.read(cx).selection {
        Some(Selection::Stack(..)) => format!("{push} stack"),
        _ => push.to_owned(),
    })
}

/// Not with nothing to push, or what stops it; from uncommitted changes, it pushes the branch
/// they commit to, which its key says when it can't.
fn pushable(app: &AppView, cx: &App) -> bool {
    let Some((project, ref_name)) = app.branch_of_selection(cx) else {
        return true;
    };
    (app.store.read(cx).workspace(&project))
        .is_none_or(|ws| ws.push_blocked(ref_name.as_ref()).is_none())
}

/// Everywhere while they're on, not only where the page has one: on a page without one, what
/// it says is where they are, which nothing else tells a newcomer.
fn briefs_on(app: &AppView, cx: &App) -> bool {
    app.store.read(cx).briefs
}

fn brief_here(app: &AppView, cx: &App) -> bool {
    app.detail.read(cx).has_brief()
}

fn brief_shown(app: &AppView, cx: &App) -> bool {
    app.detail.read(cx).showing_brief()
}

fn changes_can_go_beside(app: &AppView, cx: &App) -> bool {
    let detail = app.detail.read(cx);
    detail.showing_brief() && detail.can_show_changes_beside(cx)
}

fn briefs_off(app: &AppView, cx: &App) -> bool {
    !briefs_on(app, cx)
}

fn has_pull_request(app: &AppView, cx: &App) -> bool {
    app.pull_request_url(cx).is_some()
}

/// Not where it would only say why not, as with nothing committed yet.
fn pull_request_creatable(app: &AppView, cx: &App) -> bool {
    let store = app.store.read(cx);
    (store.selection.as_ref())
        .is_none_or(|s| !matches!(crate::pr::create_offer(store, s), Some(Err(_))))
}

/// Only when the target has moved on since the workspace last took it in.
fn target_moved(app: &AppView, cx: &App) -> bool {
    (selected_workspace(app, cx).and_then(|ws| ws.target.as_ref()))
        .is_some_and(|t| t.needs_update())
}

/// A worktree's branch starts from the target, so not before there is one.
fn has_target(app: &AppView, cx: &App) -> bool {
    selected_workspace(app, cx).is_some_and(|ws| ws.target.is_some())
}

fn pull_phrase(app: &AppView, cx: &App) -> Option<String> {
    let store = app.store.read(cx);
    let branch = store.branch_for(store.selection.as_ref()?)?;
    let commits = plural(branch.incoming.len(), "commit");
    Some(format!("Pull {commits} from the remote"))
}

/// What the navigator squashes, the row selected. With the keys in a diff it's the changes
/// they're on there, whatever the row, which the title alone says.
fn squash_phrase(app: &AppView, window: &Window, cx: &App) -> Option<String> {
    if app.detail_with_keys(window, cx).is_some() {
        return None;
    }
    if let Some(n) = picks(app, cx) {
        return Some(format!("Squash {n} commits"));
    }
    match &app.store.read(cx).selection {
        Some(Selection::Branch(..)) => Some("Squash branch".to_owned()),
        _ => None,
    }
}

/// Into the oldest picked, whose message the squash keeps: the commits list newest first.
fn squash_picked_phrase(app: &AppView, cx: &App) -> Option<String> {
    let n = picks(app, cx)?;
    let store = app.store.read(cx);
    let Some(Selection::Commit(project, id)) = &store.selection else {
        return None;
    };
    let (_, oldest) = (store.with_picks(project, *id).iter())
        .rev()
        .find_map(|id| store.find_commit(project, *id))?;
    let title = quoted(&commit_title(&oldest.title));
    Some(format!("Squash {n} commits into {title}"))
}

/// A branch alone in its stack is unapplied by itself.
fn unapply_phrase(app: &AppView, cx: &App) -> Option<String> {
    if !on_branch(app, cx) {
        return None;
    }
    let (workspace, ref_name) = selected_branch(app, cx)?;
    (!BranchEdits::of(workspace, &ref_name).stacked).then(|| "Unapply branch".to_owned())
}

/// Merging a branch with others below it takes the whole stack.
fn merge_phrase(app: &AppView, cx: &App) -> Option<String> {
    let (workspace, ref_name) = selected_branch(app, cx)?;
    let target = &workspace.target.as_ref()?.name;
    Some(match BranchEdits::of(workspace, &ref_name).merge? {
        true => format!("Merge the stack into {target}"),
        false => format!("Merge into {target}"),
    })
}

/// Not a linked worktree's last branch, which the worktree keeps.
fn deletable(app: &AppView, cx: &App) -> bool {
    let store = app.store.read(cx);
    let Some(selection @ Selection::Branch(_, Some(ref_name))) = &store.selection else {
        return true;
    };
    let Some((workspace, _)) = selected_branch(app, cx) else {
        return true;
    };
    let last = (workspace.lane_of(Some(ref_name))).is_some_and(|(lane, _)| {
        (lane.branches.iter())
            .filter(|b| b.ref_name.is_some())
            .count()
            < 2
    });
    !(last && store.in_linked(selection))
}

#[cfg(test)]
mod tests {
    use super::{COMMANDS, title_of};

    #[test]
    fn every_command_has_a_title_of_its_own() {
        let mut seen = std::collections::HashSet::new();
        for command in COMMANDS {
            assert!(
                seen.insert(command.title()),
                "{:?} names two commands, so searching for it can't tell which is meant, and \
                 running one puts both first",
                command.title()
            );
        }
    }

    #[test]
    fn no_alias_is_a_commands_title() {
        let titles: std::collections::HashSet<&str> = COMMANDS.iter().map(|c| c.title()).collect();
        for command in COMMANDS {
            for alias in command.aliases() {
                assert!(
                    !titles.contains(alias),
                    "{alias:?}, an alias of {:?}, is a command's title, so typing it finds both",
                    command.title()
                );
            }
        }
    }

    #[test]
    fn commands_without_a_palette_title_are_named_from_their_action() {
        assert_eq!(
            title_of("navigator::SelectNext"),
            "Select next",
            "words from the name"
        );
        assert_eq!(
            title_of("remote::UpdateFromTarget"),
            "Update workspace and worktrees",
            "the palette's own title first"
        );
    }

    #[test]
    fn no_title_ends_in_an_ellipsis() {
        for command in COMMANDS {
            let title = command.title();
            assert!(
                !title.ends_with('…') && !title.ends_with("..."),
                "{title:?}: a command is named for what it does, not for whether it asks first"
            );
        }
    }
}
