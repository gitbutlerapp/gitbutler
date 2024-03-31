pub use gitbutler_core::error::Code;
pub(crate) use legacy::Error;

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
