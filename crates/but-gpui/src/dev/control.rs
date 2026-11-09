//! Layout commands: what a script gives as `layout …`, and what a dev build the user is looking
//! at takes on its control socket as `dev.layout`, so an agent can set up the window as asked
//! (`scripts/control.sh`). Only layout: nothing here commits, discards, pushes, types or
//! clicks, so the worst a command does is move the user's tiles.
//!
//! ```text
//! select Branches         # select a row as Go to anything finds it typed in full: by its
//!                         # whole name first, ignoring case, else by part of it, in the
//!                         # selected project first
//! pick first              # step a list page's list (Branches, a project's page) to its
//!                         # first row, or next, previous or last
//! files on                # show the file column, as `f` does: beside the diff, or beside
//!                         # a list page's list; off hides it
//! navigator off           # show or hide the navigator, as ⌘B does
//! detail off              # show or hide the detail, as ⌥⌘B does
//! split right Graph       # pin a row beside the tile with the keys, or below it, or a new
//!                         # terminal with `split below terminal`
//! close Graph             # close the tile showing a row, or every pinned tile with `all`
//! zoom                    # zoom the tile with the keys, or send it back
//! equalize                # give the tiles the same room
//! show                    # change nothing, only say how it's laid out
//! ```

use anyhow::{Context as _, Result, bail, ensure};
use gpui::{Action, App, AppContext as _, Entity, Window};

use crate::app::{AppView, EqualizeTiles, SelectNext, SelectPrevious, ZoomTile, tile_label};
use crate::navigator::{SelectFirst, SelectLast};
use crate::state::{Axis, Selection};

/// Run the layout command `line`, what follows `layout`.
pub(super) fn run(line: &str, window: &mut Window, cx: &mut App) -> Result<()> {
    let app = window.root::<AppView>().flatten().context("no app view")?;
    let line = line.trim();
    let (command, rest) = line.split_once(' ').unwrap_or((line, ""));
    let rest = rest.trim();
    match command {
        "" | "show" => {}
        "select" => {
            let selection = find(&app, rest, cx)?;
            if app.read(cx).store.read(cx).selection.as_ref() != Some(&selection) {
                app.update(cx, |app, cx| app.select(selection, window, cx));
            }
        }
        "pick" => {
            let step: Box<dyn Action> = match rest {
                "first" => Box::new(SelectFirst),
                "next" => Box::new(SelectNext),
                "previous" => Box::new(SelectPrevious),
                "last" => Box::new(SelectLast),
                _ => bail!("pick takes first, next, previous or last"),
            };
            let detail = app.read(cx).detail.clone();
            ensure!(
                detail.read(cx).list_page(),
                "pick steps a list page's list, as Branches' or a project's page has"
            );
            // Its list takes the keys, as clicking it does, and steps as they would.
            detail.update(cx, |detail, cx| detail.focus_files(window, cx));
            window.dispatch_action(step, cx);
        }
        "files" => {
            let shown = on_off(command, rest)?;
            let detail = app.read(cx).detail.clone();
            let toggled = detail.read(cx).files_toggled(cx);
            ensure!(
                toggled.is_some(),
                "this page has no file column to show or hide: on Branches, pick a branch first"
            );
            if toggled != Some(shown) {
                detail.update(cx, |detail, cx| detail.toggle_files(window, cx));
            }
        }
        "navigator" => {
            let shown = on_off(command, rest)?;
            app.update(cx, |app, cx| app.set_navigator(shown, window, cx));
        }
        "detail" => {
            let shown = on_off(command, rest)?;
            app.update(cx, |app, cx| app.set_detail(shown, window, cx));
        }
        "split" => {
            let (side, what) = rest
                .split_once(' ')
                .context("split takes right or below, then terminal or a row")?;
            let axis = match side {
                "right" => Axis::Across,
                "below" => Axis::Down,
                _ => bail!("split takes right or below"),
            };
            let at = app.read(cx).focused_tile(window, cx);
            let store = app.read(cx).store.clone();
            if what.trim() == "terminal" {
                store.update(cx, |store, cx| store.pin_terminal(at, axis, cx));
            } else {
                let selection = find(&app, what.trim(), cx)?;
                let (home, pinned) = {
                    let store = store.read(cx);
                    let home = store.selection.as_ref().map(|s| store.row_key(s));
                    (home, store.row_key(&selection))
                };
                let shown = store.read(cx).pinned().contains(&pinned);
                ensure!(
                    home.as_ref() != Some(&pinned) && !shown,
                    "{what} shows already"
                );
                store.update(cx, |store, cx| store.pin(at, axis, pinned, cx));
            }
        }
        "close" => {
            let store = app.read(cx).store.clone();
            let pinned = store.read(cx).pinned();
            let closing = match rest {
                "all" => pinned,
                _ => {
                    let selection = find(&app, rest, cx)?;
                    let row = store.read(cx).row_key(&selection);
                    ensure!(pinned.contains(&row), "no tile of its own shows {rest}");
                    vec![row]
                }
            };
            for row in closing {
                store.update(cx, |store, cx| store.close_tile(&row, cx));
            }
        }
        "zoom" | "equalize" => {
            ensure!(
                app.read(cx).store.read(cx).tiling().is_some(),
                "there's one tile, with nothing to {command}"
            );
            match command {
                "zoom" => window.dispatch_action(Box::new(ZoomTile), cx),
                _ => window.dispatch_action(Box::new(EqualizeTiles), cx),
            }
        }
        _ => bail!(
            "layout takes select, pick, files, navigator, detail, split, close, zoom, equalize or show"
        ),
    }
    Ok(())
}

/// How the window is laid out, a fact a line: what's selected, what shows, and its tiles.
pub(super) fn describe(window: &mut Window, cx: &App) -> Result<String> {
    let app = window.root::<AppView>().flatten().context("no app view")?;
    let app = app.read(cx);
    let store = app.store.read(cx);
    let selected = (store.selection.as_ref())
        .map(|selection| tile_label(store, &store.row_key(selection)).1.to_string())
        .unwrap_or_else(|| "nothing".to_owned());
    let files = match app.detail.read(cx).files_toggled(cx) {
        Some(true) => "on",
        Some(false) => "off",
        None => "none here",
    };
    let shown = |shown: bool| if shown { "on" } else { "off" };
    let tiles = app
        .tile_diagram(cx)
        .unwrap_or_else(|| "one tile".to_owned());
    Ok(format!(
        "selected: {selected}\nnavigator: {}, detail: {}, files: {files}\ntiles:\n{tiles}",
        shown(app.navigator_open),
        shown(app.detail_open),
    ))
}

/// The row Go to anything finds as `query`.
fn find(app: &Entity<AppView>, query: &str, cx: &App) -> Result<Selection> {
    ensure!(!query.is_empty(), "name a row, as Go to anything finds it");
    crate::palette::go_to_row(app.read(cx).store.read(cx), query)
        .with_context(|| format!("Go to anything finds nothing called {query}"))
}

fn on_off(command: &str, rest: &str) -> Result<bool> {
    match rest {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => bail!("{command} takes on or off"),
    }
}

/// Answer `dev.layout`: run the lines in its `commands`, each given a moment to show before the
/// next, and answer with each command's error, then how the window is laid out.
pub(crate) fn layout(
    request: but_ui_control::Request,
    reply: crate::control::Reply,
    window: &mut Window,
    cx: &mut App,
) {
    use std::time::Duration;

    #[derive(serde::Deserialize)]
    struct Params {
        commands: Vec<String>,
    }
    let params: Params = match serde_json::from_value(request.params) {
        Ok(params) => params,
        Err(err) => {
            return reply.err(but_ui_control::RpcError::new(
                but_ui_control::code::INVALID_PARAMS,
                format!("dev.layout takes its commands as a list: {err}"),
            ));
        }
    };
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        let mut errors = Vec::new();
        for line in params.commands {
            let command = line.trim();
            let command = (command.strip_prefix("layout "))
                .unwrap_or(command.strip_prefix("layout").unwrap_or(command))
                .to_owned();
            let result = (cx.update_window(handle, |_, window, cx| run(&command, window, cx)))
                .and_then(|result| result);
            if let Err(err) = result {
                errors.push(format!("{line}: {err:#}"));
            }
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
        }
        let layout = (cx.update_window(handle, |_, window, cx| describe(window, cx)))
            .and_then(|layout| layout)
            .unwrap_or_else(|err| format!("error: {err:#}"));
        reply.ok(serde_json::json!({ "errors": errors, "layout": layout }));
    })
    .detach();
}
