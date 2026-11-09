//! Driving the app's UI as a person at the keyboard does. Everything the app can do is an
//! action with a name, as `tiling::SplitRight` or `remote::Push`, which acts where the keys
//! are, as a key bound to it would: [`ACTIONS`] lists the ones that would act there now, and
//! [`DISPATCH`] runs one. [`SELECT`] and [`FOCUS`] move the keys, as a click does, and [`TYPE`]
//! types into the field that has them. [`STATE`] says what shows and where the keys are.
//!
//! An action that would throw work away asks first in the app, as it does from a key.

use serde::{Deserialize, Serialize};

/// What shows and where the keys are: [`State`].
pub const STATE: &str = "ui.state";
/// The actions that would act where the keys are: `Vec<`[`ActionInfo`]`>`.
pub const ACTIONS: &str = "ui.actions";
/// Run an action: [`DispatchParams`] in, `null` out.
pub const DISPATCH: &str = "ui.dispatch";
/// Select a row, as Go to anything finds it: [`SelectParams`] in, the [`Row`] selected out.
pub const SELECT: &str = "ui.select";
/// Give a tile or a pane the keys: [`FocusParams`] in, `null` out.
pub const FOCUS: &str = "ui.focus";
/// Type into the field that has the keys: [`TypeParams`] in, `null` out.
pub const TYPE: &str = "ui.type";

/// The navigator's tile id.
pub const NAVIGATOR: &str = "navigator";
/// The tile id of the terminal overview under the navigator.
pub const TERMINALS: &str = "terminals";
/// The tile id of the tile that follows the navigator's selection.
pub const HOME: &str = "home";

/// The window as it shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// The navigator shows.
    pub navigator: bool,
    /// The detail, right of the navigator, shows.
    pub detail: bool,
    /// The navigator's selected row.
    pub selection: Option<Row>,
    /// The home tile first.
    pub tiles: Vec<Tile>,
    /// Where the keys are.
    pub keys: Keys,
    /// What's open over the window and takes the keys.
    pub overlay: Option<Overlay>,
}

/// A row of the navigator, or a page reached from one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// The project's name.
    pub project: Option<String>,
    /// `project`, `branch`, `commit`, `terminal` and so on.
    pub kind: String,
    /// What the app shows for it.
    pub label: String,
}

/// A tile of the window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    /// [`HOME`], or the label of the row it shows.
    pub id: String,
    /// The row it shows.
    pub shows: Row,
    /// Where it is.
    pub area: Area,
    /// The panes of its page that take the keys, as `files` or `diff`, in reading order.
    pub panes: Vec<String>,
}

/// A rectangle as fractions of the room the tiles share, from its top left.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct Area {
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
}

/// Where the keys are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Keys {
    /// A tile id, [`NAVIGATOR`] or [`TERMINALS`]; `None` when nothing in the window has them.
    pub tile: Option<String>,
    /// The pane of that tile's page, as `diff`.
    pub pane: Option<String>,
    /// Keys there type text, as in a text field or a terminal, instead of running actions.
    pub text_field: bool,
}

/// What's open over the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Overlay {
    /// ⌘K's palette.
    Palette,
    /// A right-click or ⋯ menu.
    Menu,
    /// A dialog, as a form or settings.
    Modal,
    /// A question the app asks before it goes on.
    Prompt,
}

/// An action that would act where the keys are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionInfo {
    /// As `tiling::SplitRight`.
    pub name: String,
    /// What ⌘K shows.
    pub title: String,
    /// As `cmd-k`.
    pub keys: Option<String>,
    /// It throws work away, so the app asks first.
    pub risky: bool,
}

/// An action to run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DispatchParams {
    /// As [`ActionInfo::name`].
    pub action: String,
    /// The action's argument, as a keymap entry gives it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
    /// Give this tile the keys first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<String>,
    /// Give this pane the keys first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<String>,
}

/// A row to select.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectParams {
    /// Matched as Go to anything does: the whole name first, ignoring case, else part of it,
    /// in the selected project first.
    pub query: String,
}

/// Where to give the keys.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FocusParams {
    /// A tile id or [`NAVIGATOR`]; `None` for the tile with the keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<String>,
    /// As `files` or `diff`; `None` for where a click on the tile leaves the keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane: Option<String>,
}

/// Text to type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeParams {
    /// A newline presses Enter.
    pub text: String,
}
