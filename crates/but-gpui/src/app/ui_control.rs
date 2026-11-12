#![cfg_attr(
    not(unix),
    allow(dead_code, reason = "only the unix socket hands requests over")
)]

use but_ui_control::ui::{
    self, ActionInfo, Area as UiArea, DispatchParams, FocusParams, Keys, Row, SelectParams, State,
    TypeParams,
};
use but_ui_control::{Request, RpcError, code};
use gpui::{App, AppContext as _, Context, Keystroke, Window};
use serde::de::DeserializeOwned;

use super::{AppView, KeyPlace, tile_label};
use crate::commands::{COMMANDS, title_of};
use crate::control::Reply;
use crate::detail::KeyPane;
use crate::state::{Area, RowKey, Selection, Store, Tile};

/// The actions a person never runs by name, as a field's own editing keys.
const UNLISTED: [&str; 2] = ["zed::", "stories::"];

const NAVIGATOR: &str = "navigator";
const TERMINALS: &str = "terminals";
const HOME: &str = "home";

pub(crate) fn handle(request: Request, reply: Reply, window: &mut Window, cx: &mut App) {
    let Some(app) = window.root::<AppView>().flatten() else {
        return reply.err(RpcError::new(code::INTERNAL_ERROR, "The window has no app"));
    };
    // Where the keys are and what can act there are read from the last frame, which may be
    // older than what the last request moved.
    window.draw(cx).clear(cx);
    match request.method.as_str() {
        ui::STATE => reply.ok(app.read(cx).ui_state(window, cx)),
        ui::ACTIONS => reply.ok(app.read(cx).ui_actions(window, cx)),
        ui::DISPATCH => reply.answer(
            params::<DispatchParams>(&request)
                .and_then(|params| dispatch(&app, params, window, cx)),
        ),
        ui::SELECT => {
            reply.answer(params::<SelectParams>(&request).and_then(|params| {
                app.update(cx, |app, cx| app.ui_select(&params.query, window, cx))
            }))
        }
        ui::FOCUS => reply.answer(
            params::<FocusParams>(&request)
                .and_then(|params| app.update(cx, |app, cx| app.ui_focus(&params, window, cx))),
        ),
        ui::TYPE => match params::<TypeParams>(&request) {
            Ok(params) => type_lines(app, params.text, reply, window, cx),
            Err(err) => reply.err(err),
        },
        method => reply.err(RpcError::new(
            code::METHOD_NOT_FOUND,
            format!("The app has no method {method}"),
        )),
    }
}

fn params<P: DeserializeOwned>(request: &Request) -> Result<P, RpcError> {
    serde_json::from_value(request.params.clone()).map_err(|err| {
        RpcError::new(
            code::INVALID_PARAMS,
            format!("The params don't fit {}: {err}", request.method),
        )
    })
}

/// Give the tile and pane `params` names the keys, then run its action there.
fn dispatch(
    app: &gpui::Entity<AppView>,
    params: DispatchParams,
    window: &mut Window,
    cx: &mut App,
) -> Result<(), RpcError> {
    if params.tile.is_some() || params.pane.is_some() {
        let focus = FocusParams {
            tile: params.tile.clone(),
            pane: params.pane.clone(),
        };
        app.update(cx, |app, cx| app.ui_focus(&focus, window, cx))?;
        // A view registers its actions' handlers as it draws.
        window.draw(cx).clear(cx);
    }
    app.update(cx, |app, cx| {
        app.ui_dispatch(&params.action, params.args, window, cx)
    })
}

/// Type `text` where the keys are, outside the app's update, because a key's handler updates
/// the app itself. A piece per update, ending at each Enter or Tab, so where they send the keys
/// is settled and checked before the next piece goes in.
fn type_lines(
    app: gpui::Entity<AppView>,
    text: String,
    reply: Reply,
    window: &mut Window,
    cx: &mut App,
) {
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        for (typed, piece) in text.split_inclusive(['\n', '\t']).enumerate() {
            let result = cx.update_window(handle, |_, window, cx| {
                window.draw(cx).clear(cx);
                let text_field = app.read(cx).ui_keys(window, cx).text_field;
                type_text(piece, text_field, window, cx)
            });
            let err = match result {
                Ok(Ok(())) => continue,
                Ok(Err(err)) => err,
                Err(_) => RpcError::new(code::INTERNAL_ERROR, "The window closed"),
            };
            return reply.err(match typed {
                0 => err,
                typed => RpcError::new(
                    err.code,
                    format!(
                        "After typing {typed} pieces, up to an Enter or Tab: {}",
                        err.message
                    ),
                ),
            });
        }
        reply.ok(());
    })
    .detach();
}

impl AppView {
    fn ui_state(&self, window: &Window, cx: &App) -> State {
        let store = self.store.read(cx);
        let shown = match self.detail_open {
            true => self.tile_ids(cx),
            false => Vec::new(),
        };
        let tiles = (shown.into_iter())
            .map(|(tile, area, id)| ui::Tile {
                id,
                shows: self.tile_shows(&tile, cx),
                area: UiArea {
                    left: area.left,
                    top: area.top,
                    width: area.width,
                    height: area.height,
                },
                panes: (self.tile_detail(&tile, cx).iter())
                    .flat_map(|detail| detail.read(cx).key_panes(cx))
                    .map(|pane| pane.name().to_owned())
                    .collect(),
            })
            .collect();
        let overlay = match () {
            _ if self.prompt.is_some() => Some("prompt"),
            _ if self.modal.is_some() || self.icon_picker.is_some() => Some("modal"),
            _ if self.palette.is_some() => Some("palette"),
            _ if self.menu.is_some() => Some("menu"),
            _ => None,
        }
        .map(str::to_owned);
        State {
            navigator: self.rows_in_sight(),
            detail: self.detail_open,
            selection: store.selection.as_ref().map(|s| row_of(store, s)),
            tiles,
            keys: self.ui_keys(window, cx),
            overlay,
        }
    }

    fn ui_keys(&self, window: &Window, cx: &App) -> Keys {
        if window.focused(cx).is_none() {
            return Keys {
                tile: None,
                pane: None,
                text_field: false,
            };
        }
        let text_field = types_text(&window.context_stack());
        let (tile, pane) = match self.place_with_keys(window, cx) {
            KeyPlace::Navigator => (NAVIGATOR.to_owned(), None),
            KeyPlace::Terminals => (TERMINALS.to_owned(), None),
            KeyPlace::Tile(tile, pane) => (
                self.tile_id(&tile, cx),
                pane.map(|pane| pane.name().to_owned()),
            ),
        };
        Keys {
            tile: Some(tile),
            pane,
            text_field,
        }
    }

    fn ui_actions(&self, window: &Window, cx: &App) -> Vec<ActionInfo> {
        let mut actions: Vec<ActionInfo> = (window.available_actions(cx).into_iter())
            .filter(|action| {
                !UNLISTED
                    .iter()
                    .any(|prefix| action.name().starts_with(prefix))
            })
            .map(|action| {
                let name = action.name();
                let command = COMMANDS.iter().find(|command| command.name() == name);
                let keys =
                    (window.highest_precedence_binding_for_action(&*action)).map(|binding| {
                        let keys: Vec<String> = binding
                            .keystrokes()
                            .iter()
                            .map(|key| key.unparse())
                            .collect();
                        keys.join(" ")
                    });
                ActionInfo {
                    name: name.to_owned(),
                    title: title_of(name),
                    keys,
                    risky: command.is_some_and(|command| command.risky()),
                }
            })
            .collect();
        actions.sort_by(|a, b| a.name.cmp(&b.name));
        actions
    }

    fn ui_dispatch(
        &mut self,
        name: &str,
        args: Option<serde_json::Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), RpcError> {
        let action = cx.build_action(name, args).map_err(|err| {
            let code = match cx.all_action_names().contains(&name) {
                true => code::INVALID_PARAMS,
                false => code::NOT_FOUND,
            };
            RpcError::new(code, format!("Can't run {name}: {err}"))
        })?;
        let wanted = action.as_any().type_id();
        let available = (window.available_actions(cx).iter())
            .any(|available| available.as_any().type_id() == wanted);
        if !available {
            let keys = self.ui_keys(window, cx);
            let place = match (keys.tile, keys.pane) {
                (Some(tile), Some(pane)) => format!("{tile} › {pane}"),
                (Some(tile), None) => tile,
                _ => "nowhere: the window doesn't have the keys".to_owned(),
            };
            return Err(RpcError::new(
                code::NOT_AVAILABLE,
                format!(
                    "{name} can't act where the keys are ({place}). ui.actions lists what can, and ui.focus moves the keys."
                ),
            ));
        }
        window.dispatch_action(action, cx);
        Ok(())
    }

    fn ui_select(
        &mut self,
        query: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Row, RpcError> {
        let selection = crate::palette::go_to_row(self.store.read(cx), query).ok_or_else(|| {
            RpcError::new(
                code::NOT_FOUND,
                format!("Go to anything finds nothing called {query}"),
            )
        })?;
        self.select(selection.clone(), window, cx);
        Ok(row_of(self.store.read(cx), &selection))
    }

    fn ui_focus(
        &mut self,
        params: &FocusParams,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), RpcError> {
        let tile = match params.tile.as_deref() {
            Some(NAVIGATOR) => {
                if params.pane.is_some() {
                    return Err(RpcError::new(
                        code::INVALID_PARAMS,
                        "The navigator has no panes",
                    ));
                }
                if !self.rows_in_sight() {
                    return Err(RpcError::new(
                        code::NOT_AVAILABLE,
                        "The navigator is hidden: app::ToggleNavigator shows it",
                    ));
                }
                self.focus_place(&KeyPlace::Navigator, window, cx);
                return Ok(());
            }
            Some(TERMINALS) => {
                if params.pane.is_some() {
                    return Err(RpcError::new(
                        code::INVALID_PARAMS,
                        "The terminal overview has no panes",
                    ));
                }
                if !self.terminals_shown(cx) {
                    return Err(RpcError::new(
                        code::NOT_AVAILABLE,
                        "The terminal overview is hidden",
                    ));
                }
                self.focus_place(&KeyPlace::Terminals, window, cx);
                return Ok(());
            }
            Some(_) if !self.detail_open => return Err(detail_hidden()),
            Some(id) => {
                let tile = (self.tile_ids(cx).into_iter())
                    .find(|(_, _, shown)| shown == id)
                    .map(|(tile, _, _)| tile)
                    .ok_or_else(|| {
                        RpcError::new(
                            code::NOT_FOUND,
                            format!("No tile shows as {id}: ui.state lists the tiles"),
                        )
                    })?;
                if let Some(focus) = self.tile_focus(&tile, cx) {
                    window.focus(&focus, cx);
                }
                tile
            }
            None => (self.zoomed().cloned()).unwrap_or_else(|| self.focused_tile(window, cx)),
        };
        let Some(pane) = params.pane.as_deref() else {
            return Ok(());
        };
        if !self.detail_open {
            return Err(detail_hidden());
        }
        let tile_id = self.tile_id(&tile, cx);
        let detail = self.tile_detail(&tile, cx).ok_or_else(|| {
            RpcError::new(
                code::NOT_AVAILABLE,
                format!("{tile_id} shows a terminal, which has no panes"),
            )
        })?;
        let panes = detail.read(cx).key_panes(cx);
        let found = KeyPane::named(pane).filter(|pane| panes.contains(pane));
        let Some(pane) = found else {
            let names: Vec<&str> = panes.iter().map(|pane| pane.name()).collect();
            return Err(RpcError::new(
                code::NOT_AVAILABLE,
                format!(
                    "{tile_id} has no {pane} pane now; it has {}",
                    names.join(", ")
                ),
            ));
        };
        detail.update(cx, |detail, cx| detail.focus_key_pane(pane, window, cx));
        Ok(())
    }

    /// The tiles that show, with ids that tell them apart.
    fn tile_ids(&self, cx: &App) -> Vec<(Tile, Area, String)> {
        let store = self.store.read(cx);
        let shown = self.shown_tiles(cx);
        let labels: Vec<String> = (shown.iter())
            .filter_map(|(tile, _)| match tile {
                Tile::Home => None,
                Tile::Pinned(row) => Some(tile_label(store, row).1.to_string()),
            })
            .collect();
        let mut ids = unique_ids(
            labels.iter().map(String::as_str),
            &[HOME, NAVIGATOR, TERMINALS],
        )
        .into_iter();
        (shown.into_iter())
            .map(|(tile, area)| {
                let id = match tile {
                    Tile::Home => HOME.to_owned(),
                    Tile::Pinned(_) => ids.next().unwrap_or_default(),
                };
                (tile, area, id)
            })
            .collect()
    }

    fn tile_id(&self, tile: &Tile, cx: &App) -> String {
        (self.tile_ids(cx).into_iter())
            .find(|(shown, _, _)| shown == tile)
            .map_or_else(|| HOME.to_owned(), |(_, _, id)| id)
    }

    fn tile_shows(&self, tile: &Tile, cx: &App) -> Row {
        let store = self.store.read(cx);
        match tile {
            Tile::Home => match &store.selection {
                Some(selection) => row_of(store, selection),
                None => Row {
                    project: None,
                    kind: "nothing".to_owned(),
                    label: String::new(),
                },
            },
            Tile::Pinned(row) => {
                let (_, label) = tile_label(store, row);
                let (kind, project) = match row {
                    RowKey::Row(selection) => (kind_of(selection), Some(selection.project())),
                    RowKey::Commit(project, _) => ("commit", Some(project)),
                    RowKey::Tab(project, _) => ("tab", Some(project)),
                    RowKey::Changes(selection) => ("changes", Some(selection.project())),
                };
                Row {
                    project: project
                        .and_then(|id| store.project(id))
                        .map(|e| e.project.name.clone()),
                    kind: kind.to_owned(),
                    label: label.to_string(),
                }
            }
        }
    }
}

fn type_text(
    text: &str,
    text_field: bool,
    window: &mut Window,
    cx: &mut App,
) -> Result<(), RpcError> {
    if !text_field {
        return Err(RpcError::new(
            code::NOT_AVAILABLE,
            "Typing where the keys are doesn't go in as text: focus a text field or a terminal first",
        ));
    }
    for ch in text.chars() {
        window.dispatch_keystroke(keystroke_for(ch)?, cx);
    }
    Ok(())
}

fn keystroke_for(ch: char) -> Result<Keystroke, RpcError> {
    let key = match ch {
        '\n' => "enter".to_owned(),
        '\t' => "tab".to_owned(),
        ' ' => "space".to_owned(),
        ch => ch.to_string(),
    };
    let mut keystroke = Keystroke::parse(&key)
        .map_err(|err| RpcError::new(code::INVALID_PARAMS, format!("Can't type {ch:?}: {err}")))?;
    if !ch.is_control() {
        keystroke.key_char = Some(ch.to_string());
    }
    Ok(keystroke)
}

fn row_of(store: &Store, selection: &Selection) -> Row {
    let (_, label) = tile_label(store, &store.row_key(selection));
    Row {
        project: (store.project(selection.project())).map(|e| e.project.name.clone()),
        kind: kind_of(selection).to_owned(),
        label: label.to_string(),
    }
}

fn kind_of(selection: &Selection) -> &'static str {
    match selection {
        Selection::Project(..) => "project",
        Selection::Section(..) => "section",
        Selection::Checkout(..) => "checkout",
        Selection::Uncommitted(..) => "uncommitted",
        Selection::Terminal(..) => "terminal",
        Selection::Branch(..) => "branch",
        Selection::Stack(..) => "stack",
        Selection::Commit(..) => "commit",
        Selection::Upstream(..) => "upstream-commit",
        Selection::Mesh(..) => "mesh",
        Selection::Pending(..) => "pending-commit",
    }
}

/// Arranging tiles claims `TextInput` only to keep shortcuts out: its letters move tiles.
fn types_text(contexts: &[gpui::KeyContext]) -> bool {
    let arranging = |context: &gpui::KeyContext| {
        context.contains("ArrangingTiles") || context.contains("SwappingTiles")
    };
    contexts.iter().any(|context| context.contains("TextInput")) && !contexts.iter().any(arranging)
}

fn detail_hidden() -> RpcError {
    RpcError::new(
        code::NOT_AVAILABLE,
        "The detail is hidden, and its tiles with it: app::ToggleDetail shows it",
    )
}

/// Each label as an id, a repeated one or one in `taken` numbered on, as `Terminal 2`.
fn unique_ids<'a>(labels: impl IntoIterator<Item = &'a str>, taken: &[&str]) -> Vec<String> {
    let labels: Vec<&str> = labels.into_iter().collect();
    let mut used = std::collections::HashSet::new();
    let free = |id: &str, used: &std::collections::HashSet<String>| {
        !used.contains(id) && !taken.contains(&id)
    };
    (labels.iter())
        .map(|label| {
            let mut id = (*label).to_owned();
            let mut n = 1;
            while !free(&id, &used) || (n > 1 && labels.contains(&id.as_str())) {
                n += 1;
                id = format!("{label} {n}");
            }
            used.insert(id.clone());
            id
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{keystroke_for, types_text, unique_ids};

    #[test]
    fn every_printable_character_types_as_itself() {
        for ch in (' '..='~').chain(['é', 'ß']) {
            let keystroke = keystroke_for(ch).unwrap_or_else(|err| panic!("{ch:?}: {err}"));
            assert_eq!(
                keystroke.key_char.as_deref(),
                Some(ch.to_string().as_str()),
                "{ch:?}"
            );
            let held = keystroke.modifiers;
            assert!(
                !(held.control || held.alt || held.platform || held.function),
                "{ch:?} types with at most shift held, as on a keyboard, so it runs no shortcut"
            );
        }
        assert_eq!(keystroke_for('\n').unwrap().key, "enter");
    }

    #[test]
    fn keys_type_text_only_in_a_field() {
        let contexts = |stack: &[&str]| -> Vec<gpui::KeyContext> {
            stack
                .iter()
                .map(|context| gpui::KeyContext::parse(context).unwrap())
                .collect()
        };
        assert!(types_text(&contexts(&["Workspace", "Editor TextInput"])));
        assert!(types_text(&contexts(&["Workspace", "Terminal TextInput"])));
        assert!(
            !types_text(&contexts(&["Workspace", "ArrangingTiles TextInput"])),
            "arranging tiles holds the keys from shortcuts, but its letters move tiles"
        );
        assert!(!types_text(&contexts(&[
            "Workspace",
            "SwappingTiles TextInput"
        ])));
        assert!(!types_text(&contexts(&["Workspace", "Diff"])));
    }

    #[test]
    fn tiles_that_show_the_same_get_ids_of_their_own() {
        assert_eq!(
            unique_ids(["Terminal", "Graph", "Terminal", "Terminal"], &[]),
            ["Terminal", "Graph", "Terminal 2", "Terminal 3"],
            "a repeated label is numbered on"
        );
        assert_eq!(
            unique_ids(["home", "navigator"], &["home", "navigator", "terminals"]),
            ["home 2", "navigator 2"],
            "a label can't take a name the window keeps for itself"
        );
        assert_eq!(
            unique_ids(["Terminal 2", "Terminal", "Terminal"], &[]),
            ["Terminal 2", "Terminal", "Terminal 3"],
            "nor one another tile already has"
        );
    }
}
