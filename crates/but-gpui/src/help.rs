//! The Help menu and About: where the docs are, where to talk to us or report a problem, and
//! where GitButler keeps its settings and logs. Its release notes open in the app (`whats_new`).

use gpui::{App, PromptLevel, actions};

use crate::backend::{logs, settings};

actions!(
    help,
    [
        About,
        OpenDiscord,
        OpenDocs,
        OpenReleaseNotes,
        ReportIssue,
        ShowConfigFolder,
        ShowLogsFolder
    ]
);

const DISCORD: &str = "https://discord.gg/MmFkmaJ42D";
const DOCS: &str = "https://docs.gitbutler.com";
const ISSUES: &str = "https://github.com/gitbutlerapp/gitbutler/issues/new/choose";

pub fn register(cx: &mut App) {
    cx.on_action(|_: &OpenDiscord, cx| cx.open_url(DISCORD));
    cx.on_action(|_: &OpenDocs, cx| cx.open_url(DOCS));
    cx.on_action(|_: &ReportIssue, cx| cx.open_url(ISSUES));
    cx.on_action(|_: &ShowConfigFolder, cx| {
        if let Ok(dir) = settings::config_dir() {
            cx.reveal_path(&dir);
        }
    });
    cx.on_action(|_: &ShowLogsFolder, cx| {
        if let Ok(dir) = logs::dir() {
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
            // The version is the desktop app's, which started this one; built on its own,
            // there's none.
            let version = (cx.try_global::<crate::Embedded>()).map_or(String::new(), |embedded| {
                format!(", version {}", embedded.version)
            });
            let detail = format!(
                "A native GitButler client, built on GPUI. It shares projects, settings and \
                 accounts with GitButler's other apps.\n\nThis is a {} build{version}.",
                settings::channel()
            );
            // Nothing to wait for: OK only closes it.
            drop(window.prompt(PromptLevel::Info, "GitButler", Some(&detail), &["OK"], cx));
        })
        .ok();
}
