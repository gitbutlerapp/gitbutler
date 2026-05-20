//! Utilities for communicating user errors (i.e. the equivalent of 4xx HTTP responses) to the user.

use std::fmt::Display;

use crate::theme::{self, Paint};

/// Signifies that a command could not complete its intended action due to the user providing input
/// that is somehow invalid.
///
/// Think of this as the `but` equivalent of a 4xx HTTP response.
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
}

impl Display for BadInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(arg) = &self.arg {
            writeln!(
                f,
                "{} bad input for '{}'",
                theme::get().error.paint("error:"),
                theme::get().attention.paint(arg),
            )?;
            writeln!(f)?;
            write!(f, "{}", self.message)
        } else {
            write!(f, "{} {}", theme::get().error.paint("error:"), self.message)
        }
    }
}
