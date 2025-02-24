use std::io::Write;

mod output_channel;
pub use output_channel::OutputChannel;

pub mod metrics;
#[cfg(feature = "legacy")]
pub use metrics::types::BackgroundMetrics;
pub use metrics::types::OneshotMetricsContext;

/// Utilities attached to `anyhow::Result<impl serde::Serialize>`.
pub trait ResultJsonExt {
    /// Write this value as pretty `JSON` to stdout if `json` is `true`.
    ///
    /// This style is great if you don't want to forget that JSON must be implemented.
    /// Note that "null" isn't printed and silently dropped.
    fn output_json(self, json: bool) -> anyhow::Result<()>;
}

pub trait ResultErrorExt {
    fn show_root_cause_error_then_exit_without_destructors(self, out: OutputChannel) -> !;
}

impl ResultErrorExt for anyhow::Result<()> {
    fn show_root_cause_error_then_exit_without_destructors(self, out: OutputChannel) -> ! {
        // Trigger the pager to be flushed before exiting early, or destructors aren't called.
        drop(out);
        let code = if let Err(e) = &self {
            writeln!(std::io::stderr(), "{} {}", e, e.root_cause()).ok();
            1
        } else {
            0
        };
        std::process::exit(code);
    }
}

/// Utilities attached to `anyhow::Result<T>`.
pub trait ResultMetricsExt {
    fn emit_metrics(self, ctx: Option<OneshotMetricsContext>) -> anyhow::Result<()>;
}

impl<T> ResultJsonExt for anyhow::Result<T>
where
    T: serde::Serialize,
{
    fn output_json(self, json: bool) -> anyhow::Result<()> {
        if json && let Ok(value) = &self {
            json_pretty_to_stdout(value)?;
        }
        self.map(|_| ())
    }
}

fn json_pretty_to_stdout(value: &impl serde::Serialize) -> std::io::Result<()> {
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    let value = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    if value != "null" {
        stdout.write_all(value.as_bytes())?;
        stdout.write_all(b"\n").ok();
    }
    Ok(())
}
