//! Support library for `but-api-macros` compile-time tests.
//!
//! This crate exists to give trybuild UI tests a small, controlled environment that
//! resembles the parts of `but-api` the `#[but_api]` macro expands against.
//! In particular it provides lightweight stand-ins for modules and types the *macro*
//! references directly from the `but-api` crate, (for example `json`, `panic_capture`) so
//! expansion can be validated without running the macro from the `but-api` crate that
//! it was designed for.
//!
//! Keep this crate intentionally minimal. Extend it only when macro output starts
//! referencing new paths, traits, or conversion behavior that existing test shims
//! cannot satisfy.

use std::str::FromStr;

pub mod panic_capture {
    pub fn panic_payload_to_anyhow(
        function_name: &str,
        _payload: Box<dyn std::any::Any + Send>,
    ) -> anyhow::Error {
        anyhow::anyhow!("panic captured in {function_name}")
    }
}

pub mod json {
    use std::str::FromStr;

    #[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
    #[serde(transparent)]
    pub struct HexHash(pub String);

    impl std::fmt::Display for HexHash {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.0)
        }
    }

    impl From<gix::ObjectId> for HexHash {
        fn from(value: gix::ObjectId) -> Self {
            Self(value.to_hex().to_string())
        }
    }

    impl From<HexHash> for gix::ObjectId {
        fn from(value: HexHash) -> Self {
            gix::ObjectId::from_hex(value.0.as_bytes())
                .expect("HexHash test helper must always store valid object id hex")
        }
    }

    impl FromStr for HexHash {
        type Err = gix::hash::decode::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let _ = gix::ObjectId::from_hex(s.as_bytes())?;
            Ok(Self(s.to_owned()))
        }
    }

    #[derive(Debug)]
    pub struct Error(pub anyhow::Error);

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl std::error::Error for Error {}

    impl From<anyhow::Error> for Error {
        fn from(value: anyhow::Error) -> Self {
            Self(value)
        }
    }

    impl From<serde_json::Error> for Error {
        fn from(value: serde_json::Error) -> Self {
            Self(value.into())
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct UiValue {
    pub value: i32,
}

impl From<i32> for UiValue {
    fn from(value: i32) -> Self {
        Self { value }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct UiValueTry {
    pub value: i32,
}

impl TryFrom<i32> for UiValueTry {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(Self { value })
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ComplexParam {
    pub a: i32,
    pub b: i32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ComplexResult {
    pub combined: i32,
}

impl From<(usize, isize)> for ComplexResult {
    fn from((u, i): (usize, isize)) -> Self {
        Self {
            combined: u as i32 + i as i32,
        }
    }
}

pub fn oid_from_hex(hex: &str) -> gix::ObjectId {
    gix::ObjectId::from_str(hex).expect("test literal object ids are valid")
}
