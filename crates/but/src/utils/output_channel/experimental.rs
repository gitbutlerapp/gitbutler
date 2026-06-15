//! Experimental output used by new (but-2) CLI commands.
//!
//! Still very much in flux and will change as we implement and dog food the new commands.

use crate::{
    args::OutputFormat,
    theme::Theme,
    utils::{InputOutputChannel, OutputChannel, WriteWithUtils},
};

pub struct IntermediateChannel<'out> {
    out: &'out mut OutputChannel,
}

impl std::fmt::Write for IntermediateChannel<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.out.progress_channel().write_str(s)
    }
}

impl<'out> WriteWithUtils for IntermediateChannel<'out> {
    fn truncate_if_unpaged(&self, text: &str, max_width: usize) -> String {
        self.out.truncate_if_unpaged(text, max_width)
    }

    fn is_paged(&self) -> bool {
        self.out.is_paged()
    }
}

impl<'out> IntermediateChannel<'out> {
    pub fn new(out: &'out mut OutputChannel) -> Self {
        Self { out }
    }

    pub fn prepare_for_terminal_input(&mut self) -> Option<InputOutputChannel<'_>> {
        self.out.prepare_for_terminal_input()
    }
}

pub trait CliOutputHuman {
    fn on_human(self, out: &mut dyn WriteWithUtils, theme: &Theme) -> anyhow::Result<()>;
}

#[allow(dead_code)]
pub trait CliOutput: CliOutputHuman {
    fn on_shell(self, out: &mut dyn WriteWithUtils) -> anyhow::Result<()>;

    fn on_json(self) -> impl serde::Serialize;
}

pub trait OutputChannelExt {
    #[allow(dead_code)]
    fn print_cli_output(&mut self, output: impl CliOutput) -> anyhow::Result<()>;

    #[allow(dead_code)]
    fn print_cli_output_human(&mut self, output: impl CliOutputHuman) -> anyhow::Result<()>;
}

impl OutputChannelExt for OutputChannel {
    fn print_cli_output(&mut self, output: impl CliOutput) -> anyhow::Result<()> {
        match self.format {
            OutputFormat::Human | OutputFormat::Agent => output.on_human(self, crate::theme::get()),
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
