//! Schema registry for collecting JSON schemas for TypeScript type generation.
//!
//! Types are manually registered here. Only types that derive `schemars::JsonSchema`
//! can be registered. New types can be added incrementally as `JsonSchema` is derived
//! on more types across the codebase.
//!
//! The `but-schema-gen` tool reads these registrations and produces TypeScript definitions.

/// A registry entry for a type's JSON schema.
pub struct TypeSchemaEntry {
    /// The TypeScript-friendly name for this type (e.g. "TreeStats").
    pub name: &'static str,
    /// A function that generates the JSON schema for this type.
    pub schema_fn: fn() -> schemars::Schema,
}

inventory::collect!(TypeSchemaEntry);

/// Collect all registered type schemas.
///
/// Returns a list of `(name, schema)` pairs for all registered types.
/// This is used by the `but-schema-gen` tool to produce TypeScript definitions.
pub fn collect_all_schemas() -> Vec<(&'static str, schemars::Schema)> {
    let mut schemas: Vec<_> = inventory::iter::<TypeSchemaEntry>
        .into_iter()
        .map(|entry| (entry.name, (entry.schema_fn)()))
        .collect();
    // Deduplicate by name (same type may be registered by multiple functions)
    schemas.sort_by_key(|(name, _)| *name);
    schemas.dedup_by_key(|(name, _)| *name);
    schemas
}

// Register types that have JsonSchema derives.
// Add more registrations here as JsonSchema is derived on more types.
#[cfg(feature = "export-schema")]
mod registrations {
    // use super::TypeSchemaEntry;

    use but_workspace::{
        legacy::{StacksFilter, ui::StackEntry},
        ui::StackDetails,
    };
    use schemars::schema_for;

    use crate::schema::TypeSchemaEntry;

    inventory::submit! { TypeSchemaEntry {
        name: "StackDetails",
        schema_fn: || schema_for!(StackDetails)
    }}

    inventory::submit! { TypeSchemaEntry {
        name: "StacksFilter",
        schema_fn: || schema_for!(StacksFilter)
    }}

    inventory::submit! {TypeSchemaEntry {
        name: "StackEntry",
        schema_fn: || schema_for!(StackEntry)
    }}
}
