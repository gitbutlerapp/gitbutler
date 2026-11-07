//! `but _view` and `but _wait`: what the GitButler GUI shows, and what its user tells agents,
//! asked over the socket it listens at while it runs.

use std::time::Duration;

use anyhow::anyhow;
use but_path::AppChannel;
use but_path::app::{self, Event, EventKind, Place, View, Wait, Waited, method};

use crate::theme::Theme;
use crate::utils::rejection::shell_quote;
use crate::utils::{CliOutput, CliOutputHuman, WriteWithUtils};

/// What the CLI says when no GUI listens.
pub(crate) const NOT_RUNNING: &str = "GitButler isn't running: start it with `but gui`";

/// How long the GUI has to say what it shows.
const VIEW_TIMEOUT: Duration = Duration::from_secs(10);

/// What the GUI shows.
pub(crate) fn view() -> anyhow::Result<ViewOutcome> {
    let view: Option<View> = app::call(
        AppChannel::new(),
        method::VIEW,
        &serde_json::json!({}),
        Some(VIEW_TIMEOUT),
    )?;
    view.map(ViewOutcome).ok_or_else(|| anyhow!(NOT_RUNNING))
}

/// What the GUI's user tells agents after event `after`, or with none, what they tell next,
/// waiting up to `timeout` seconds, or for as long as it takes.
pub(crate) fn wait(after: Option<u64>, timeout: Option<u64>) -> anyhow::Result<WaitOutcome> {
    let wait = Wait {
        after,
        timeout_secs: timeout,
    };
    // A little past the GUI's own timeout, so its empty answer gets through.
    let read_timeout = timeout.map(|secs| Duration::from_secs(secs + 10));
    let waited: Option<Waited> = app::call(AppChannel::new(), method::WAIT, &wait, read_timeout)?;
    waited.map(WaitOutcome).ok_or_else(|| anyhow!(NOT_RUNNING))
}

/// `place` as `but _open … --gui` takes it: each parameter `key=value`, quoted for a shell
/// where it needs it.
pub(crate) fn place_args(place: &Place) -> String {
    (place.0.iter())
        .map(|(key, value)| shell_quote(&format!("{key}={value}")).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `view` in a line: its title and place, and its project's folder.
pub(crate) fn view_line(view: &View) -> String {
    let Some(path) = &view.path else {
        return "no project".into();
    };
    let title = view.title.as_deref().unwrap_or("its project");
    format!("{title}: {} in {}", place_args(&view.at), path.display())
}

pub(crate) struct ViewOutcome(View);

impl CliOutputHuman for ViewOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        match self.0.path {
            Some(_) => writeln!(out, "GitButler shows {}", view_line(&self.0))?,
            None => writeln!(out, "GitButler shows no project")?,
        }
        Ok(())
    }
}

impl CliOutput for ViewOutcome {
    fn on_json(self) -> impl serde::Serialize {
        self.0
    }
}

pub(crate) struct WaitOutcome(Waited);

impl CliOutputHuman for WaitOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let Waited { events, last } = self.0;
        if events.is_empty() {
            writeln!(out, "The user said nothing in time")?;
        }
        for Event { seq, kind, view } in events {
            match kind {
                EventKind::Asked { note } => {
                    writeln!(out, "#{seq} The user asks: {note}")?;
                    writeln!(out, "  about {}", view_line(&view))?;
                }
                EventKind::Next => writeln!(out, "#{seq} The user is ready for the next step")?,
                EventKind::Done => writeln!(out, "#{seq} The user ended the walkthrough")?,
            }
        }
        writeln!(out, "Wait for more with: but _wait --after {last}")?;
        Ok(())
    }
}

impl CliOutput for WaitOutcome {
    fn on_json(self) -> impl serde::Serialize {
        self.0
    }
}
