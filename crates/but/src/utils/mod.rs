use std::io::Write;

mod output_channel;
use but_api::json::{ChangeIdString, HexHash};
pub(crate) use output_channel::PromptLine;
pub use output_channel::{
    CliOutput, CliOutputHuman, Confirm, ConfirmDefault, ConfirmOrEmpty, InputOutputChannel,
    IntermediateChannel, OutputChannel, WriteWithUtils,
};

mod object_id;
pub use object_id::{get_change_id_for_commit, shorten_hex_object_id, shorten_object_id};

mod pager;

mod debug_as_type;
pub(crate) use debug_as_type::DebugAsType;

pub mod metrics;
pub use metrics::types::OneshotMetricsContext;

use crate::id::CommitId;

pub mod detect_agent;
pub mod time;

pub(crate) mod binary_path;
pub(crate) mod diff_specs;
#[cfg(feature = "legacy")]
pub(crate) mod merged_upstream;
#[cfg(feature = "legacy")]
pub(crate) mod rejection;
pub(crate) mod targeting;

pub mod diff_rendering;
pub mod string_interning;

pub trait ResultErrorExt {
    fn show_root_cause_error_then_exit_without_destructors(self, out: OutputChannel) -> !;
}

pub mod envs;

impl ResultErrorExt for anyhow::Result<()> {
    fn show_root_cause_error_then_exit_without_destructors(self, out: OutputChannel) -> ! {
        let full_error_chain = out.full_error_chain();
        // Trigger the pager to be flushed before exiting early, or destructors aren't called.
        drop(out);
        let code = if let Err(e) = &self {
            if full_error_chain {
                writeln!(std::io::stderr(), "{e:#}").ok();
            } else {
                writeln!(std::io::stderr(), "{} {}", e, e.root_cause()).ok();
            }
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
    json_pretty_to(value, &mut stdout)
}

fn json_pretty_to(
    value: &impl serde::Serialize,
    out: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    let value = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    if value != "null" {
        out.write_all(value.as_bytes())?;
        out.write_all(b"\n").ok();
    }
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitIdJson {
    pub commit_id: HexHash,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_id: Option<ChangeIdString>,
}

impl From<CommitId> for CommitIdJson {
    fn from(value: CommitId) -> Self {
        Self {
            commit_id: HexHash(value.commit_id),
            change_id: value.change_id.map(Into::into),
        }
    }
}
