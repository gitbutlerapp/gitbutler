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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub navigator: bool,
    pub detail: bool,
    pub selection: Option<Row>,
    pub tiles: Vec<Tile>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tile {
    pub id: String,
    pub shows: Row,
    pub area: Area,
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
    /// A newline presses Enter.
    pub text: String,
}
