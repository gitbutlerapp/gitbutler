use crate::{args::OutputFormat, utils::json_pretty_to_stdout};
use minus::ExitStrategy;
use std::io::{IsTerminal, Write};

/// A utility `std::io::Write` implementation that can always be used to generate output for humans or for scripts.
pub struct OutputChannel {
    /// How to print the output, one should match on it. Match on this if you prefer this style.
    format: OutputFormat,
    /// The output to use if there is no pager.
    inner: std::io::Stdout,
    /// Possibly a pager we are using. If `Some`, our `inner` is the pager itself which we interact with from here.
    pager: Option<minus::Pager>,
}

/// A channel that implements [`std::io::Write`], to make unbuffered writes to [`std::io::stderr`]
/// if the error channel is connected to a terminal, for providing progress or error information.
/// Broken pipes will also be ignored, thus the output written to this channel should be considered optional.
pub struct ProgressChannel {
    /// The channel writes will go to, if we are connected to a terminal.
    inner: Option<std::io::Stderr>,
}

impl Default for ProgressChannel {
    fn default() -> Self {
        ProgressChannel {
            inner: {
                let stderr = std::io::stderr();
                stderr.is_terminal().then_some(stderr)
            },
        }
    }
}

impl std::io::Write for ProgressChannel {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(stderr) = self.inner.as_mut() {
            stderr
                .write(buf)
                .or_else(|err| ignore_broken_pipe(err).map(|()| buf.len()))
        } else {
            // Pretend we wrote everything
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(stderr) = self.inner.as_mut() {
            stderr.flush().or_else(ignore_broken_pipe)
        } else {
            Ok(())
        }
    }
}

impl std::fmt::Write for ProgressChannel {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        use std::io::Write;
        self.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
    }
}

fn ignore_broken_pipe(err: std::io::Error) -> std::io::Result<()> {
    if err.kind() == std::io::ErrorKind::BrokenPipe {
        Ok(())
    } else {
        Err(err)
    }
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

    /// Get the output format setting.
    pub fn format(&self) -> OutputFormat {
        self.format
    }

    /// Before performing further output, obtain an input channel which always bypasses the pager when writing,
    /// while allowing prompting the user for input.
    /// If `None` is returned, terminal input isn't available and one should suggest to use command-line
    /// arguments to unambiguously specify an operation.
    pub fn prepare_for_terminal_input(&mut self) -> Option<InputOutputChannel<'_>> {
        use std::io::IsTerminal;
        let stdin = std::io::stdin();
        if !stdin.is_terminal() || !self.inner.is_terminal() {
            return None;
        }
        if self.for_human().is_none() {
            tracing::warn!(
                "Stdin is a terminal, and output wasn't configured for human consumption"
            );
            return None;
        }
        Some(InputOutputChannel { out: self, stdin })
    }

    /// A convenience function to create a progress channel, which doesn't have any relationship with this instance.
    pub fn progress_channel(&self) -> ProgressChannel {
        ProgressChannel::default()
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
        use std::io::Write;
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

/// A channel to obtain various kinds of user input from a terminal, bypassing the pager.
pub struct InputOutputChannel<'out> {
    out: &'out mut OutputChannel,
    stdin: std::io::Stdin,
}

impl std::fmt::Write for InputOutputChannel<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        use std::io::Write;
        // bypass the pager, fail on broken pipes (we are prompting)
        self.out
            .inner
            .write_all(s.as_bytes())
            .map_err(|_| std::fmt::Error)
    }
}

impl InputOutputChannel<'_> {
    /// Prompt a non-empty string from the user, or `None` if the input was empty.
    pub fn prompt(&mut self, prompt: impl AsRef<str>) -> anyhow::Result<Option<String>> {
        use std::fmt::Write;
        let prompt = prompt.as_ref();
        writeln!(self, "{}", prompt)?;
        write!(self, "> ")?;
        std::io::Write::flush(&mut self.out.inner)?;

        let mut input = String::new();
        self.stdin.read_line(&mut input)?;
        let input = input.trim().to_owned();
        if input.is_empty() {
            return Ok(None);
        }
        Ok(Some(input))
    }
}

/// Be sure to flush everything written after the prompt as well - the output channel may be buffered.
impl Drop for InputOutputChannel<'_> {
    fn drop(&mut self) {
        self.out.inner.flush().ok();
    }
}

/// Lifecycle
impl OutputChannel {
    /// Create a new instance to output with `format` (advisory), which affects where it prints to.
    /// The `use_pager` parameter controls whether a pager should be created.
    ///
    /// It's configured to print to stdout unless [`OutputFormat::Json`] is used, then it prints everything
    /// to a `/dev/null` equivalent, so callers never have to worry if they interleave JSON with other output.
    pub fn new_with_optional_pager(format: OutputFormat, use_pager: bool) -> Self {
        OutputChannel {
            format,
            inner: std::io::stdout(),
            pager: if !matches!(format, OutputFormat::Human)
                || std::env::var_os("NOPAGER").is_some()
                || !use_pager
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
