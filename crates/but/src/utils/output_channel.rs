use std::io::Write;

use minus::ExitStrategy;

use crate::{args::OutputFormat, utils::json_pretty_to_stdout};

/// A utility `std::io::Write` implementation that can always be used to generate output for humans or for scripts.
pub struct OutputChannel {
    /// How to print the output, one should match on it. Match on this if you prefer this style.
    format: OutputFormat,
    /// The output to use if there is no pager.
    inner: std::io::Stdout,
    /// Possibly a pager we are using. If `Some`, our `inner` is the pager itself which we interact with from here.
    pager: Option<minus::Pager>,
}

/// Conversions
impl OutputChannel {
    /// Provide a write implementation for humans, if the format setting permits.
    pub fn for_human(&mut self) -> Option<&mut (dyn std::fmt::Write + 'static)> {
        matches!(self.format, OutputFormat::Human).then(|| self as &mut dyn std::fmt::Write)
    }
    /// Provide a write implementation for Shwll output, if the format setting permits.
    pub fn for_shell(&mut self) -> Option<&mut (dyn std::fmt::Write + 'static)> {
        matches!(self.format, OutputFormat::Shell).then(|| self as &mut dyn std::fmt::Write)
    }
    /// Provide a handle to receive a serde-serializable value to write to stdout.
    pub fn for_json(&mut self) -> Option<&mut Self> {
        matches!(self.format, OutputFormat::Json).then_some(self)
    }
}

/// JSON utilities
impl OutputChannel {
    /// Write `value` as pretty JSON to the output.
    ///
    /// Note that it's owned to avoid double-printing with [ResultJsonExt::output_json]
    pub fn write_value(&mut self, value: impl serde::Serialize) -> std::io::Result<()> {
        json_pretty_to_stdout(&value)
    }
}

impl std::fmt::Write for OutputChannel {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        match self.format {
            OutputFormat::Human | OutputFormat::Shell => {
                if let Some(out) = self.pager.as_mut() {
                    out.write_str(s)
                } else {
                    self.inner.write_all(s.as_bytes()).or_else(|err| {
                        if err.kind() == std::io::ErrorKind::BrokenPipe {
                            // Ignore broken pipes and keep writing.
                            // This allows the caller to use `?` without having to think
                            // about ignoring errors selectively.
                            Ok(())
                        } else {
                            Err(std::fmt::Error)
                        }
                    })
                }
            }
            OutputFormat::Json | OutputFormat::None => {
                // It's not an error to try to write in JSON mode, it's a feature.
                // However, the only way to write JSON is to use [Self::write_value()].
                Ok(())
            }
        }
    }
}

/// Lifecycle
impl OutputChannel {
    /// Create a new instance to output with `format` (advisory), which affects where it prints to.
    ///
    /// It's configured to print to stdout unless [`OutputFormat::Json`] is used, then it prints everything
    /// to a `/dev/null` equivalent, so callers never have to worry if they interleave JSON with other output.
    ///
    /// WARNING: the current implementation is static and would cache everything in memory.
    ///          Use `dynamic_output` (cargo feature + see https://docs.rs/minus/5.6.1/minus/#threads) otherwise.
    ///          It also needs to avoid
    pub fn new_with_pager(format: OutputFormat) -> Self {
        OutputChannel {
            format,
            inner: std::io::stdout(),
            pager: if !matches!(format, OutputFormat::Human)
                || std::env::var_os("NOPAGER").is_some()
            {
                None
            } else {
                let pager = minus::Pager::new();
                let msg = "can talk to newly created pager";
                pager.set_exit_strategy(ExitStrategy::PagerQuit).expect(msg);
                pager.set_prompt("GitButler").expect(msg);
                Some(pager)
            },
        }
    }

    /// Like [`Self::new_with_pager`], but will never create a pager or write JSON.
    /// Use this if a second instance of a channel is needed, and the first one could have a pager.
    pub fn new_without_pager_non_json(format: OutputFormat) -> Self {
        OutputChannel {
            format: match format {
                OutputFormat::Human | OutputFormat::Shell | OutputFormat::None => format,
                OutputFormat::Json => OutputFormat::None,
            },
            inner: std::io::stdout(),
            pager: None,
        }
    }
}

impl Drop for OutputChannel {
    fn drop(&mut self) {
        if let Some(pager) = self.pager.take() {
            minus::page_all(pager).ok();
        }
    }
}
