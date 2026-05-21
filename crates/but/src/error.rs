//! Utilities for communicating user errors (i.e. the equivalent of 4xx HTTP responses) to the user.

use std::fmt::Display;

use crate::theme::{self, Paint};

/// Signifies that a command could not complete its intended action due to the user providing input
/// that is somehow invalid.
///
/// Think of this as the `but` equivalent of a 4xx HTTP response.
#[derive(Debug)]
pub struct BadInput {
    /// A message to print verbatim to the user
    pub message: String,
    /// If applicable, the input argument to which the bad input was passed
    pub arg: Option<String>,
}

impl BadInput {
    /// Create a new [`BadInput`]
    pub fn new<S: AsRef<str>>(message: S) -> Self {
        Self {
            message: message.as_ref().to_string(),
            arg: None,
        }
    }

    /// Add the argument for which this message applies.
    pub fn arg<S: AsRef<str>>(mut self, arg: S) -> Self {
        self.arg = Some(arg.as_ref().to_string());
        self
    }

    /// Wrap this value as a [`CliError::BadInput`] in a [`CliResult`].
    pub fn into_cli_result<T>(self) -> CliResult<T> {
        Err(self.into())
    }
}

impl Display for BadInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(arg) = &self.arg {
            writeln!(
                f,
                "{} Bad input for '{}'",
                theme::get().error.paint("Error:"),
                theme::get().attention.paint(arg),
            )?;
            writeln!(f)?;
            write!(f, "{}", self.message)
        } else {
            write!(f, "{} {}", theme::get().error.paint("Error:"), self.message)
        }
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
            Self::Internal(value) => Self::Internal(value.context(context)),
        }
    }
}

impl Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadInput(value) => value.fmt(f),
            Self::Internal(value) => value.fmt(f),
        }
    }
}

#[derive(Debug)]
pub enum CliError {
    /// User provided bad input.
    BadInput(BadInput),
    /// Something went wrong internally.
    Internal(anyhow::Error),
}

pub type CliResult<T> = Result<T, CliError>;
