//! ## How-To
//!
//! This is a primer on how to use the [`Error`] provided here.
//!
//! ### Interfacing with `tauri` using [`Error`]
//!
//! `tauri` serializes backend errors and makes these available as JSON objects to the frontend. The format
//! is an implementation detail, but here it's implemented to turn each [`Error`] into a dict with `code`
//! and `messsage` fields.
//!
//! The values in these fields are controlled by attaching context, please [see the `core` docs](gitbutler_core::error))
//! on how to do this.
//!
//! To assure context is picked up correctly to be made available to the UI if present, use
//! [`Error`] in the `tauri` commands. Due to technical limitations, it will only auto-convert
//! from `anyhow::Error`, or [core::Error](gitbutler_core::error::Error).
//! Errors that are known to have context can be converted using [`Error::from_error_with_context`].
//! If there is an error without context, one would have to convert to `anyhow::Error` as intermediate step,
//! typically by adding `.context()`.
pub(crate) use frontend::Error;

mod frontend {
    use gitbutler_core::error::{into_anyhow, AnyhowContextExt, ErrorWithContext};
    use serde::{ser::SerializeMap, Serialize};
    use std::borrow::Cow;

    /// An error type for serialization, dynamically extracting context information during serialization,
    /// meant for consumption by the frontend.
    #[derive(Debug)]
    pub struct Error(anyhow::Error);

    impl From<anyhow::Error> for Error {
        fn from(value: anyhow::Error) -> Self {
            Self(value)
        }
    }

    impl From<gitbutler_core::error::Error> for Error {
        fn from(value: gitbutler_core::error::Error) -> Self {
            Self(value.into())
        }
    }

    impl Error {
        /// Convert an error with context to our type.
        ///
        /// Note that this is only needed as trait specialization isn't working well enough yet.
        pub fn from_error_with_context(err: impl ErrorWithContext + Send + Sync + 'static) -> Self {
            Self(into_anyhow(err))
        }
    }

    impl Serialize for Error {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let ctx = self.0.custom_context().unwrap_or_default();

            let mut map = serializer.serialize_map(Some(2))?;
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

    #[cfg(test)]
    mod tests {
        use super::*;
        use anyhow::anyhow;
        use gitbutler_core::error::Context;

        fn json(err: anyhow::Error) -> String {
            serde_json::to_string(&Error(err)).unwrap()
        }

        #[test]
        fn find_context() {
            let err = anyhow!("err msg").context(Context::new_static("ctx msg"));
            assert_eq!(
                json(err),
                "{\"message\":\"ctx msg\"}",
                "Contexts often provide their own message, so the error message is ignored"
            );
        }
    }
}
