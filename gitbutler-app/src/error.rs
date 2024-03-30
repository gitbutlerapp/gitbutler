#[cfg(feature = "sentry")]
mod sentry;

pub(crate) use legacy::*;

pub(crate) mod gb {
    #[cfg(feature = "error-context")]
    pub use error_context::*;

    #[cfg(feature = "error-context")]
    mod error_context {
        use std::collections::BTreeMap;

        use backtrace::Backtrace;

        use super::{ErrorKind, Result, WithContext};

        #[derive(Debug)]
        pub struct Context {
            pub backtrace: Backtrace,
            pub caused_by: Option<Box<ErrorContext>>,
            pub vars: BTreeMap<String, String>,
        }

        impl Default for Context {
            fn default() -> Self {
                Self {
                    backtrace: Backtrace::new_unresolved(),
                    caused_by: None,
                    vars: BTreeMap::default(),
                }
            }
        }

        #[derive(Debug)]
        pub struct ErrorContext {
            error: ErrorKind,
            context: Context,
        }

        impl core::fmt::Display for ErrorContext {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.error.fmt(f)
            }
        }

        impl std::error::Error for ErrorContext {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                self.context
                    .caused_by
                    .as_ref()
                    .map(|e| e as &dyn std::error::Error)
            }

            fn provide<'a>(&'a self, request: &mut std::error::Request<'a>) {
                if request.would_be_satisfied_by_ref_of::<Backtrace>() {
                    request.provide_ref(&self.context.backtrace);
                }
            }
        }

        impl ErrorContext {
            #[inline]
            pub fn error(&self) -> &ErrorKind {
                &self.error
            }

            #[inline]
            pub fn context(&self) -> &Context {
                &self.context
            }

            pub(crate) fn into_owned(self) -> (ErrorKind, Context) {
                (self.error, self.context)
            }
        }

        impl<E: Into<ErrorContext>> WithContext<ErrorContext> for E {
            fn add_err_context<K: Into<String>, V: Into<String>>(
                self,
                name: K,
                value: V,
            ) -> ErrorContext {
                let mut e = self.into();
                e.context.vars.insert(name.into(), value.into());
                e
            }

            fn wrap_err<K: Into<ErrorKind>>(self, error: K) -> ErrorContext {
                let mut new_err = ErrorContext {
                    error: error.into(),
                    context: Context::default(),
                };

                new_err.context.caused_by = Some(Box::new(self.into()));
                new_err
            }
        }

        impl<T, E> WithContext<Result<T>> for std::result::Result<T, E>
        where
            E: Into<ErrorKind>,
        {
            #[inline]
            fn add_err_context<K: Into<String>, V: Into<String>>(
                self,
                name: K,
                value: V,
            ) -> Result<T> {
                self.map_err(|e| {
                    ErrorContext {
                        error: e.into(),
                        context: Context::default(),
                    }
                    .add_err_context(name, value)
                })
            }

            #[inline]
            fn wrap_err<K: Into<ErrorKind>>(self, error: K) -> Result<T> {
                self.map_err(|e| {
                    ErrorContext {
                        error: e.into(),
                        context: Context::default(),
                    }
                    .wrap_err(error)
                })
            }
        }

        #[cfg(feature = "error-context")]
        impl serde::Serialize for ErrorContext {
            fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                use serde::ser::SerializeSeq;
                let mut seq = serializer.serialize_seq(None)?;
                let mut current = Some(self);
                while let Some(err) = current {
                    seq.serialize_element(&err.error)?;
                    current = err.context.caused_by.as_deref();
                }
                seq.end()
            }
        }

        impl From<ErrorKind> for ErrorContext {
            fn from(error: ErrorKind) -> Self {
                Self {
                    error,
                    context: Context::default(),
                }
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn error_context() {
                fn low_level_io() -> std::result::Result<(), std::io::Error> {
                    Err(std::io::Error::new(std::io::ErrorKind::Other, "oh no!"))
                }

                fn app_level_io() -> Result<()> {
                    low_level_io().add_err_context("foo", "bar")?;
                    unreachable!();
                }

                use std::error::Error;

                let r = app_level_io();
                assert!(r.is_err());
                let e = r.unwrap_err();
                assert_eq!(e.context().vars.get("foo"), Some(&"bar".to_string()));
                assert!(e.source().is_none());
                assert!(e.to_string().starts_with("io.other-error:"));
            }
        }
    }

    pub trait WithContext<R> {
        fn add_err_context<K: Into<String>, V: Into<String>>(self, name: K, value: V) -> R;
        fn wrap_err<E: Into<ErrorKind>>(self, error: E) -> R;
    }

    #[cfg(not(feature = "error-context"))]
    pub struct Context;

    pub trait ErrorCode {
        fn code(&self) -> String;
        fn message(&self) -> String;
    }

    #[derive(Debug, thiserror::Error)]
    pub enum ErrorKind {
        Io(#[from] ::std::io::Error),
        Git(#[from] ::git2::Error),
        CommonDirNotAvailable(String),
    }

    impl ErrorCode for std::io::Error {
        fn code(&self) -> String {
            slug::slugify(self.kind().to_string())
        }

        fn message(&self) -> String {
            self.to_string()
        }
    }

    impl ErrorCode for git2::Error {
        fn code(&self) -> String {
            slug::slugify(format!("{:?}", self.class()))
        }

        fn message(&self) -> String {
            self.to_string()
        }
    }

    impl ErrorCode for ErrorKind {
        fn code(&self) -> String {
            match self {
                ErrorKind::Io(e) => format!("io.{}", <std::io::Error as ErrorCode>::code(e)),
                ErrorKind::Git(e) => format!("git.{}", <git2::Error as ErrorCode>::code(e)),
                ErrorKind::CommonDirNotAvailable(_) => "no-common-dir".to_string(),
            }
        }

        fn message(&self) -> String {
            match self {
                ErrorKind::Io(e) => <std::io::Error as ErrorCode>::message(e),
                ErrorKind::Git(e) => <git2::Error as ErrorCode>::message(e),
                ErrorKind::CommonDirNotAvailable(s) => format!("{s} is not available"),
            }
        }
    }

    impl core::fmt::Display for ErrorKind {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            format!(
                "{}: {}",
                <Self as ErrorCode>::code(self),
                <Self as ErrorCode>::message(self)
            )
            .fmt(f)
        }
    }

    #[cfg(not(feature = "error-context"))]
    pub type Error = ErrorKind;
    #[cfg(feature = "error-context")]
    pub type Error = ErrorContext;

    pub type Result<T> = ::std::result::Result<T, Error>;

    #[cfg(not(feature = "error-context"))]
    impl ErrorKind {
        #[inline]
        pub fn error(&self) -> &Error {
            self
        }

        #[inline]
        pub fn context(&self) -> Option<&Context> {
            None
        }
    }

    #[cfg(not(feature = "error-context"))]
    impl WithContext<ErrorKind> for ErrorKind {
        #[inline]
        fn add_err_context<K: Into<String>, V: Into<String>>(self, _name: K, _value: V) -> Error {
            self
        }

        #[inline]
        fn wrap_err(self, _error: Error) -> Error {
            self
        }
    }

    #[cfg(not(feature = "error-context"))]
    impl<T, E> WithContext<std::result::Result<T, E>> for std::result::Result<T, E> {
        #[inline]
        fn add_err_context<K: Into<String>, V: Into<String>>(
            self,
            _name: K,
            _value: V,
        ) -> std::result::Result<T, E> {
            self
        }

        #[inline]
        fn wrap_err(self, _error: Error) -> std::result::Result<T, E> {
            self
        }
    }

    #[cfg(feature = "error-context")]
    impl serde::Serialize for ErrorKind {
        fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            use serde::ser::SerializeTuple;
            let mut seq = serializer.serialize_tuple(2)?;
            seq.serialize_element(&self.code())?;
            seq.serialize_element(&self.message())?;
            seq.end()
        }
    }
}

//#[deprecated(
//    note = "the types in the error::legacy::* module are deprecated; use error::gb::Error and error::gb::Result instead"
//)]
mod legacy {
    use core::fmt;

    use gitbutler_core::project_repository;
    use serde::{ser::SerializeMap, Serialize};

    #[derive(Debug)]
    pub enum Code {
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

    impl fmt::Display for Code {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Code::Menu => write!(f, "errors.menu"),
                Code::Unknown => write!(f, "errors.unknown"),
                Code::Validation => write!(f, "errors.validation"),
                Code::Projects => write!(f, "errors.projects"),
                Code::Branches => write!(f, "errors.branches"),
                Code::ProjectGitAuth => write!(f, "errors.projects.git.auth"),
                Code::ProjectGitRemote => write!(f, "errors.projects.git.remote"),
                Code::ProjectHead => write!(f, "errors.projects.head"),
                Code::ProjectConflict => write!(f, "errors.projects.conflict"),
                //TODO: rename js side to be more precise what kind of hook error this is
                Code::PreCommitHook => write!(f, "errors.hook"),
                Code::CommitMsgHook => write!(f, "errors.hooks.commit.msg"),
            }
        }
    }

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

    impl From<project_repository::OpenError> for Error {
        fn from(value: project_repository::OpenError) -> Self {
            match value {
                project_repository::OpenError::NotFound(path) => Error::UserError {
                    code: Code::Projects,
                    message: format!("{} not found", path.display()),
                },
                project_repository::OpenError::Other(error) => {
                    tracing::error!(?error);
                    Error::Unknown
                }
            }
        }
    }
}
