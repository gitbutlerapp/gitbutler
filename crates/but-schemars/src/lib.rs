//! Schemars utilities for json schema generation

pub fn stack_id_opt(generate: &mut schemars::SchemaGenerator) -> schemars::Schema {
    generate.subschema_for::<Option<String>>()
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
