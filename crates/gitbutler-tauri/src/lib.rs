#![cfg_attr(
    all(windows, not(test), not(debug_assertions)),
    windows_subsystem = "windows"
)]
// FIXME(qix-): Stuff we want to fix but don't have a lot of time for.
// FIXME(qix-): PRs welcome!
#![allow(
    clippy::used_underscore_binding,
    clippy::module_name_repetitions,
    clippy::struct_field_names,
    clippy::too_many_lines
)]

mod app;
pub use app::App;

pub mod commands;

pub mod logs;
pub mod menu;
pub mod window;
pub use window::state::event::ChangeForFrontend;
pub use window::state::WindowState;

pub mod askpass;
pub mod config;
pub mod error;
pub mod forge;
pub mod github;
pub mod modes;
pub mod open;
pub mod projects;
pub mod remotes;
pub mod repo;
pub mod secret;
pub mod undo;
pub mod users;
pub mod virtual_branches;

pub mod settings;
pub mod stack;
pub mod zip;

pub mod diff;
pub mod env;
pub mod workspace;

/// Utility types that make it easier to transform data from the frontend to the backend.
///
/// Note that these types *should not* be used to transfer anything to the frontend.
mod from_json {
    use serde::{Deserialize, Deserializer};
    use std::str::FromStr;

    /// A type that deserializes a hexadecimal hash into an object id automatically.
    #[derive(Debug, Clone)]
    pub struct HexHash(gix::ObjectId);

    impl From<HexHash> for gix::ObjectId {
        fn from(value: HexHash) -> Self {
            value.0
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
        }
    }
}
