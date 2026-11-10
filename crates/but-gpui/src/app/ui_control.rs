#![cfg_attr(
    not(unix),
    allow(dead_code, reason = "only the unix socket hands requests over")
)]

use but_ui_control::ui::{
    self, ActionInfo, Area as UiArea, DispatchParams, FocusParams, Keys, Overlay, Row,
    SelectParams, State, TypeParams,
};
use but_ui_control::{Request, RpcError, code};
use gpui::{App, Context, Keystroke, Window};
use serde::de::DeserializeOwned;

use super::{AppView, KeyPlace, tile_label};
use crate::commands::{COMMANDS, title_of};
use crate::control::Reply;
use crate::detail::KeyPane;
use crate::state::{RowKey, Selection, Store, Tile};

/// The actions a person never runs by name, as a field's own editing keys.
const UNLISTED: [&str; 2] = ["zed::", "stories::"];

pub(crate) fn handle(request: Request, reply: Reply, window: &mut Window, cx: &mut App) {
    let Some(app) = window.root::<AppView>().flatten() else {
        return reply.err(RpcError::new(code::INTERNAL_ERROR, "The window has no app"));
    };
    fn params<P: DeserializeOwned>(request: &Request) -> Result<P, RpcError> {
        serde_json::from_value(request.params.clone()).map_err(|err| {
            RpcError::new(
                code::INVALID_PARAMS,
                format!("The params don't fit {}: {err}", request.method),
            )
        })
    }
    let answer = match request.method.as_str() {
        ui::STATE => Ok(serde_json::to_value(app.read(cx).ui_state(window, cx))),
        ui::ACTIONS => Ok(serde_json::to_value(app.read(cx).ui_actions(window, cx))),
        ui::DISPATCH => params::<DispatchParams>(&request).and_then(|params| {
            app.update(cx, |app, cx| app.ui_dispatch(params, window, cx))
                .map(|()| serde_json::to_value(()))
        }),
        ui::SELECT => params::<SelectParams>(&request).and_then(|params| {
            (app.update(cx, |app, cx| app.ui_select(&params.query, window, cx)))
                .map(serde_json::to_value)
        }),
        ui::FOCUS => params::<FocusParams>(&request).and_then(|params| {
            app.update(cx, |app, cx| app.ui_focus(&params, window, cx))
                .map(|()| serde_json::to_value(()))
        }),
        // Outside the app's update, because a key's handler updates the app itself.
        ui::TYPE => params::<TypeParams>(&request).and_then(|params| {
            let text_field = app.read(cx).ui_keys(window, cx).text_field;
            type_text(&params.text, text_field, window, cx).map(|()| serde_json::to_value(()))
        }),
        method => Err(RpcError::new(
            code::METHOD_NOT_FOUND,
            format!("The app has no method {method}"),
        )),
    };
    match answer {
        Ok(Ok(value)) => reply.ok(value),
        Ok(Err(err)) => reply.err(RpcError::new(code::INTERNAL_ERROR, err.to_string())),
        Err(err) => reply.err(err),
    }
}

impl AppView {
    fn ui_state(&self, window: &Window, cx: &App) -> State {
        let store = self.store.read(cx);
        let tiles = (self.shown_tiles(cx).into_iter())
            .map(|(tile, area)| ui::Tile {
                id: self.tile_id(&tile, cx),
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
            _ if self.prompt.is_some() => Some(Overlay::Prompt),
            _ if self.modal.is_some() || self.icon_picker.is_some() => Some(Overlay::Modal),
            _ if self.palette.is_some() => Some(Overlay::Palette),
            _ if self.menu.is_some() => Some(Overlay::Menu),
            _ => None,
        };
        State {
            navigator: self.navigator_open,
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
        let text_field =
            (window.context_stack().iter()).any(|context| context.contains("TextInput"));
        let (tile, pane) = match self.place_with_keys(window, cx) {
            KeyPlace::Navigator => (ui::NAVIGATOR.to_owned(), None),
            KeyPlace::Terminals => (ui::TERMINALS.to_owned(), None),
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
        params: DispatchParams,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), RpcError> {
        if params.tile.is_some() || params.pane.is_some() {
            let focus = FocusParams {
                tile: params.tile.clone(),
                pane: params.pane.clone(),
            };
            self.ui_focus(&focus, window, cx)?;
        }
        let action = cx
            .build_action(&params.action, params.args)
            .map_err(|err| {
                let code = match cx.all_action_names().contains(&params.action.as_str()) {
                    true => code::INVALID_PARAMS,
                    false => code::NOT_FOUND,
                };
                RpcError::new(code, format!("Can't run {}: {err}", params.action))
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
                    "{} can't act where the keys are ({place}). ui.actions lists what can, and ui.focus moves the keys.",
                    params.action
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
            Some(ui::NAVIGATOR) => {
                if params.pane.is_some() {
                    return Err(RpcError::new(
                        code::INVALID_PARAMS,
                        "The navigator has no panes",
                    ));
                }
                if !self.navigator_open {
                    return Err(RpcError::new(
                        code::NOT_AVAILABLE,
                        "The navigator is hidden: app::ToggleNavigator shows it",
                    ));
                }
                let focus = gpui::Focusable::focus_handle(self.navigator.read(cx), cx);
                window.focus(&focus, cx);
                return Ok(());
            }
            Some(ui::TERMINALS) => {
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
            Some(id) => {
                let tile = (self.shown_tiles(cx).into_iter())
                    .map(|(tile, _)| tile)
                    .find(|tile| self.tile_id(tile, cx) == id)
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
            None => self.focused_tile(window, cx),
        };
        let Some(pane) = params.pane.as_deref() else {
            return Ok(());
        };
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

    fn tile_id(&self, tile: &Tile, cx: &App) -> String {
        match tile {
            Tile::Home => ui::HOME.to_owned(),
            Tile::Pinned(row) => tile_label(self.store.read(cx), row).1.to_string(),
        }
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
        let key = match ch {
            '\n' => "enter".to_owned(),
            '\t' => "tab".to_owned(),
            ' ' => "space".to_owned(),
            ch => ch.to_string(),
        };
        let mut keystroke = Keystroke::parse(&key).map_err(|err| {
            RpcError::new(code::INVALID_PARAMS, format!("Can't type {ch:?}: {err}"))
        })?;
        if !ch.is_control() {
            keystroke.key_char = Some(ch.to_string());
        }
        window.dispatch_keystroke(keystroke, cx);
    }
    Ok(())
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
    }
}
