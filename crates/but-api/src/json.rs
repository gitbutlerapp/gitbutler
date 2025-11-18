pub use error::{Error, ToError, UnmarkedError};
use gix::refs::Target;
use schemars;
use schemars::JsonSchema;
use serde::Serialize;

mod hex_hash {
    use std::{ops::Deref, str::FromStr};

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// A type that deserializes a hexadecimal hash into an object id automatically.
    #[derive(Debug, Clone, Copy)]
    pub struct HexHash(pub gix::ObjectId);

    impl From<HexHash> for gix::ObjectId {
        fn from(value: HexHash) -> Self {
            value.0
        }
    }

    impl From<gix::ObjectId> for HexHash {
        fn from(value: gix::ObjectId) -> Self {
            HexHash(value)
        }
    }

    impl Deref for HexHash {
        type Target = gix::ObjectId;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<'de> Deserialize<'de> for HexHash {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            let hex = String::deserialize(deserializer)?;
            gix::ObjectId::from_str(&hex)
                .map(HexHash)
                .map_err(serde::de::Error::custom)
        }
    }

    impl Serialize for HexHash {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serializer.serialize_str(&self.0.to_hex().to_string())
        }
    }

    mod stringy {
        use schemars::JsonSchema;
        use serde::{Deserialize, Deserializer, Serialize, Serializer};
        use std::str::FromStr;

        /// A type that deserializes a hexadecimal hash into a string, unchanged.
        /// This is to workaround `schemars` which doesn't (always) work with transformations.
        #[derive(Debug, Clone, JsonSchema)]
        pub struct HexHashString(String);

        impl TryFrom<HexHashString> for gix::ObjectId {
            type Error = gix::hash::decode::Error;

            fn try_from(value: HexHashString) -> Result<Self, Self::Error> {
                value.0.parse()
            }
        }

        impl From<gix::ObjectId> for HexHashString {
            fn from(value: gix::ObjectId) -> Self {
                HexHashString(value.to_hex().to_string())
            }
        }

        impl<'de> Deserialize<'de> for HexHashString {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let hex = String::deserialize(deserializer)?;
                gix::ObjectId::from_str(&hex)
                    .map(|_| HexHashString(hex))
                    .map_err(serde::de::Error::custom)
            }
        }

        impl Serialize for HexHashString {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }
    }
    pub use stringy::HexHashString;

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn hex_hash() {
            let hex_str = "5c69907b1244089142905dba380371728e2e8160";
            let expected = gix::ObjectId::from_str(hex_str).expect("valid SHA1 hex-string");
            let actual =
                serde_json::from_str::<HexHash>(&format!("\"{hex_str}\"")).expect("input is valid");
            assert_eq!(actual.0, expected);

            let actual = serde_json::to_string(&actual);
            assert_eq!(
                actual.unwrap(),
                "\"5c69907b1244089142905dba380371728e2e8160\""
            );
        }
    }
}
pub use hex_hash::{HexHash, HexHashString};

mod error {
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

    use std::borrow::Cow;

    use but_error::AnyhowContextExt;
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

    impl From<Error> for anyhow::Error {
        fn from(value: Error) -> Self {
            value.0
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
        use but_error::{Code, Context};

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

/// To make bstring work with schemars.
#[cfg(feature = "path-bytes")]
fn bstring_schema(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    // TODO: implement this. How to get description and what not?
    generate.root_schema_for::<String>()
}

/// The full name of a Git reference.
#[derive(Debug, Clone, schemars::JsonSchema, Serialize)]
pub struct FullRefName {
    /// The full name, like `refs/heads/main` or `refs/remotes/origin/foo`.
    /// Note that it might be degenerated if it can't be represented in Unicode.
    pub full: String,
    /// `full` without degeneration, as plain bytes.
    #[cfg(feature = "path-bytes")]
    #[schemars(schema_with = "bstring_schema")]
    pub full_bytes: bstr::BString,
}

impl From<gix::refs::FullName> for FullRefName {
    fn from(value: gix::refs::FullName) -> Self {
        FullRefName {
            full: value.as_bstr().to_string(),
            #[cfg(feature = "path-bytes")]
            full_bytes: value.as_bstr().into(),
        }
    }
}

/// A Git reference identified by its full reference name, along with the information Git stores about it.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Reference {
    /// The full name, like `refs/heads/main` or `refs/remotes/origin/foo`.
    /// Note that it might be degenerated if it can't be represented in Unicode.
    pub name: FullRefName,
    /// Set if the reference points to an object id. This is the common case.
    #[serde(default)]
    pub target_id: Option<HexHashString>,
    /// Set if the reference points to the name of another reference. This happens if the reference is symbolic.
    #[serde(default)]
    pub target_ref: Option<FullRefName>,
}

impl From<gix::refs::Reference> for Reference {
    fn from(
        gix::refs::Reference {
            name,
            target,
            peeled: _ignored,
        }: gix::refs::Reference,
    ) -> Self {
        Reference {
            name: name.into(),
            target_id: match &target {
                Target::Object(id) => Some((*id).into()),
                Target::Symbolic(_) => None,
            },
            target_ref: match target {
                Target::Object(_) => None,
                Target::Symbolic(rn) => Some(rn.into()),
            },
        }
    }
}
