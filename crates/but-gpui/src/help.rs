//! The Help menu and About: where the docs and release notes are, where to report a
//! problem, and where GitButler keeps its settings.

use gpui::{App, PromptLevel, actions};

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
        if let Ok(dir) = but_path::app_config_dir() {
            cx.reveal_path(&dir);
        }
    });
    cx.on_action(|_: &About, cx| {
        let Some(window) = cx.active_window() else {
            return;
        };
        window
            .update(cx, |_, window, cx| {
                let channel = match but_path::AppChannel::new() {
                    but_path::AppChannel::Release => "release",
                    but_path::AppChannel::Nightly => "nightly",
                    but_path::AppChannel::Dev => "development",
                };
                let detail = format!(
                    "A native GitButler client, built on GPUI. It shares projects, \
                     settings and accounts with GitButler's other apps.\n\nThis is a \
                     {channel} build."
                );
                // Nothing to wait for: OK only closes it.
                drop(window.prompt(PromptLevel::Info, "GitButler", Some(&detail), &["OK"], cx));
            })
            .ok();
    });
}
