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
            serde_json::to_string(&Error(err)).unwrap()
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
