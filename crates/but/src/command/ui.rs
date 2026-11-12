//! Implementation of the `but _ui` command.

use but_ui_control::Client;
use but_ui_control::ui::{
    self, ActionInfo, CloseParams, DispatchParams, FocusParams, Input, Open, OpenParams, Opened,
    Read, ReadParams, Row, SelectParams, State, TerminalStatus, TypeParams,
};
use serde::Serialize;

use crate::{
    CliResult,
    args::ui::{Platform, Subcommands, TypeArgs},
    bad_input,
    theme::Theme,
    utils::{CliOutput, CliOutputHuman, OutputChannel, WriteWithUtils},
};

pub fn handle(out: &mut OutputChannel, args: Platform) -> CliResult<()> {
    let operation = resolve(args)?;
    let outcome = run(operation)?;
    out.print_cli_output(outcome)?;
    Ok(())
}

#[derive(Debug)]
enum UiOperation {
    State,
    Actions,
    Dispatch(DispatchParams),
    Select(SelectParams),
    Focus(FocusParams),
    Type(TypeParams),
    Read(ReadParams),
    Open(OpenParams),
    Close(CloseParams),
}

#[must_use]
enum UiOutcome {
    State(State),
    Actions(Vec<ActionInfo>),
    Dispatched(String),
    Selected(Row),
    Focused(FocusParams),
    Typed { characters: usize, keys: usize },
    Read(Read),
    Opened(Opened),
    Closed(String),
}

fn resolve(args: Platform) -> CliResult<UiOperation> {
    Ok(match args.cmd {
        Subcommands::State => UiOperation::State,
        Subcommands::Actions => UiOperation::Actions,
        Subcommands::Dispatch {
            action,
            args,
            tile,
            pane,
        } => {
            let args = match args {
                Some(json) => Some(serde_json::from_str(&json).map_err(|err| {
                    bad_input(format!("The args aren't JSON: {err}")).arg_name("JSON")
                })?),
                None => None,
            };
            UiOperation::Dispatch(DispatchParams {
                action,
                args,
                tile,
                pane,
            })
        }
        Subcommands::Select { query } => UiOperation::Select(SelectParams { query }),
        Subcommands::Focus { tile, pane } => UiOperation::Focus(FocusParams { tile, pane }),
        Subcommands::Type(TypeArgs { tile, input }) => {
            UiOperation::Type(TypeParams { input, tile })
        }
        Subcommands::Read { tile, lines } => UiOperation::Read(ReadParams { tile, lines }),
        Subcommands::Close { tile } => UiOperation::Close(CloseParams { tile }),
        Subcommands::Open {
            beside,
            below,
            terminal: _,
            run,
            show,
        } => {
            // clap has it be one of them.
            let open = match show {
                Some(name) => Open::Show(name),
                None => Open::Terminal { run },
            };
            UiOperation::Open(OpenParams {
                beside,
                below,
                open,
            })
        }
    })
}

fn run(operation: UiOperation) -> anyhow::Result<UiOutcome> {
    let mut client = Client::connect()?;
    Ok(match operation {
        UiOperation::State => UiOutcome::State(client.call(ui::STATE, ())?),
        UiOperation::Actions => UiOutcome::Actions(client.call(ui::ACTIONS, ())?),
        UiOperation::Dispatch(params) => {
            client.call::<()>(ui::DISPATCH, &params)?;
            UiOutcome::Dispatched(params.action)
        }
        UiOperation::Select(params) => UiOutcome::Selected(client.call(ui::SELECT, &params)?),
        UiOperation::Focus(params) => {
            client.call::<()>(ui::FOCUS, &params)?;
            UiOutcome::Focused(params)
        }
        UiOperation::Type(params) => {
            client.call::<()>(ui::TYPE, &params)?;
            let (mut characters, mut keys) = (0, 0);
            for piece in &params.input {
                match piece {
                    Input::Text(text) => characters += text.chars().count(),
                    Input::Key(_) => keys += 1,
                }
            }
            UiOutcome::Typed { characters, keys }
        }
        UiOperation::Read(params) => UiOutcome::Read(client.call(ui::READ, &params)?),
        UiOperation::Open(params) => UiOutcome::Opened(client.call(ui::OPEN, &params)?),
        UiOperation::Close(params) => {
            client.call::<()>(ui::CLOSE, &params)?;
            UiOutcome::Closed(params.tile)
        }
    })
}

fn status_display(status: &TerminalStatus) -> String {
    match status {
        TerminalStatus::Idle => "idle".to_owned(),
        TerminalStatus::Working => "working".to_owned(),
        TerminalStatus::Blocked { reason } => with_reason("blocked", reason.as_deref()),
        TerminalStatus::Done => "done".to_owned(),
        TerminalStatus::Failed { reason } => with_reason("failed", reason.as_deref()),
    }
}

fn with_reason(state: &str, reason: Option<&str>) -> String {
    match reason {
        Some(reason) => format!("{state} ({reason})"),
        None => state.to_owned(),
    }
}

fn row_display(row: &Row) -> String {
    let project = (row.project.as_ref())
        .map(|p| format!(" in {p}"))
        .unwrap_or_default();
    format!("{} {}{project}", row.kind, row.label)
}

impl CliOutputHuman for UiOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        match self {
            UiOutcome::State(state) => {
                let shown = |on: bool| if on { "shown" } else { "hidden" };
                writeln!(
                    out,
                    "Navigator {}, detail {}.",
                    shown(state.navigator),
                    shown(state.detail)
                )?;
                match &state.selection {
                    Some(row) => writeln!(out, "Selected: {}", row_display(row))?,
                    None => writeln!(out, "Selected: nothing")?,
                }
                for layout in &state.layouts {
                    let open = if layout.open { "Open" } else { "Not open" };
                    writeln!(out, "{open}: {}", row_display(&layout.row))?;
                    for tile in &layout.tiles {
                        let mut marks = Vec::new();
                        if tile.id == layout.home {
                            marks.push("home".to_owned());
                        }
                        if layout.zoomed.as_ref() == Some(&tile.id) {
                            marks.push("zoomed".to_owned());
                        }
                        if !tile.panes.is_empty() {
                            marks.push(format!("panes: {}", tile.panes.join(", ")));
                        }
                        let marks = match marks.is_empty() {
                            true => String::new(),
                            false => format!(" ({})", marks.join("; ")),
                        };
                        writeln!(out, "  {}: {}{marks}", tile.id, row_display(&tile.shows))?;
                    }
                }
                let keys = match (&state.keys.tile, &state.keys.pane) {
                    (Some(tile), Some(pane)) => format!("{tile} › {pane}"),
                    (Some(tile), None) => tile.clone(),
                    _ => "not in the window".to_owned(),
                };
                let field = if state.keys.text_field {
                    ", where typing goes in as text"
                } else {
                    ""
                };
                writeln!(out, "Keys: {keys}{field}")?;
                if let Some(overlay) = state.overlay {
                    writeln!(out, "Open over the window: {overlay}")?;
                }
            }
            UiOutcome::Actions(actions) => {
                for action in actions {
                    let keys = action
                        .keys
                        .map(|keys| format!(" [{keys}]"))
                        .unwrap_or_default();
                    let risky = if action.risky { " (risky)" } else { "" };
                    writeln!(out, "{}  {}{keys}{risky}", action.name, action.title)?;
                }
            }
            UiOutcome::Dispatched(action) => writeln!(out, "Ran {action}.")?,
            UiOutcome::Selected(row) => writeln!(out, "Selected {}.", row_display(&row))?,
            UiOutcome::Focused(FocusParams { tile, pane }) => match (tile, pane) {
                (Some(tile), Some(pane)) => writeln!(out, "Gave the keys to {tile} › {pane}.")?,
                (Some(tile), None) => writeln!(out, "Gave the keys to {tile}.")?,
                (None, Some(pane)) => writeln!(out, "Gave the keys to the {pane} pane.")?,
                (None, None) => writeln!(out, "The keys stay where they are.")?,
            },
            UiOutcome::Typed { characters, keys } => writeln!(
                out,
                "Typed {characters} characters and pressed {keys} keys."
            )?,
            UiOutcome::Closed(tile) => writeln!(out, "Closed tile {tile}.")?,
            UiOutcome::Opened(opened) => writeln!(
                out,
                "Opened tile {}: {}.",
                opened.id,
                row_display(&opened.shows)
            )?,
            UiOutcome::Read(read) => {
                writeln!(out, "{}: {}", read.title, status_display(&read.status))?;
                writeln!(out)?;
                writeln!(out, "{}", read.text)?;
            }
        }
        Ok(())
    }
}

impl CliOutput for UiOutcome {
    fn on_json(self) -> impl Serialize {
        match self {
            UiOutcome::State(state) => serde_json::to_value(state),
            UiOutcome::Actions(actions) => serde_json::to_value(actions),
            UiOutcome::Dispatched(action) => {
                serde_json::to_value(serde_json::json!({ "dispatched": action }))
            }
            UiOutcome::Selected(row) => serde_json::to_value(row),
            UiOutcome::Focused(params) => serde_json::to_value(params),
            UiOutcome::Typed { characters, keys } => {
                serde_json::to_value(serde_json::json!({ "typed": characters, "pressed": keys }))
            }
            UiOutcome::Read(read) => serde_json::to_value(read),
            UiOutcome::Opened(opened) => serde_json::to_value(opened),
            UiOutcome::Closed(tile) => serde_json::to_value(serde_json::json!({ "closed": tile })),
        }
        .unwrap_or_default()
    }
}
