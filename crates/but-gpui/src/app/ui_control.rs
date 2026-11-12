#![cfg_attr(
    not(unix),
    allow(dead_code, reason = "only the unix socket hands requests over")
)]

use but_tty::Status;
use but_ui_control::ui::{
    self, ActionInfo, Area as UiArea, CloseParams, DispatchParams, FocusParams, Input, Keys, Open,
    OpenParams, Opened, Read, ReadParams, Row, SelectParams, State, TerminalStatus, TypeParams,
};
use but_ui_control::{Request, RpcError, code};
use gpui::{App, AppContext as _, Context, Keystroke, Modifiers, Window};
use serde::de::DeserializeOwned;

use super::{AppView, KeyPlace, tile_label};
use crate::commands::{COMMANDS, title_of};
use crate::control::Reply;
use crate::detail::KeyPane;
use crate::state::{Axis, RowKey, Selection, Store, Tile};
use crate::terminal::{Terminal, TerminalId};

/// The actions a person never runs by name, as a field's own editing keys.
const UNLISTED: [&str; 2] = ["zed::", "stories::"];

const NAVIGATOR: &str = "navigator";
const TERMINALS: &str = "terminals";

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
        ui::TYPE => {
            let checked = params::<TypeParams>(&request).and_then(|params| {
                let pieces = pieces(params.input, params.tile.is_some())?;
                Ok((params.tile, pieces))
            });
            match checked {
                Ok((Some(tile), pieces)) => reply.answer(type_into(&app, &tile, pieces, cx)),
                Ok((None, pieces)) => type_pieces(app, pieces, reply, window, cx),
                Err(err) => reply.err(err),
            }
        }
        ui::READ => match params::<ReadParams>(&request) {
            Ok(params) => read(&app, params, reply, cx),
            Err(err) => reply.err(err),
        },
        ui::CLOSE => reply.answer(
            params::<CloseParams>(&request).and_then(|params| close(&app, &params.tile, cx)),
        ),
        ui::OPEN => match params::<OpenParams>(&request) {
            Ok(params) => open(&app, params, reply, cx),
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

/// A piece of what `ui.type` sends: text up to an Enter or a Tab, a keystroke a character, or
/// a key pressed by name.
enum Piece {
    Text(Vec<Keystroke>),
    Key(Keystroke),
}

/// `input` in pieces, every character and key checked first, so nothing goes in when one is
/// wrong. Sent `to_terminal` in the background, ⌘ keys are refused: they're the app's, and the
/// terminal would take them as the key without ⌘.
fn pieces(input: Vec<Input>, to_terminal: bool) -> Result<Vec<Piece>, RpcError> {
    let mut pieces = Vec::new();
    for piece in input {
        match piece {
            Input::Text(text) => {
                for line in text.split_inclusive(['\n', '\t']) {
                    let keystrokes = (line.chars())
                        .map(keystroke_for)
                        .collect::<Result<_, _>>()?;
                    pieces.push(Piece::Text(keystrokes));
                }
            }
            Input::Key(name) => pieces.push(Piece::Key(key_named(&name, to_terminal)?)),
        }
    }
    Ok(pieces)
}

/// The key `name` names, as keymaps write keys: `enter`, `up`, `ctrl-c`.
fn key_named(name: &str, to_terminal: bool) -> Result<Keystroke, RpcError> {
    let mut keystroke = Keystroke::parse(name).map_err(|err| {
        RpcError::new(code::INVALID_PARAMS, format!("Can't press {name:?}: {err}"))
    })?;
    if keystroke.key.contains(' ') || !crate::terminal::has_key(&keystroke) {
        return Err(RpcError::new(
            code::INVALID_PARAMS,
            format!(
                "{name:?} isn't a key: keys are named as keymaps name them, as enter, up or ctrl-c"
            ),
        ));
    }
    if to_terminal && keystroke.modifiers.platform {
        return Err(RpcError::new(
            code::NOT_AVAILABLE,
            format!(
                "{name} is the app's, not the terminal's: press it where the keys are, without a tile"
            ),
        ));
    }
    // A key pressed alone types what it shows, as a keyboard's does.
    if keystroke.modifiers == Modifiers::none() {
        keystroke.key_char = match keystroke.key.as_str() {
            "space" => Some(" ".to_owned()),
            key if key.chars().count() == 1 => Some(key.to_owned()),
            _ => None,
        };
    }
    Ok(keystroke)
}

/// Send `pieces` to the terminal tile `id` shows, in whichever row's tiling it is, leaving the
/// keys and the selection where they are.
fn type_into(
    app: &gpui::Entity<AppView>,
    id: &str,
    pieces: Vec<Piece>,
    cx: &mut App,
) -> Result<(), RpcError> {
    let (_, terminal) = terminal_in_tile(app.read(cx).store.read(cx), id)?;
    terminal.update(cx, |terminal, cx| {
        for piece in &pieces {
            match piece {
                Piece::Text(keystrokes) => {
                    for keystroke in keystrokes {
                        terminal.press(keystroke, cx);
                    }
                }
                Piece::Key(keystroke) => terminal.press(keystroke, cx),
            }
        }
    });
    Ok(())
}

/// The terminal tile `id` shows, in whichever row's tiling it is.
fn terminal_in_tile(
    store: &Store,
    id: &str,
) -> Result<(TerminalId, gpui::Entity<Terminal>), RpcError> {
    let (row, tile) = store.tile_numbered(id).ok_or_else(|| {
        RpcError::new(
            code::NOT_FOUND,
            format!("There's no tile {id}: ui.state lists the tiles"),
        )
    })?;
    let terminal = match tile.shows(&row) {
        RowKey::Row(Selection::Terminal(_, terminal)) => {
            (store.terminals.get(*terminal)).map(|entity| (*terminal, entity.clone()))
        }
        _ => None,
    };
    terminal.ok_or_else(|| {
        RpcError::new(
            code::NOT_AVAILABLE,
            format!(
                "Tile {id} shows no terminal, and only a terminal can be typed into or read without the keys"
            ),
        )
    })
}

/// Open what `params` says in a new tile beside tile `params.beside`, in whichever row's tiling
/// it is, leaving the keys and the selection where they are, and reply with the tile: for a
/// terminal, once its shell has started, and in a tile of its own if the one beside closed
/// meanwhile.
fn open(app: &gpui::Entity<AppView>, params: OpenParams, reply: Reply, cx: &mut App) {
    let store = app.read(cx).store.clone();
    let Some((row, at)) = store.read(cx).tile_numbered_there(&params.beside) else {
        return reply.err(RpcError::new(
            code::NOT_FOUND,
            format!(
                "There's no tile {}: ui.state lists the tiles",
                params.beside
            ),
        ));
    };
    let axis = match params.below {
        true => Axis::Down,
        false => Axis::Across,
    };
    let beside = (row.clone(), at.clone(), axis);
    match params.open {
        Open::Show(name) => {
            let project = at.shows(&row).project();
            let Some(pinned) = crate::palette::go_to_tile(store.read(cx), &name, project) else {
                return reply.err(RpcError::new(
                    code::NOT_FOUND,
                    format!("Go to anything finds nothing called {name}"),
                ));
            };
            let pinned_there = store.update(cx, |store, cx| {
                store.pin_quietly(beside, pinned.clone(), cx)
            });
            if !pinned_there {
                return reply.err(RpcError::new(
                    code::NOT_AVAILABLE,
                    format!(
                        "{name} can't open beside tile {}: it's in a tile of that layout already, or it's the row the layout is for",
                        params.beside
                    ),
                ));
            }
            reply.answer(opened(app, &row, &Tile::Pinned(pinned), cx));
        }
        Open::Terminal { run } => {
            let (sender, went) = futures::channel::oneshot::channel();
            let starting = store.update(cx, |store, cx| {
                store.pin_terminal_quietly(beside, run, sender, cx)
            });
            if !starting {
                return reply.err(RpcError::new(
                    code::NOT_AVAILABLE,
                    format!(
                        "Tile {} shows nothing a terminal can open on",
                        params.beside
                    ),
                ));
            }
            let app = app.clone();
            cx.spawn(async move |cx| {
                let answer = match went.await {
                    Ok(went) => cx.update(|cx| opened(&app, &went.row, &went.tile, cx)),
                    Err(_) => Err(RpcError::new(
                        code::INTERNAL_ERROR,
                        "The terminal's shell didn't start",
                    )),
                };
                reply.answer(answer);
            })
            .detach();
        }
    }
}

/// Close tile `id`, in whichever row's tiling it is, as its × does, leaving the keys and the
/// selection where they are, and without asking first: a terminal closing with it stops what
/// runs in it. A page's home tile closes only with its row.
fn close(app: &gpui::Entity<AppView>, id: &str, cx: &mut App) -> Result<(), RpcError> {
    let store = app.read(cx).store.clone();
    let (row, tile) = store.read(cx).tile_numbered(id).ok_or_else(|| {
        RpcError::new(
            code::NOT_FOUND,
            format!("There's no tile {id}: ui.state lists the tiles"),
        )
    })?;
    match (tile, row) {
        (Tile::Pinned(pinned), row) => {
            store.update(cx, |store, cx| store.close_tile_in(&row, &pinned, cx));
        }
        (Tile::Home, RowKey::Row(Selection::Terminal(_, terminal))) => {
            store.update(cx, |store, cx| store.close_terminal(terminal, cx));
        }
        (Tile::Home, _) => {
            return Err(RpcError::new(
                code::NOT_AVAILABLE,
                format!("Tile {id} is the page its layout is for, and closes only with its row"),
            ));
        }
    }
    Ok(())
}

/// The tile `tile` of `row`'s tiling, as `ui.open` replies with it.
fn opened(
    app: &gpui::Entity<AppView>,
    row: &RowKey,
    tile: &Tile,
    cx: &App,
) -> Result<Opened, RpcError> {
    let app = app.read(cx);
    let id = (app.store.read(cx).tile_id_in(row, tile))
        .ok_or_else(|| RpcError::new(code::INTERNAL_ERROR, "The tile closed as it opened"))?;
    Ok(Opened {
        id: id.to_string(),
        shows: app.row_shows(tile.shows(row), cx),
    })
}

/// Reply with what the terminal tile `params.tile` shows, in whichever row's tiling it is, once
/// its emulator has the text.
fn read(app: &gpui::Entity<AppView>, params: ReadParams, reply: Reply, cx: &mut App) {
    let store = app.read(cx).store.read(cx);
    let (id, terminal) = match terminal_in_tile(store, &params.tile) {
        Ok(found) => found,
        Err(err) => return reply.err(err),
    };
    let (Some(title), Some(status)) = (store.terminals.title(id), store.terminals.status(id))
    else {
        return reply.err(RpcError::new(
            code::NOT_FOUND,
            format!("Tile {}'s terminal closed", params.tile),
        ));
    };
    let (title, status) = (title.to_string(), status_of(status));
    let text = terminal.read(cx).text();
    cx.spawn(async move |_| match text.await {
        Ok(Some(text)) => reply.ok(Read {
            title,
            status,
            text: last_lines(&text, params.lines),
        }),
        Ok(None) | Err(_) => reply.err(RpcError::new(
            code::INTERNAL_ERROR,
            format!("Tile {}'s terminal couldn't be read", params.tile),
        )),
    })
    .detach();
}

fn status_of(status: &Status) -> TerminalStatus {
    match status {
        Status::Idle => TerminalStatus::Idle,
        Status::Working => TerminalStatus::Working,
        Status::Blocked(why) => TerminalStatus::Blocked {
            reason: reason_of(why),
        },
        Status::Done => TerminalStatus::Done,
        Status::Failed(why) => TerminalStatus::Failed {
            reason: reason_of(why),
        },
    }
}

/// The reason a program gave, if it gave one.
fn reason_of(why: &str) -> Option<String> {
    (!why.is_empty()).then(|| why.to_owned())
}

/// The last `lines` of `text`, or all of it without.
fn last_lines(text: &str, lines: Option<usize>) -> String {
    let Some(lines) = lines else {
        return text.to_owned();
    };
    let all: Vec<&str> = text.lines().collect();
    let skip = all.len().saturating_sub(lines);
    all.into_iter().skip(skip).collect::<Vec<_>>().join("\n")
}

/// Send `pieces` where the keys are, outside the app's update, because a key's handler updates
/// the app itself. A piece per update, so where the one before sent the keys is settled and
/// checked before the next goes in. Text goes only into a text field or a terminal, as its
/// letters would run shortcuts elsewhere; a key goes wherever the keys are.
fn type_pieces(
    app: gpui::Entity<AppView>,
    pieces: Vec<Piece>,
    reply: Reply,
    window: &mut Window,
    cx: &mut App,
) {
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        for (typed, piece) in pieces.into_iter().enumerate() {
            let result = cx.update_window(handle, |_, window, cx| {
                window.draw(cx).clear(cx);
                match piece {
                    Piece::Text(keystrokes) => {
                        let text_field = app.read(cx).ui_keys(window, cx).text_field;
                        type_text(keystrokes, text_field, window, cx)
                    }
                    Piece::Key(keystroke) => {
                        window.dispatch_keystroke(keystroke, cx);
                        Ok(())
                    }
                }
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
                        "After {typed} pieces, each a key or text up to an Enter or Tab: {}",
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
        let on_screen: Vec<Tile> = match self.detail_open {
            true => (self.shown_tiles(cx).into_iter())
                .map(|(tile, _)| tile)
                .collect(),
            false => Vec::new(),
        };
        let selected = (store.selection.as_ref()).map(|selection| store.row_key(selection));
        let layouts = (store.layouts().into_iter())
            .map(|layout| {
                let open = self.detail_open && Some(&layout.row) == selected.as_ref();
                let tiles = (layout.tiles.into_iter())
                    .map(|(id, tile, shown, area)| {
                        let panes = match open && on_screen.contains(&tile) {
                            true => (self.tile_detail(&tile, cx).iter())
                                .flat_map(|detail| detail.read(cx).key_panes(cx))
                                .map(|pane| pane.name().to_owned())
                                .collect(),
                            false => Vec::new(),
                        };
                        ui::Tile {
                            id: id.to_string(),
                            shows: self.row_shows(&shown, cx),
                            area: UiArea {
                                left: area.left,
                                top: area.top,
                                width: area.width,
                                height: area.height,
                            },
                            panes,
                        }
                    })
                    .collect();
                ui::Layout {
                    row: self.row_shows(&layout.row, cx),
                    open,
                    home: layout.home.to_string(),
                    zoomed: (self.zoomed().filter(|_| open))
                        .and_then(|tile| store.tile_id(tile))
                        .map(|id| id.to_string()),
                    tiles,
                }
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
            layouts,
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
            KeyPlace::Navigator => (Some(NAVIGATOR.to_owned()), None),
            KeyPlace::Terminals => (Some(TERMINALS.to_owned()), None),
            KeyPlace::Tile(tile, pane) => (
                self.tile_id(&tile, cx),
                pane.map(|pane| pane.name().to_owned()),
            ),
        };
        Keys {
            tile,
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
            Some(id) => {
                let tile = self.tile_named(id, cx)?;
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
        let named =
            (self.tile_id(&tile, cx)).map_or_else(|| "It".to_owned(), |id| format!("Tile {id}"));
        let detail = self.tile_detail(&tile, cx).ok_or_else(|| {
            RpcError::new(
                code::NOT_AVAILABLE,
                format!("{named} shows a terminal, which has no panes"),
            )
        })?;
        let panes = detail.read(cx).key_panes(cx);
        let found = KeyPane::named(pane).filter(|pane| panes.contains(pane));
        let Some(pane) = found else {
            let names: Vec<&str> = panes.iter().map(|pane| pane.name()).collect();
            return Err(RpcError::new(
                code::NOT_AVAILABLE,
                format!(
                    "{named} has no {pane} pane now; it has {}",
                    names.join(", ")
                ),
            ));
        };
        detail.update(cx, |detail, cx| detail.focus_key_pane(pane, window, cx));
        Ok(())
    }

    /// The tile on screen numbered `id`, as [`State::layouts`] lists it.
    fn tile_named(&self, id: &str, cx: &App) -> Result<Tile, RpcError> {
        if !self.detail_open {
            return Err(detail_hidden());
        }
        let store = self.store.read(cx);
        (self.shown_tiles(cx).into_iter())
            .map(|(tile, _)| tile)
            .find(|tile| store.tile_id(tile).is_some_and(|shown| shown.to_string() == id))
            .ok_or_else(|| match store.tile_numbered_there(id) {
                Some(_) => RpcError::new(
                    code::NOT_AVAILABLE,
                    format!(
                        "Tile {id} isn't on screen: its layout isn't open, or another tile is zoomed over it"
                    ),
                ),
                None => RpcError::new(
                    code::NOT_FOUND,
                    format!("There's no tile {id}: ui.state lists the tiles"),
                ),
            })
    }

    fn tile_id(&self, tile: &Tile, cx: &App) -> Option<String> {
        (self.store.read(cx).tile_id(tile)).map(|id| id.to_string())
    }

    /// What `row` is, as a tile showing it shows it.
    fn row_shows(&self, row: &RowKey, cx: &App) -> Row {
        let store = self.store.read(cx);
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

fn type_text(
    keystrokes: Vec<Keystroke>,
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
    for keystroke in keystrokes {
        window.dispatch_keystroke(keystroke, cx);
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

#[cfg(test)]
mod tests {
    use but_ui_control::ui::Input;

    use super::{key_named, keystroke_for, last_lines, pieces, types_text};

    #[test]
    fn a_key_is_named_as_keymaps_name_it() {
        let keystroke = key_named("ctrl-c", true).unwrap();
        assert!(
            keystroke.key == "c" && keystroke.modifiers.control,
            "ctrl-c is c with ctrl held"
        );
    }

    #[test]
    fn a_name_that_isnt_a_key_is_refused() {
        assert!(
            key_named("enterr", false).is_err(),
            "a misspelt key would send nothing"
        );
    }

    #[test]
    fn a_cmd_key_is_refused_for_a_terminal_in_the_background() {
        assert!(
            key_named("cmd-k", true).is_err(),
            "the terminal would take it as k"
        );
        assert!(
            key_named("cmd-k", false).is_ok(),
            "where the keys are, it's the app's shortcut"
        );
    }

    #[test]
    fn a_key_pressed_alone_types_what_it_shows() {
        assert_eq!(
            key_named("a", true).unwrap().key_char.as_deref(),
            Some("a"),
            "as a keyboard's a does"
        );
        assert_eq!(
            key_named("enter", true).unwrap().key_char,
            None,
            "enter types nothing"
        );
    }

    #[test]
    fn one_bad_key_refuses_all_of_it() {
        let input = Vec::from([
            Input::Text("ls".to_owned()),
            Input::Key("enter".to_owned()),
            Input::Key("nope".to_owned()),
        ]);
        assert!(
            pieces(input, true).is_err(),
            "nothing goes in when a piece is wrong"
        );
    }

    #[test]
    fn text_goes_in_pieces_up_to_each_enter() {
        let input = Vec::from([Input::Text("a\nb".to_owned()), Input::Key("up".to_owned())]);
        assert_eq!(
            pieces(input, true).unwrap().len(),
            3,
            "the text before the enter, after it, and the key"
        );
    }

    #[test]
    fn the_last_lines_are_the_ones_asked_for() {
        assert_eq!(
            last_lines("one\ntwo\nthree", Some(2)),
            "two\nthree",
            "the lines at the end"
        );
    }

    #[test]
    fn more_lines_than_there_are_is_all_of_them() {
        assert_eq!(
            last_lines("one\ntwo", Some(5)),
            "one\ntwo",
            "nothing is made up"
        );
    }

    #[test]
    fn no_count_is_all_of_it() {
        assert_eq!(last_lines("one\ntwo", None), "one\ntwo", "the whole text");
    }

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
}
