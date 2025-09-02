//! Utilities to control which errors show in the frontend.
//!
//! ## How to use this
//!
//! Just make sure this `Error` type is used for each provided `tauri` command. The rest happens automatically
//! such that [context](gitbutler_error::error::Context) is handled correctly.
//!
//! ### Interfacing with `tauri` using `Error`
//!
//! `tauri` serializes backend errors and makes these available as JSON objects to the frontend. The format
//! is an implementation detail, but here it's implemented to turn each `Error` into a dict with `code`
//! and `messsage` fields.
//!
//! The values in these fields are controlled by attaching context, please [see the `error` docs](gitbutler_error::error))
//! on how to do this.
pub use frontend::{Error, ToError, UnmarkedError};

mod frontend {
    use std::borrow::Cow;

    use gitbutler_error::error::AnyhowContextExt;
    use serde::{Serialize, ser::SerializeMap};

    /// An error type for serialization which isn't expected to carry a code.
    #[derive(Debug)]
    pub struct UnmarkedError(anyhow::Error);

    impl<T> From<T> for UnmarkedError
    where
        T: std::error::Error + Send + Sync + 'static,
    {
        fn from(err: T) -> Self {
            Self(err.into())
        }
    }

    impl Serialize for UnmarkedError {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let ctx = self.0.custom_context_or_root_cause();

            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("code", &ctx.code.to_string())?;
            let message = ctx.message.unwrap_or_else(|| {
                self.0
                    .source()
                    .map(|err| Cow::Owned(err.to_string()))
                    .unwrap_or_else(|| Cow::Borrowed("Something went wrong"))
            });
            map.serialize_entry("message", &message)?;
            map.end()
        }
    }

    /// An error type for serialization, dynamically extracting context information during serialization,
    /// meant for consumption by the frontend.
    #[derive(Debug)]
    pub struct Error(anyhow::Error);

    impl From<anyhow::Error> for Error {
        fn from(value: anyhow::Error) -> Self {
            Self(value)
        }
    }

    pub trait ToError<T> {
        fn to_error(self) -> Result<T, Error>;
    }

    impl<T, E: std::error::Error + Send + Sync + 'static> ToError<T> for Result<T, E> {
        fn to_error(self) -> Result<T, Error> {
            self.map_err(|e| Error(e.into()))
        }
    }

    impl Serialize for Error {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let ctx = self.0.custom_context_or_root_cause();

            let mut map = serializer.serialize_map(Some(2))?;
            map.serialize_entry("code", &ctx.code.to_string())?;
            let message = ctx.message.unwrap_or_else(|| {
                self.0
                    .source()
                    .map(|err| Cow::Owned(err.to_string()))
                    .unwrap_or_else(|| Cow::Borrowed("An unknown backend error occurred"))
            });
            map.serialize_entry("message", &message)?;
            map.end()
        }
    }

    #[cfg(test)]
    mod tests {
        use anyhow::anyhow;
        use gitbutler_error::error::{Code, Context};

        use super::*;

        fn json(err: anyhow::Error) -> String {
            serde_json::to_string(&Error(err)).unwrap()
        }

        #[test]
        fn no_context_or_code_shows_root_error() {
            let err = anyhow!("err msg");
            assert_eq!(
                format!("{err:#}"),
                "err msg",
                "just one error on display here"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"errors.unknown\",\"message\":\"err msg\"}",
                "if there is no explicit error code or context, the original error message is shown"
            );
        }

        #[test]
        fn find_code() {
            let err = anyhow!("err msg").context(Code::Validation);
            assert_eq!(
                format!("{err:#}"),
                "errors.validation: err msg",
                "note how the context becomes an error, in front of the original one"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"errors.validation\",\"message\":\"err msg\"}",
                "the 'code' is available as string, but the message is taken from the source error"
            );
        }

        #[test]
        fn find_code_after_cause() {
            let original_err = std::io::Error::other("actual cause");
            let err = anyhow::Error::from(original_err)
                .context("err msg")
                .context(Code::Validation);

            assert_eq!(
                format!("{err:#}"),
                "errors.validation: err msg: actual cause",
                "an even longer chain, with the cause as root as one might expect"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"errors.validation\",\"message\":\"err msg\"}",
                "in order to attach a custom message to an original cause, our messaging (and Code) is the tail"
            );
        }

        #[test]
        fn find_context() {
            let err = anyhow!("err msg").context(Context::new_static(Code::Validation, "ctx msg"));
            assert_eq!(format!("{err:#}"), "ctx msg: err msg");
            assert_eq!(
                json(err),
                "{\"code\":\"errors.validation\",\"message\":\"ctx msg\"}",
                "Contexts often provide their own message, so the error message is ignored"
            );
        }

        #[test]
        fn find_context_without_message() {
            let err = anyhow!("err msg").context(Context::from(Code::Validation));
            assert_eq!(
                format!("{err:#}"),
                "Something went wrong: err msg",
                "on display, `Context` does just insert a generic message"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"errors.validation\",\"message\":\"err msg\"}",
                "Contexts without a message show the error's message as well"
            );
        }

        #[test]
        fn find_nested_code() {
            let err = anyhow!("bottom msg")
                .context("top msg")
                .context(Code::Validation);
            assert_eq!(
                format!("{err:#}"),
                "errors.validation: top msg: bottom msg",
                "now it's clear why bottom is bottom"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"errors.validation\",\"message\":\"top msg\"}",
                "the 'code' gets the message of the error that it provides context to, and it finds it down the chain"
            );
        }

        #[test]
        fn multiple_codes() {
            let err = anyhow!("bottom msg")
                .context(Code::ProjectGitAuth)
                .context("top msg")
                .context(Code::Validation);
            assert_eq!(
                format!("{err:#}"),
                "errors.validation: top msg: errors.projects.git.auth: bottom msg",
                "each code is treated like its own error in the chain"
            );
            assert_eq!(
                json(err),
                "{\"code\":\"errors.validation\",\"message\":\"top msg\"}",
                "it finds the most recent 'code' (and the same would be true for contexts, of course)"
            );
        }
    }
}
