//! The API layer is what can be used to create GitButler applications.
//!
//! ### Coordinating Filesystem Access
//!
//! For them to behave correctly in multi-threaded scenarios, be sure to use an *exclusive or shared* lock
//! on this level.
//! Lower-level crates like `but-workspace` won't use filesystem-based locking beyond what Git offers natively.
use std::sync::Arc;

use but_broadcaster::Broadcaster;
use but_claude::bridge::Claudes;
use serde::Deserialize;
use tokio::sync::Mutex;

pub mod commands;
pub use commands::*;
pub mod error;
pub mod hex_hash;

#[derive(Clone)]
pub struct App {
    pub broadcaster: Arc<Mutex<Broadcaster>>,
    pub archival: Arc<but_feedback::Archival>,
    pub claudes: Arc<Claudes>,
}

#[derive(Deserialize)]
pub struct NoParams {}

/// Types meant to be serialised to JSON, without degenerating information despite the need to be UTF-8 encodable.
/// EXPERIMENTAL
pub mod json {
    use gix::refs::Target;
    use schemars;
    use serde::Serialize;

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
    // TODO: make this work with schemars
    #[derive(Debug, Clone, Serialize)]
    pub struct Reference {
        /// The full name, like `refs/heads/main` or `refs/remotes/origin/foo`.
        /// Note that it might be degenerated if it can't be represented in Unicode.
        pub name: FullRefName,
        /// Set if the reference points to an object id. This is the common case.
        #[serde(with = "but_serde::object_id_opt", default)]
        pub target_id: Option<gix::ObjectId>,
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
                    Target::Object(id) => Some(*id),
                    Target::Symbolic(_) => None,
                },
                target_ref: match target {
                    Target::Object(_) => None,
                    Target::Symbolic(rn) => Some(rn.into()),
                },
            }
        }
    }
}
