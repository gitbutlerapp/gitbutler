use std::io::Write;

mod output_channel;
#[cfg(all(feature = "legacy", feature = "but-2"))]
pub use output_channel::experimental::*;
pub use output_channel::{
    Confirm, ConfirmDefault, ConfirmOrEmpty, InputOutputChannel, OutputChannel, WriteWithUtils,
};

mod object_id;
pub use object_id::{shorten_hex_object_id, shorten_object_id, split_short_id};

mod pager;

mod debug_as_type;
pub(crate) use debug_as_type::DebugAsType;

pub mod metrics;
#[cfg(feature = "legacy")]
pub use metrics::types::BackgroundMetrics;
pub use metrics::types::OneshotMetricsContext;

pub mod detect_agent;
pub mod time;

pub(crate) mod binary_path;

pub trait ResultErrorExt {
    fn show_root_cause_error_then_exit_without_destructors(self, out: OutputChannel) -> !;
}

pub mod envs;

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

/// Metrics utilities for results
pub trait ResultMetricsExt<T, E> {
    /// Emit metrics for the [`Result`].
    ///
    /// The result must simply be propagated through this method, regardless of if emitting metrics
    /// is successful or not. We do not want a failure to emit metrics to impact the user
    /// experience.
    fn emit_metrics(self, ctx: Option<OneshotMetricsContext>) -> Result<T, E>;
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
