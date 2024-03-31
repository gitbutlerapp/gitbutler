pub use gitbutler_core::error::Code;
pub(crate) use legacy::Error;

mod frontend {
    use anyhow::Error;
    use gitbutler_core::error::{into_anyhow, AnyhowContextExt, ErrorWithContext};
    use serde::{ser::SerializeMap, Serialize};
    use std::borrow::Cow;

    /// An error type for serialization, dynamically extracting context information during serialization,
    /// meant for consumption by the frontend.
    #[derive(Debug)]
    pub struct Error2(anyhow::Error);

    impl From<anyhow::Error> for Error2 {
        fn from(value: Error) -> Self {
            Self(value)
        }
    }

    impl From<gitbutler_core::error::Error2> for Error2 {
        fn from(value: gitbutler_core::error::Error2) -> Self {
            Self(value.into())
        }
    }

    impl Error2 {
        /// Convert an error with context to our type.
        ///
        /// Note that this is only needed as trait specialization isn't working well enough yet.
        pub fn from_error_with_context(err: impl ErrorWithContext + Send + Sync + 'static) -> Self {
            Self(into_anyhow(err))
        }
    }

    impl Serialize for Error2 {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let ctx = self.0.custom_context().unwrap_or_default();

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

    #[cfg(test)]
    mod tests {
        use super::*;
        use anyhow::anyhow;
        use gitbutler_core::error::{Code, Context};

        fn json(err: anyhow::Error) -> String {
            serde_json::to_string(&Error2(err)).unwrap()
        }

        #[test]
        fn no_context_or_code() {
            let err = anyhow!("err msg");
            assert_eq!(
                json(err),
                "{\"code\":\"errors.unknown\",\"message\":\"Something went wrong\"}",
                "if there is no explicit error code or context, the original error message isn't shown"
            );
        }

        #[test]
        fn find_code() {
            let err = anyhow!("err msg").context(Code::Projects);
            assert_eq!(
                json(err),
                "{\"code\":\"errors.projects\",\"message\":\"err msg\"}",
                "the 'code' is available as string, but the message is taken from the source error"
            );
        }

        #[test]
        fn find_context() {
            let err = anyhow!("err msg").context(Context::new_static(Code::Projects, "ctx msg"));
            assert_eq!(
                json(err),
                "{\"code\":\"errors.projects\",\"message\":\"ctx msg\"}",
                "Contexts often provide their own message, so the error message is ignored"
            );
        }

        #[test]
        fn find_context_without_message() {
            let err = anyhow!("err msg").context(Context::from(Code::Projects));
            assert_eq!(
                json(err),
                "{\"code\":\"errors.projects\",\"message\":\"err msg\"}",
                "Contexts without a message show the error's message as well"
            );
        }

        #[test]
        fn find_nested_code() {
            let err = anyhow!("bottom msg")
                .context("top msg")
                .context(Code::Projects);
            assert_eq!(
                json(err),
                "{\"code\":\"errors.projects\",\"message\":\"top msg\"}",
                "the 'code' gets the message of the error that it provides context to, and it finds it down the chain"
            );
        }

        #[test]
        fn multiple_codes() {
            let err = anyhow!("bottom msg")
                .context(Code::Menu)
                .context("top msg")
                .context(Code::Projects);
            assert_eq!(
                json(err),
                "{\"code\":\"errors.projects\",\"message\":\"top msg\"}",
                "it finds the most recent 'code' (and the same would be true for contexts, of course)"
            );
        }
    }
}

//#[deprecated(
//    note = "the types in the error::legacy::* module are deprecated; use error::gb::Error and error::gb::Result instead"
//)]
mod legacy {
    use gitbutler_core::error::Code;
    use gitbutler_core::project_repository;
    use serde::{ser::SerializeMap, Serialize};

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
