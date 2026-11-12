use serde::{Deserialize, Serialize};

/// Result: [`State`].
pub const STATE: &str = "ui.state";
/// Result: `Vec<`[`ActionInfo`]`>`, the actions that would act where the keys are.
pub const ACTIONS: &str = "ui.actions";
/// Params: [`DispatchParams`].
pub const DISPATCH: &str = "ui.dispatch";
/// Params: [`SelectParams`]. Result: the [`Row`] selected.
pub const SELECT: &str = "ui.select";
/// Params: [`FocusParams`].
pub const FOCUS: &str = "ui.focus";
/// Params: [`TypeParams`].
pub const TYPE: &str = "ui.type";
/// Params: [`ReadParams`]. Result: [`Read`].
pub const READ: &str = "ui.read";
/// Params: [`OpenParams`]. Result: [`Opened`].
pub const OPEN: &str = "ui.open";
/// Params: [`CloseParams`].
pub const CLOSE: &str = "ui.close";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub navigator: bool,
    pub detail: bool,
    pub selection: Option<Row>,
    /// Each row's tiles, the open layout's first, then the rest in the order they were made.
    pub layouts: Vec<Layout>,
    pub keys: Keys,
    /// What's open over the window and takes the keys, as `palette`.
    pub overlay: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub project: Option<String>,
    pub kind: String,
    pub label: String,
}

/// How a row lays out its tiles: the page it shows itself, its home tile, and any pinned
/// beside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub row: Row,
    /// It's the one on screen: the selected row's, while the detail shows.
    pub open: bool,
    /// The number of its home tile, the one showing the row itself.
    pub home: String,
    /// The number of the tile zoomed over the others, while one is.
    pub zoomed: Option<String>,
    /// Its tiles, in the order they're laid out.
    pub tiles: Vec<Tile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    /// Its number, which it keeps until it closes, wherever it moves and whatever it shows. A
    /// tile opened again gets a new one, and the numbers start over with the app.
    pub id: String,
    pub shows: Row,
    /// Where it is in its layout, zoomed over or not.
    pub area: Area,
    /// The panes that can take the keys now: none while it's not on screen.
    pub panes: Vec<String>,
}

/// Fractions of the room the tiles share.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Area {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keys {
    pub tile: Option<String>,
    pub pane: Option<String>,
    /// Keys there type text, as in a text field or a terminal, instead of running actions.
    pub text_field: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionInfo {
    pub name: String,
    pub title: String,
    pub keys: Option<String>,
    /// It throws work away.
    pub risky: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DispatchParams {
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectParams {
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FocusParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeParams {
    /// What to send, in order.
    pub input: Vec<Input>,
    /// Send it to this tile's terminal instead, leaving the keys where they are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<String>,
}

/// A piece of what `ui.type` sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Input {
    /// Text as it is: a newline presses Enter, and a tab Tab.
    Text(String),
    /// A key press by name, as `enter`, `up` or `ctrl-c`, as keymaps and `ui.actions` write keys.
    Key(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadParams {
    /// The tile whose terminal to read, in whichever layout it is.
    pub tile: String,
    /// How many of the last lines to read. All of them, scrollback and screen, without.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<usize>,
}

/// What a terminal shows, and what the program running in it says it's doing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Read {
    pub title: String,
    pub status: TerminalStatus,
    /// Its lines as plain text, those the terminal wrapped joined again.
    pub text: String,
}

/// What the program running in a terminal reports it's doing, as Claude Code does with its
/// progress. A program that reports nothing, as a shell's commands, stays `idle`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TerminalStatus {
    Idle,
    Working,
    /// It can't go on until the user acts.
    Blocked {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// It finished, for the user to look at.
    Done,
    Failed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenParams {
    /// The tile to open it beside, in whichever layout it is.
    pub beside: String,
    /// Below it, instead of to its right.
    #[serde(default)]
    pub below: bool,
    pub open: Open,
}

/// What `ui.open` opens in a new tile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Open {
    /// A new terminal, on the checkout of what the tile beside shows.
    Terminal {
        /// A command to run in it, through the login shell.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        run: Option<String>,
    },
    /// A row or a project's page, by name, as Go to anything finds it.
    Show(String),
}

/// The tile `ui.open` opened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Opened {
    pub id: String,
    pub shows: Row,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseParams {
    /// The tile to close, in whichever layout it is.
    pub tile: String,
}
