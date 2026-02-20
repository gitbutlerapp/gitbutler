//! Schema registry for collecting JSON schemas for TypeScript type generation.
//!
//! Types are manually registered here. Only types that derive `schemars::JsonSchema`
//! can be registered. New types can be added incrementally as `JsonSchema` is derived
//! on more types across the codebase.
//!
//! The `but-ts` tool reads these registrations and produces TypeScript definitions.

/// A registry entry for a type's JSON schema.
pub struct TypeSchemaEntry {
    /// The TypeScript-friendly name for this type (e.g. "TreeStats").
    pub name: &'static str,
    /// A function that generates the JSON schema for this type.
    pub schema_fn: fn() -> schemars::Schema,
}

/// Collect all registered type schemas.
///
/// Returns a list of `(name, schema)` pairs for all registered types.
/// This is used by the `but-ts` tool to produce TypeScript definitions.
pub fn collect_all_schemas() -> Vec<(&'static str, schemars::Schema)> {
    #[cfg(feature = "export-schema")]
    {
        use but_workspace::{
            legacy::{StacksFilter, ui::StackEntry},
            ui::StackDetails,
        };
        use schemars::schema_for;

        // Register types that have JsonSchema derives. Add more entries here as derives
        // are added across the codebase.
        let mut schemas: Vec<_> = [
            TypeSchemaEntry {
                name: "StackDetails",
                schema_fn: || schema_for!(StackDetails),
            },
            TypeSchemaEntry {
                name: "StacksFilter",
                schema_fn: || schema_for!(StacksFilter),
            },
            TypeSchemaEntry {
                name: "StackEntry",
                schema_fn: || schema_for!(StackEntry),
            },
            TypeSchemaEntry {
                name: "StackId",
                schema_fn: || schema_for!(String),
            },
            TypeSchemaEntry {
                name: "ProjectId",
                schema_fn: || schema_for!(String),
            },
            #[cfg(feature = "legacy")]
            TypeSchemaEntry {
                name: "ProjectForFrontend",
                schema_fn: || schema_for!(crate::legacy::projects::ProjectForFrontend),
            },
        ]
        .into_iter()
        .map(|entry| (entry.name, (entry.schema_fn)()))
        .collect();

        // Keep deterministic output and deduplicate by name in case of accidental duplicates.
        schemas.sort_by_key(|(name, _)| *name);
        schemas.dedup_by_key(|(name, _)| *name);
        schemas
    }

    #[cfg(not(feature = "export-schema"))]
    {
        Vec::new()
    }
}
