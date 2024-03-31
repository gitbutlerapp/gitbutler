use std::borrow::Cow;
use std::fmt::{Debug, Display};

/// A unique code that consumers of the API may rely on to identify errors.
#[derive(Debug, Default, Copy, Clone, PartialOrd, PartialEq)]
pub enum Code {
    #[default]
    Unknown,
    Validation,
    Projects,
    Branches,
    ProjectGitAuth,
    ProjectGitRemote,
    ProjectConflict,
    ProjectHead,
    Menu,
    PreCommitHook,
    CommitMsgHook,
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Code::Menu => "errors.menu",
            Code::Unknown => "errors.unknown",
            Code::Validation => "errors.validation",
            Code::Projects => "errors.projects",
            Code::Branches => "errors.branches",
            Code::ProjectGitAuth => "errors.projects.git.auth",
            Code::ProjectGitRemote => "errors.projects.git.remote",
            Code::ProjectHead => "errors.projects.head",
            Code::ProjectConflict => "errors.projects.conflict",
            //TODO: rename js side to be more precise what kind of hook error this is
            Code::PreCommitHook => "errors.hook",
            Code::CommitMsgHook => "errors.hooks.commit.msg",
        };
        f.write_str(code)
    }
}

/// A context to wrap around lower errors to allow its classification, along with a message for the user.
#[derive(Default, Debug, Clone)]
pub struct Context {
    /// The identifier of the error.
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
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Context {
            code,
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
}

/// A trait that if implemented on `thiserror` instance, allows to extract context we provide
/// in its variants.
///
/// Note that this is a workaround for the inability to control or implement the `provide()` method
/// on the `std::error::Error` implementation of `thiserror`.
pub trait ErrorWithContext: std::error::Error {
    /// Obtain the [`Context`], if present.
    fn context(&self) -> Option<Context>;
}

/// Convert `err` into an `anyhow` error, but also add provided `Code` or `Context` as anyhow context.
/// This uses the new `provide()` API to attach arbitrary information to error implementations.
pub fn into_anyhow(err: impl ErrorWithContext + Send + Sync + 'static) -> anyhow::Error {
    let context = err.context();
    let err = anyhow::Error::from(err);
    if let Some(context) = context {
        err.context(context)
    } else {
        err
    }
}

/// A wrapper around an `anyhow` error which automatically extracts the context that might be attached
/// to `thiserror` instances.
///
/// Whenever `thiserror` is involved, this error type should be used if the alternative would be to write
/// a `thiserror` which just forwards its context (like `app::Error` previously).
#[derive(Debug)]
pub struct Error2(anyhow::Error);

impl From<anyhow::Error> for Error2 {
    fn from(value: anyhow::Error) -> Self {
        Self(value)
    }
}

impl From<Error2> for anyhow::Error {
    fn from(value: Error2) -> Self {
        value.0
    }
}

impl<E> From<E> for Error2
where
    E: ErrorWithContext + Send + Sync + 'static,
{
    fn from(value: E) -> Self {
        Self(into_anyhow(value))
    }
}

impl Error2 {
    /// A manual, none-overlapping implementation of `From` (or else there are conflicts).
    pub fn from_err(err: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self(err.into())
    }

    /// Associated more context to the contained anyhow error
    pub fn context<C>(self, context: C) -> Self
    where
        C: Display + Send + Sync + 'static,
    {
        let err = self.0;
        Self(err.context(context))
    }

    /// Returns `true` if `E` is contained in our error chain.
    pub fn is<E>(&self) -> bool
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        self.0.is::<E>()
    }

    /// Downcast our instance to the given type `E`, or `None` if it's not contained in our context or error chain.
    pub fn downcast_ref<E>(&self) -> Option<&E>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        self.0.downcast_ref::<E>()
    }
}

pub use legacy::Error;

//#[deprecated(
//    note = "the types in the error::legacy::* module are deprecated; use error::gb::Error and error::gb::Result instead"
//)]
mod legacy {
    use serde::{ser::SerializeMap, Serialize};

    use crate::error::Code;
    use crate::{keys, projects, users};

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("[{code}]: {message}")]
        UserError { code: Code, message: String },
        #[error("[errors.unknown]: Something went wrong")]
        Unknown,
    }

    impl Serialize for Error {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let (code, message) = match self {
                Error::UserError { code, message } => (code.to_string(), message.to_string()),
                Error::Unknown => (
                    Code::Unknown.to_string(),
                    "Something went wrong".to_string(),
                ),
            };

            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("code", &code)?;
            map.serialize_entry("message", &message)?;
            map.end()
        }
    }

    impl From<anyhow::Error> for Error {
        fn from(error: anyhow::Error) -> Self {
            tracing::error!(?error);
            Error::Unknown
        }
    }

    impl From<keys::GetOrCreateError> for Error {
        fn from(error: keys::GetOrCreateError) -> Self {
            tracing::error!(?error);
            Error::Unknown
        }
    }

    impl From<users::GetError> for Error {
        fn from(error: users::GetError) -> Self {
            tracing::error!(?error);
            Error::Unknown
        }
    }

    impl From<projects::controller::GetError> for Error {
        fn from(error: projects::controller::GetError) -> Self {
            tracing::error!(?error);
            Error::Unknown
        }
    }
}
