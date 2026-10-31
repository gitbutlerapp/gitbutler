//! What the app can do, as ⌘K and the right-click menus offer it: each command's one title,
//! whether it throws work away, how a menu words it for the selection, and when it's offered.

use gpui::{Action, App};

use crate::app::{
    ActualSize, AddProject, AppView, CheckForUpdates, CopySelection, CutSelection, Fetch,
    FocusChanges, FocusDiff, FocusNavigator, NewBranch, NewBranchOnTop, OpenPullRequest, PasteHere,
    Pull, Push, Quit, Redo, StartCommit, ToggleDetail, ToggleFileTree, ToggleNavigator, Undo,
    UpdateFromTarget, UpdateWorkspace, UpdateWorktrees, ZoomIn, ZoomOut,
};
use crate::backend::{Checkout, FullName, Workspace};
use crate::detail::{
    DiscardFile, DiscardHunk, FoldFile, NextFile, NextHunk, PreviousFile, PreviousHunk, UnfoldFile,
};
use crate::edits::{
    AddEmptyCommitAbove, AddEmptyCommitBelow, BranchEdits, CherryPick, CopyChangeId, CopyMessage,
    DeleteBranch, DiscardChanges, DiscardCommit, NameBranch, NewBranchBelow, NewBranchHere,
    RenameBranch, Reword, RewrapMessage, SquashAll, UnapplyStack, Uncommit, Unstack,
};
use crate::palette::{CopyMore, GoTo, GoToTerminal};
use crate::state::Selection;
use crate::ui::{commit_title, plural, quoted};

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
    fn phrase(&self, _app: &AppView, _cx: &App) -> Option<String> {
        None
    }

    /// Whether ⌘K and the menus offer it where its handler is registered: not where it could
    /// only say why it can't run, as Push with nothing to push, whose key still says so.
    fn listed(&self, _app: &AppView, _cx: &App) -> bool {
        true
    }
}

/// Gives each action its title, `risky` after the ones that throw work away, and between braces
/// the functions of the app that word it for a menu (`phrase`) and say whether it's offered
/// (`listed`), and lists them as [`COMMANDS`], in that order.
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
        fn phrase(&self, app: &AppView, cx: &App) -> Option<String> {
            let phrase: fn(&AppView, &App) -> Option<String> = $method;
            phrase(app, cx)
        }
    };
    (@option listed $method:expr) => {
        fn listed(&self, app: &AppView, cx: &App) -> bool {
            let listed: fn(&AppView, &App) -> bool = $method;
            listed(app, cx)
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
    DiscardHunk => "Discard hunk" risky {
        phrase: |app, cx| (app.detail.read(cx).cut_kind(cx) == Some("lines"))
            .then(|| "Discard lines".to_owned()),
    },
    DiscardFile => "Discard file" risky {
        phrase: |app, cx| match app.detail.read(cx).cut_kind(cx)? {
            "lines" => Some("Discard lines".to_owned()),
            "files" => Some("Discard files".to_owned()),
            _ => None,
        },
    },
    crate::detail::MarkResolved => "Mark resolved",
    crate::detail::UncommitChanges => "Uncommit changes" risky {
        phrase: |app, cx| Some(format!("Uncommit {}", app.detail.read(cx).cut_kind(cx)?)),
    },
    crate::detail::SplitIntoNewCommit => "Split into a new commit",
    crate::detail::DiscardFromCommit => "Discard changes from commit" risky {
        phrase: |app, cx| {
            Some(format!("Discard {} from commit", app.detail.read(cx).cut_kind(cx)?))
        },
    },
    StartCommit => "Commit",
    Undo => "Undo",
    Redo => "Redo",
    Push => "Push" risky { phrase: push_phrase, listed: pushable },
    OpenPullRequest => "Open pull request" { listed: has_pull_request },
    Fetch => "Fetch",
    Pull => "Pull",
    UpdateFromTarget => "Update workspace and worktrees" {
        phrase: update_phrase,
        listed: target_moved,
    },
    UpdateWorkspace => "Update workspace",
    UpdateWorktrees => "Update worktrees",
    NewBranch => "New branch",
    NewBranchOnTop => "New branch on top",
    crate::app::NewWorktree => "New worktree" { listed: has_target },
    crate::app::NewWorktreeWithName => "New worktree with name" { listed: has_target },
    Reword => "Reword",
    RenameBranch => "Rename branch",
    NameBranch => "Name branch",
    Uncommit => "Uncommit" risky {
        phrase: |app, cx| picks(app, cx).map(|n| format!("Uncommit {n} commits")),
    },
    CherryPick => "Cherry-pick" {
        // The workspace's commits picked, or the Branches page's.
        phrase: |app, cx| {
            let n = picks(app, cx).or_else(|| app.detail.read(cx).branch_cherry_picks(cx))?;
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
    Unstack => "Unstack",
    UnapplyStack => "Unapply stack" risky { phrase: unapply_phrase },
    crate::edits::MergeBranch => "Merge into target" risky { phrase: merge_phrase },
    RewrapMessage => "Rewrap message",
    CopyChangeId => "Copy change ID",
    CopyMessage => "Copy message",
    DiscardCommit => "Discard commit" risky {
        phrase: |app, cx| picks(app, cx).map(|n| format!("Discard {n} commits")),
    },
    DeleteBranch => "Delete branch" risky { listed: deletable },
    DiscardChanges => "Discard changes" risky { phrase: discard_changes_phrase },
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
    CutSelection => "Cut" { phrase: cut_phrase },
    PasteHere => "Paste here",
    crate::app::MoveSelection => "Move" {
        phrase: |app, cx| picks(app, cx).map(|n| format!("Move {n} commits")),
    },
    ToggleNavigator => "Toggle navigator",
    ToggleDetail => "Toggle details",
    ZoomIn => "Zoom in",
    ZoomOut => "Zoom out",
    ActualSize => "Actual size",
    FocusNavigator => "Go to navigator",
    FocusChanges => "Go to files",
    FocusDiff => "Go to diff",
    AddProject => "Add project",
    crate::projects::CloneRepository => "Clone a repository",
    crate::settings::OpenSettings => "Settings",
    crate::settings::OpenKeyboardShortcuts => "Keyboard shortcuts",
    crate::app::OperationsHistory => "Operations history",
    crate::app::FocusNextPane => "Next pane",
    crate::app::FocusPreviousPane => "Previous pane",
    crate::app::FocusPaneRight => "Pane to the right",
    crate::app::FocusPaneLeft => "Pane to the left",
    crate::detail::SwitchTab => "Switch between Changes and Pull request",
    crate::app::CloseWindow => "Close window",
    GoTo => "Go to anything",
    GoToTerminal => "Go to a terminal",
    crate::detail::FindInDiff => "Find in diff",
    crate::detail::FilterFiles => "Filter files",
    crate::detail::ToggleWrap => "Wrap long lines",
    crate::detail::TogglePlainLines => "Toggle diff backgrounds",
    crate::detail::ToggleSplit => "Side-by-side diff",
    crate::detail::ToggleFiles => "Show or hide the files",
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
    crate::navigator::CloseTerminal => "Close terminal",
    crate::navigator::Watch => "Watch",
    crate::navigator::StopWatching => "Stop watching",
    crate::navigator::MoveUp => "Move up",
    crate::navigator::MoveDown => "Move down",
    crate::navigator::UnarchiveWorktree => "Unarchive worktree",
    crate::navigator::Reveal => (crate::ui::REVEAL),
    crate::navigator::OpenProjectSettings => "Project settings",
    crate::navigator::ArchiveWorktree => "Archive worktree" {
        phrase: |_, _| Some("Archive".to_owned()),
    },
    crate::navigator::DeleteWorktree => "Delete worktree" risky,
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
        phrase: |_, _| Some("Only local branches".to_owned()),
    },
    // The menu bar's, for Windows, which has none.
    CheckForUpdates => "Check for updates",
    crate::app::BuildAndRestart => "Rebuild and restart",
    crate::app::BuildAndRestartFromHere => "Build and restart from here",
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

fn on_branch(app: &AppView, cx: &App) -> bool {
    matches!(app.store.read(cx).selection, Some(Selection::Branch(..)))
}

/// The branch the selection pushes, merges or unapplies, as Push takes it: a branch, a commit's
/// branch or a stack's top, and the workspace it's in.
fn selected_branch<'a>(app: &AppView, cx: &'a App) -> Option<(&'a Workspace, FullName)> {
    let (project, ref_name) = app.branch_of_selection(cx)?;
    Some((app.store.read(cx).workspace(&project)?, ref_name?))
}

/// The workspace of the selected project.
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

fn has_pull_request(app: &AppView, cx: &App) -> bool {
    app.pull_request_url(cx).is_some()
}

/// Not where it would only say why not, as with nothing committed yet.
fn pull_request_creatable(app: &AppView, cx: &App) -> bool {
    let store = app.store.read(cx);
    (store.selection.as_ref())
        .is_none_or(|s| !matches!(crate::pr::create_offer(store, s), Some(Err(_))))
}

fn update_phrase(app: &AppView, cx: &App) -> Option<String> {
    let target = selected_workspace(app, cx)?.target.as_ref()?;
    Some(format!("Update from {}", target.name))
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

/// Whether the composer is open on the selected checkout. `None` when no checkout is selected.
fn composing_selected(app: &AppView, cx: &App) -> Option<bool> {
    let (project, checkout) = match &app.store.read(cx).selection {
        Some(Selection::Uncommitted(project)) => (project, Checkout::Main),
        Some(Selection::Worktree(project, name)) => (project, Checkout::Linked(name.clone())),
        _ => return None,
    };
    Some(app.detail.read(cx).composing_in(project, &checkout))
}

/// The changes the composer has checked, while it's open on them, or all of them.
fn discard_changes_phrase(app: &AppView, cx: &App) -> Option<String> {
    Some(match composing_selected(app, cx)? {
        true => "Discard checked changes".to_owned(),
        false => "Discard all changes".to_owned(),
    })
}

/// While committing, a cut takes what's checked wherever the keys are, a file's menu too.
fn cut_phrase(app: &AppView, cx: &App) -> Option<String> {
    match picks(app, cx) {
        Some(n) => Some(format!("Cut {n} commits")),
        None => composing_selected(app, cx)?.then(|| "Cut checked changes".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::COMMANDS;

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
