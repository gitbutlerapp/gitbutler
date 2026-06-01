//! Experimental output used by new (but-2) CLI commands.
//!
//! Still very much in flux and will change as we implement and dog food the new commands.

use crate::{
    args::OutputFormat,
    theme::Theme,
    utils::{OutputChannel, WriteWithUtils},
};

pub struct IntermediateChannel<'out> {
    out: &'out mut OutputChannel,
}

impl std::fmt::Write for IntermediateChannel<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.out.progress_channel().write_str(s)
    }
}

impl<'out> IntermediateChannel<'out> {
    pub fn new(out: &'out mut OutputChannel) -> Self {
        Self { out }
    }

    // TODO: some functions for prompting the user
    // pub fn can_prompt(&self) -> bool {
    //     self.out.can_prompt()
    // }

    // pub fn prompt(&self) -> anyhow::Result<()> {
    //     if !self.can_prompt() {
    //         anyhow::bail!("BUG: attempted to prompt when prompting is not allowed")
    //     } else {
    //         Ok(())
    //     }
    // }
}

pub trait CliOutputHuman {
    fn on_human(self, out: &mut dyn WriteWithUtils, theme: &Theme) -> anyhow::Result<()>;
}

pub trait CliOutput: CliOutputHuman {
    fn on_shell(self, out: &mut dyn WriteWithUtils) -> anyhow::Result<()>;

    fn on_json(self) -> impl serde::Serialize;
}

pub trait OutputChannelExt {
    fn print_cli_output(&mut self, output: impl CliOutput) -> anyhow::Result<()>;

    #[expect(dead_code)]
    fn print_cli_output_human(&mut self, output: impl CliOutputHuman) -> anyhow::Result<()>;
}

impl OutputChannelExt for OutputChannel {
    fn print_cli_output(&mut self, output: impl CliOutput) -> anyhow::Result<()> {
        match self.format {
            OutputFormat::Human => output.on_human(self, crate::theme::get()),
            OutputFormat::Shell => output.on_shell(self),
            OutputFormat::Json => {
                let value = output.on_json();
                Ok(self.write_value(value)?)
            }
            OutputFormat::None => Ok(()),
        }
    }

    fn print_cli_output_human(&mut self, output: impl CliOutputHuman) -> anyhow::Result<()> {
        if let Some(for_human) = self.for_human() {
            output.on_human(for_human, crate::theme::get())
        } else {
            anyhow::bail!(
                "BUG: attempted to write human output when requested format is {:?}",
                self.format
            )
        }
    }
}
