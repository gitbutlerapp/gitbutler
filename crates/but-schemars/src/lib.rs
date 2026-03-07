//! Schemars utilities for JSON schema generation.
//!
//! Each helper in this crate is meant to be used from
//! `#[schemars(schema_with = "...")]` on the field whose runtime type should be
//! exported as a simpler JSON shape.
//!
//! Put the annotation on the field itself, next to the matching `serde`
//! override when there is one:
//!
//! ```rust
//! #[derive(serde::Serialize, schemars::JsonSchema)]
//! struct Example {
//!     #[serde(with = "but_serde::object_id")]
//!     #[schemars(schema_with = "but_schemars::object_id")]
//!     id: gix::ObjectId,
//! }
//! ```
//!
//! The examples below intentionally show the concrete field type that should
//! carry each annotation.

/// Use on `Option<StackId>` fields that should appear as `string | null`.
///
/// ```rust
/// #[derive(schemars::JsonSchema)]
/// struct Example {
///     #[schemars(schema_with = "but_schemars::stack_id_opt")]
///     id: Option<but_core::ref_metadata::StackId>,
/// }
/// ```
pub fn stack_id_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<String>>()
}

/// Use on `StackId` fields that should appear as `string`.
///
/// ```rust
/// #[derive(schemars::JsonSchema)]
/// struct Example {
///     #[schemars(schema_with = "but_schemars::stack_id")]
///     id: but_core::ref_metadata::StackId,
/// }
/// ```
pub fn stack_id(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

/// Use on string-like fields that serialize lossily as plain strings.
///
/// This applies to:
/// - `BString` fields serialized with `but_serde::bstring_lossy`
/// - `BStringForFrontend` fields serialized as strings
///
/// ```rust
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct Example {
///     #[serde(with = "but_serde::bstring_lossy")]
///     #[schemars(schema_with = "but_schemars::bstring_lossy")]
///     name: bstr::BString,
/// }
/// ```
pub fn bstring_lossy(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

/// Use on optional *lossy* `BString` fields serialized with
/// `but_serde::bstring_lossy_opt`.
///
/// This applies to:
/// - `BString` fields serialized with `but_serde::bstring_lossy`
/// - `BStringForFrontend` fields serialized as strings
///
/// ```rust
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct Example {
///     #[serde(with = "but_serde::bstring_lossy_opt")]
///     #[schemars(schema_with = "but_schemars::bstring_lossy_opt")]
///     linked_worktree_id: Option<bstr::BString>,
/// }
/// ```
pub fn bstring_lossy_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<String>>()
}

/// Use on `gix::ObjectId` fields serialized with `but_serde::object_id`.
///
/// ```rust
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct Example {
///     #[serde(with = "but_serde::object_id")]
///     #[schemars(schema_with = "but_schemars::object_id")]
///     id: gix::ObjectId,
/// }
/// ```
pub fn object_id(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

/// Use on `Vec<gix::ObjectId>` fields serialized with `but_serde::object_id_vec`.
///
/// ```rust
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct Example {
///     #[serde(with = "but_serde::object_id_vec")]
///     #[schemars(schema_with = "but_schemars::object_id_vec")]
///     parent_ids: Vec<gix::ObjectId>,
/// }
/// ```
pub fn object_id_vec(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Vec<String>>()
}

/// Use on `gix::refs::FullName` fields serialized with
/// `but_serde::fullname_lossy`.
///
/// ```rust
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct Example {
///     #[serde(with = "but_serde::fullname_lossy")]
///     #[schemars(schema_with = "but_schemars::fullname_lossy")]
///     reference: gix::refs::FullName,
/// }
/// ```
pub fn fullname_lossy(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

/// Use on `url::Url` fields that should appear as strings in schema output.
///
/// ```rust
/// #[derive(schemars::JsonSchema)]
/// struct Example {
///     #[schemars(schema_with = "but_schemars::url")]
///     gravatar_url: url::Url,
/// }
/// ```
pub fn url(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

/// Use on project identifier fields that serialize as strings.
///
/// ```rust
/// #[derive(schemars::JsonSchema)]
/// struct Example {
///     #[schemars(schema_with = "but_schemars::project_id")]
///     id: gitbutler_project::ProjectId,
/// }
/// ```
pub fn project_id(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

/// Use on `DefaultTrue` wrapper fields that should appear as booleans.
///
/// ```rust
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct Example {
///     #[serde(default)]
///     #[schemars(schema_with = "but_schemars::default_true")]
///     ok_with_force_push: DefaultTrue,
/// }
/// ```
pub fn default_true(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<bool>()
}

/// Use on legacy `git2::Oid` fields serialized with `but_serde::oid`.
///
/// ```rust
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct Example {
///     #[serde(with = "but_serde::oid")]
///     #[schemars(schema_with = "but_schemars::oid")]
///     id: git2::Oid,
/// }
/// ```
pub fn oid(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

/// Use on `Option<gix::ObjectId>` fields serialized with `but_serde::object_id_opt`.
///
/// ```rust
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct Example {
///     #[serde(with = "but_serde::object_id_opt")]
///     #[schemars(schema_with = "but_schemars::object_id_opt")]
///     base: Option<gix::ObjectId>,
/// }
/// ```
pub fn object_id_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<String>>()
}

/// Use on raw `BString` byte payloads that should appear as `number[]`.
///
/// ```rust
/// #[derive(schemars::JsonSchema)]
/// struct Example {
///     #[schemars(schema_with = "but_schemars::bstring_bytes")]
///     path_bytes: bstr::BString,
/// }
/// ```
pub fn bstring_bytes(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Vec<u8>>()
}

/// Use on optional raw `BString` byte payloads that should appear as
/// `number[] | null`.
///
/// ```rust
/// #[derive(schemars::JsonSchema)]
/// struct Example {
///     #[schemars(schema_with = "but_schemars::bstring_bytes_opt")]
///     previous_path: Option<bstr::BString>,
/// }
/// ```
pub fn bstring_bytes_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<Vec<u8>>>()
}

#[derive(schemars::JsonSchema)]
#[allow(dead_code)]
struct GixTime {
    seconds: i64,
    offset: i32,
}

/// Use on optional `gix::date::Time` fields that should keep the same
/// `{ seconds, offset }` shape as the serialized value.
///
/// ```rust
/// #[derive(schemars::JsonSchema)]
/// struct Example {
///     #[schemars(schema_with = "but_schemars::gix_time_opt")]
///     created_at: Option<gix::date::Time>,
/// }
/// ```
pub fn gix_time_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<GixTime>>()
}

#[derive(schemars::JsonSchema)]
#[schemars(rename = "EntryKind")]
#[allow(dead_code)]
enum EntryKindSchema {
    Tree,
    Blob,
    BlobExecutable,
    Link,
    Commit,
}

/// Use on `gix::object::tree::EntryKind` fields that should export the stable
/// enum used by the frontend.
///
/// ```rust
/// #[derive(schemars::JsonSchema)]
/// struct Example {
///     #[schemars(schema_with = "but_schemars::entry_kind")]
///     kind: gix::object::tree::EntryKind,
/// }
/// ```
pub fn entry_kind(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<EntryKindSchema>()
}

/// Schema for `serde_error::Error` which serializes as `{description: string, source?: Error | null}`.
#[derive(schemars::JsonSchema)]
#[schemars(rename = "SerdeError")]
#[allow(dead_code)]
struct SerdeErrorSchema {
    description: String,
    source: Option<Box<SerdeErrorSchema>>,
}

/// Use on `serde_error::Error` fields.
///
/// ```rust
/// #[derive(schemars::JsonSchema)]
/// struct Example {
///     #[schemars(schema_with = "but_schemars::serde_error")]
///     error: serde_error::Error,
/// }
/// ```
pub fn serde_error(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<SerdeErrorSchema>()
}

/// Use on `Option<serde_error::Error>` fields.
///
/// ```rust
/// #[derive(schemars::JsonSchema)]
/// struct Example {
///     #[schemars(schema_with = "but_schemars::serde_error_opt")]
///     error: Option<serde_error::Error>,
/// }
/// ```
pub fn serde_error_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<SerdeErrorSchema>>()
}
