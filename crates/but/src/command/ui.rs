//! Implementation of the `but _ui` command, a client of the GPUI app's control socket.

use but_ui_control::Client;
use but_ui_control::ui::{
    self, ActionInfo, DispatchParams, FocusParams, Row, SelectParams, State, TypeParams,
};
use serde::Serialize;

use crate::{
    CliResult,
    args::ui::{Platform, Subcommands},
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
}

#[must_use]
enum UiOutcome {
    State(State),
    Actions(Vec<ActionInfo>),
    Dispatched(String),
    Selected(Row),
    Focused(FocusParams),
    Typed(usize),
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
        Subcommands::Type { text } => UiOperation::Type(TypeParams { text }),
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
            UiOutcome::Typed(params.text.chars().count())
        }
    })
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
                writeln!(out, "Tiles:")?;
                for tile in &state.tiles {
                    let panes = match tile.panes.is_empty() {
                        true => String::new(),
                        false => format!(" (panes: {})", tile.panes.join(", ")),
                    };
                    writeln!(out, "  {}: {}{panes}", tile.id, row_display(&tile.shows))?;
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
                    writeln!(out, "Open over the window: {overlay:?}")?;
                }
            }
            UiOutcome::Actions(actions) => {
                for action in actions {
                    let keys = action
                        .keys
                        .map(|keys| format!(" [{keys}]"))
                        .unwrap_or_default();
                    let risky = if action.risky { " (asks first)" } else { "" };
                    writeln!(out, "{}  {}{keys}{risky}", action.name, action.title)?;
                }
            }
            UiOutcome::Dispatched(action) => writeln!(out, "Ran {action}.")?,
            UiOutcome::Selected(row) => writeln!(out, "Selected {}.", row_display(&row))?,
            UiOutcome::Focused(FocusParams { tile, pane }) => {
                let tile = tile.unwrap_or_else(|| "the tile with the keys".to_owned());
                match pane {
                    Some(pane) => writeln!(out, "Gave the keys to {tile} › {pane}.")?,
                    None => writeln!(out, "Gave the keys to {tile}.")?,
                }
            }
            UiOutcome::Typed(count) => writeln!(out, "Typed {count} characters.")?,
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
            UiOutcome::Typed(count) => serde_json::to_value(serde_json::json!({ "typed": count })),
        }
        .unwrap_or_default()
    }
}
