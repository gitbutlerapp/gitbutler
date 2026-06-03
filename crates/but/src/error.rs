//! Utilities for communicating user errors (i.e. the equivalent of 4xx HTTP responses) to the user.

use std::ffi::OsString;
use std::fmt::Display;

use crate::theme::{self, Paint};

/// Signifies that a command could not complete its intended action due to the user providing input
/// that is somehow invalid.
///
/// Think of this as the `but` equivalent of a 4xx HTTP response.
#[derive(Debug)]
pub struct BadInput {
    /// A message to print verbatim to the user
    message: String,
    /// If applicable, the name of the input argument to which the bad input was passed
    arg_name: Option<String>,
    /// If applicable, the bad value that was passed
    arg_value: Option<String>,
    /// A hint to guide the user to proper usage of the command
    hint: Option<String>,
}

impl BadInput {
    /// Create a new [`BadInput`]
    pub fn new<S: AsRef<str>>(message: S) -> Self {
        Self {
            message: message.as_ref().to_string(),
            arg_name: None,
            arg_value: None,
            hint: None,
        }
    }

    /// Add the name of the argument for which this message applies.
    pub fn arg_name<S: AsRef<str>>(mut self, name: S) -> Self {
        self.arg_name = Some(name.as_ref().to_string());
        self
    }

    /// Add the value that was passed by the user.
    pub fn arg_value<S: AsRef<str>>(mut self, value: S) -> Self {
        self.arg_value = Some(value.as_ref().to_string());
        self
    }

    /// Add a hint to guide the user to correct usage.
    pub fn hint<S: AsRef<str>>(mut self, hint: S) -> Self {
        self.hint = Some(hint.as_ref().to_string());
        self
    }

    pub(crate) fn argument_name(&self) -> Option<&str> {
        self.arg_name.as_deref()
    }

    pub(crate) fn has_hint(&self) -> bool {
        self.hint.is_some()
    }
}

/// Convenience wrapper around [`BadInput::new`].
pub(crate) fn bad_input<S: AsRef<str>>(message: S) -> BadInput {
    BadInput::new(message)
}

impl Display for BadInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let t = theme::get();

        write!(f, "{}", t.error.paint("Error: "))?;

        match (&self.arg_name, &self.arg_value) {
            (Some(name), Some(value)) => {
                writeln!(
                    f,
                    "Bad input '{}' for '{}'",
                    t.attention.paint(value),
                    t.attention.paint(name),
                )?;
                writeln!(f)?;
            }
            (Some(name), None) => {
                writeln!(f, "Bad input for '{}'", t.attention.paint(name),)?;
                writeln!(f)?;
            }
            (None, Some(value)) => {
                writeln!(f, "Bad input '{}'", t.attention.paint(value))?;
                writeln!(f)?;
            }
            _ => (),
        }

        writeln!(f, "{}", self.message)?;

        if let Some(hint) = &self.hint {
            writeln!(f)?;
            writeln!(f, "{}", t.hint.paint(format!("Hint: {hint}")))?;
        }

        Ok(())
    }
}

impl From<BadInput> for CliError {
    fn from(value: BadInput) -> Self {
        Self::BadInput(value)
    }
}

impl<E> From<E> for CliError
where
    E: Into<anyhow::Error>,
{
    fn from(value: E) -> Self {
        Self::Internal(value.into())
    }
}

impl CliError {
    /// Add context to internal errors while preserving user-facing bad input messages.
    pub fn context<C>(self, context: C) -> Self
    where
        C: Display + Send + Sync + 'static,
    {
        match self {
            Self::BadInput(value) => Self::BadInput(value),
            Self::ExternalCommandNotFound(command_name) => {
                Self::ExternalCommandNotFound(command_name)
            }
            Self::Internal(value) => Self::Internal(value.context(context)),
        }
    }
}

impl Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadInput(value) => value.fmt(f),
            Self::ExternalCommandNotFound(command_name) => {
                writeln!(
                    f,
                    "{}",
                    bad_input("Unrecognized subcommand").arg_value(command_name.to_string_lossy())
                )
            }
            Self::Internal(value) => value.fmt(f),
        }
    }
}

#[derive(Debug)]
pub enum CliError {
    /// User provided bad input.
    BadInput(BadInput),
    /// We tried to execute the subcommand as an external command, but that command was not found.
    ExternalCommandNotFound(OsString),
    /// Something went wrong internally.
    Internal(anyhow::Error),
}

pub type CliResult<T> = Result<T, CliError>;
