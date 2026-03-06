//! Schemars utilities for json schema generation

pub fn stack_id_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<String>>()
}

pub fn stack_id(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

pub fn bstring(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

pub fn bstring_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<String>>()
}

pub fn object_id(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

pub fn object_id_vec(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Vec<String>>()
}

pub fn ref_full_name(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

pub fn url(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

pub fn project_id(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

pub fn default_true(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<bool>()
}

pub fn oid(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
}

pub fn object_id_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<String>>()
}

pub fn bstring_bytes(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Vec<u8>>()
}

pub fn bstring_bytes_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<Vec<u8>>>()
}

#[derive(schemars::JsonSchema)]
#[allow(dead_code)]
struct GixTime {
    seconds: i64,
    offset: i32,
}

pub fn gix_time_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<GixTime>>()
}

pub fn bstring_for_frontend(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<String>()
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

pub fn serde_error(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<SerdeErrorSchema>()
}

pub fn serde_error_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<SerdeErrorSchema>>()
}
