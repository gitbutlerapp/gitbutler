//! The Help menu and About: where the docs and release notes are, where to report a
//! problem, and where GitButler keeps its settings.

use gpui::{App, PromptLevel, actions};

use crate::backend::settings;

actions!(
    help,
    [
        About,
        OpenDocs,
        OpenReleaseNotes,
        ReportIssue,
        ShowConfigFolder
    ]
);

const DOCS: &str = "https://docs.gitbutler.com";
const RELEASE_NOTES: &str = "https://github.com/gitbutlerapp/gitbutler/releases";
const ISSUES: &str = "https://github.com/gitbutlerapp/gitbutler/issues/new/choose";

pub fn register(cx: &mut App) {
    cx.on_action(|_: &OpenDocs, cx| cx.open_url(DOCS));
    cx.on_action(|_: &OpenReleaseNotes, cx| cx.open_url(RELEASE_NOTES));
    cx.on_action(|_: &ReportIssue, cx| cx.open_url(ISSUES));
    cx.on_action(|_: &ShowConfigFolder, cx| {
        if let Ok(dir) = settings::config_dir() {
            cx.reveal_path(&dir);
        }
    });
    // A menu action arrives while its window is being updated, which can't be updated again
    // from inside; the dialog waits for that to finish.
    cx.on_action(|_: &About, cx| cx.defer(show_about));
}

fn show_about(cx: &mut App) {
    let Some(window) = cx.active_window() else {
        return;
    };
    window
        .update(cx, |_, window, cx| {
            let channel = settings::channel();
            let detail = format!(
                "A native GitButler client, built on GPUI. It shares projects, settings and \
                 accounts with GitButler's other apps.\n\nThis is a {channel} build."
            );
            // Nothing to wait for: OK only closes it.
            drop(window.prompt(PromptLevel::Info, "GitButler", Some(&detail), &["OK"], cx));
        })
        .ok();
}
