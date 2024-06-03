//! ## How-To
//!
//! This is a primer on how to use the types provided here.
//!
//! **tl;dr** - use `anyhow::Result` by direct import so a typical function looks like this:
//!
//! ```rust
//!# use anyhow::{Result, bail};
//! fn f() -> Result<()> {
//!    bail!("this went wrong")
//! }
//! ```
//!
//! ### Providing Context
//!
//! To inform about what you were trying to do when it went wrong, assign some [`context`](anyhow::Context::context)
//! directly to [results](Result), to [`options`](Option) or to `anyhow` errors.
//!
//! ```rust
//!# use anyhow::{anyhow, Result, bail, Context};
//! fn maybe() -> Option<()> {
//!    None
//! }
//!
//! fn a() -> Result<()> {
//!    maybe().context("didn't get it at this time")
//! }
//!
//! fn b() -> Result<()> {
//!    a().context("an operation couldn't be performed")
//! }
//!
//! fn c() -> Result<()> {
//!     b().map_err(|err| err.context("sometimes useful"))
//! }
//!
//! fn main() {
//!    assert_eq!(format!("{:#}", c().unwrap_err()),
//!               "sometimes useful: an operation couldn't be performed: didn't get it at this time");
//! }
//! ```
//!
//! ### Frontend Interactions
//!
//! We don't know anything about frontends here, but we also have to know something to be able to control
//! which error messages show up. Sometimes the frontend needs to decide what to do based on a particular
//! error that happened, hence it has to classify errors and it shouldn't do that by matching strings.
//!
//! #### Meet the `Code`
//!
//! The [`Code`] is a classifier for errors, and it can be attached as [`anyhow context`](anyhow::Context)
//! to be visible to `tauri`, which looks at the error chain to obtain such metadata.
//!
//! By default, the frontend will show the stringified root error if a `tauri` command fails.
//! However, **sometimes we want to cut that short and display a particular message**.
//!
//! ```rust
//!# use anyhow::{Result, Context};
//!# use gitbutler_core::error::Code;
//!
//! fn do_io() -> std::io::Result<()> {
//!     Err(std::io::Error::new(std::io::ErrorKind::Other, "this didn't work"))
//! }
//!
//! fn a() -> Result<()> {
//!     do_io()
//!         .context("whatever comes before a `Code` context shows in frontend, so THIS")
//!         .context(Code::Unknown)
//! }
//!
//! fn main() {
//!    assert_eq!(format!("{:#}", a().unwrap_err()),
//!              "errors.unknown: whatever comes before a `Code` context shows in frontend, so THIS: this didn't work",
//!              "however, that Code also shows up in the error chain in logs - context is just like an Error for anyhow");
//! }
//! ```
//!
//! #### Tuning error chains
//!
//! The style above was most convenient and can be used without hesitation, but if for some reason it's important
//! for `Code` not to show up in the error chain, one can use the [`error::Context`](Context) directly.
//!
//! ```rust
//!# use anyhow::{Result, Context};
//!# use gitbutler_core::error;
//!
//! fn do_io() -> std::io::Result<()> {
//!     Err(std::io::Error::new(std::io::ErrorKind::Other, "this didn't work"))
//! }
//!
//! fn a() -> Result<()> {
//!     do_io().context(error::Context::new("This message is shown and only this meessage")
//!                         .with_code(error::Code::Validation))
//! }
//!
//! fn main() {
//!    assert_eq!(format!("{:#}", a().unwrap_err()),
//!              "This message is shown and only this meessage: this didn't work",
//!              "now the added context just looks like an error, even though it also contains a `Code` which can be queried");
//! }
//! ```
//!
//! ### Backtraces and `anyhow`
//!
//! Backtraces are automatically collected when `anyhow` errors are instantiated, as long as the
//! `RUST_BACKTRACE` variable is set.
//!
//! #### With `thiserror`
//!
//! `thiserror` doesn't have a mechanism for generic context, and if it's needed the error can be converted
//! to `anyhow::Error`.
//!
//! By default, `thiserror` instances have no context.
use std::borrow::Cow;
use std::fmt::Debug;

/// A unique code that consumers of the API may rely on to identify errors.
///
/// ### Important
///
/// **Only add variants if a consumer, like the *frontend*, is actually using them**.
/// Remove variants when no longer in use.
///
/// In practice, it should match its [frontend counterpart](https://github.com/gitbutlerapp/gitbutler/blob/fa973fd8f1ae8807621f47601803d98b8a9cf348/app/src/lib/backend/ipc.ts#L5).
#[derive(Debug, Default, Copy, Clone, PartialOrd, PartialEq)]
pub enum Code {
    /// Much like a catch-all error code. It shouldn't be attached explicitly unless
    /// a message is provided as well as part of a [`Context`].
    #[default]
    Unknown,
    Validation,
    ProjectGitAuth,
    DefaultTargetNotFound,
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Code::Unknown => "errors.unknown",
            Code::Validation => "errors.validation",
            Code::ProjectGitAuth => "errors.projects.git.auth",
            Code::DefaultTargetNotFound => "errors.projects.default_target.not_found",
        };
        f.write_str(code)
    }
}

/// A context for classifying errors.
///
/// It provides a [`Code`], which may be [unknown](Code::Unknown), and a `message` which explains
/// more about the problem at hand.
#[derive(Default, Debug, Clone)]
pub struct Context {
    /// The classification of the error.
    pub code: Code,
    /// A description of what went wrong, if available.
    pub message: Option<Cow<'static, str>>,
}

impl std::fmt::Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message.as_deref().unwrap_or("Something went wrong"))
    }
}

impl From<Code> for Context {
    fn from(code: Code) -> Self {
        Context {
            code,
            message: None,
        }
    }
}

impl Context {
    /// Create a new instance with `code` and an owned `message`.
    pub fn new(message: impl Into<String>) -> Self {
        Context {
            code: Code::Unknown,
            message: Some(Cow::Owned(message.into())),
        }
    }

    /// Create a new instance with `code` and a statically known `message`.
    pub const fn new_static(code: Code, message: &'static str) -> Self {
        Context {
            code,
            message: Some(Cow::Borrowed(message)),
        }
    }

    /// Adjust the `code` of this instance to the given one.
    pub fn with_code(mut self, code: Code) -> Self {
        self.code = code;
        self
    }
}

mod private {
    pub trait Sealed {}
}

/// A way to obtain attached Code or context information from `anyhow` contexts, so that
/// the more complete information is preferred.
pub trait AnyhowContextExt: private::Sealed {
    /// Return our custom context that might be attached to this instance.
    ///
    /// Note that it could not be named `context()` as this method already exists.
    fn custom_context(&self) -> Option<Context>;

    /// Return our custom context or default it to the root-cause of the error.
    fn custom_context_or_root_cause(&self) -> Context;
}

impl private::Sealed for anyhow::Error {}
impl AnyhowContextExt for anyhow::Error {
    fn custom_context(&self) -> Option<Context> {
        if let Some(ctx) = self.downcast_ref::<Context>() {
            Some(ctx.clone())
        } else {
            self.downcast_ref::<Code>().map(|code| (*code).into())
        }
    }

    fn custom_context_or_root_cause(&self) -> Context {
        self.custom_context().unwrap_or_else(|| Context {
            code: Code::Unknown,
            message: Some(self.root_cause().to_string().into()),
        })
    }
}
