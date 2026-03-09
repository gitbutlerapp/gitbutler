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
            ui::{
                StackDetails,
                ref_info::{BranchReference, RemoteTrackingReference, Segment, Stack, Target},
            },
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
            TypeSchemaEntry {
                name: "RefInfo",
                schema_fn: || schema_for!(but_workspace::ui::RefInfo),
            },
            TypeSchemaEntry {
                name: "BranchReference",
                schema_fn: || schema_for!(BranchReference),
            },
            TypeSchemaEntry {
                name: "RemoteTrackingReference",
                schema_fn: || schema_for!(RemoteTrackingReference),
            },
            TypeSchemaEntry {
                name: "Target",
                schema_fn: || schema_for!(Target),
            },
            TypeSchemaEntry {
                name: "Stack",
                schema_fn: || schema_for!(Stack),
            },
            TypeSchemaEntry {
                name: "Segment",
                schema_fn: || schema_for!(Segment),
            },
            TypeSchemaEntry {
                name: "ProjectHandleOrLegacyProjectId",
                schema_fn: || schema_for!(String),
            },
            #[cfg(feature = "legacy")]
            TypeSchemaEntry {
                name: "ProjectForFrontend",
                schema_fn: || schema_for!(crate::legacy::projects::ProjectForFrontend),
            },
            TypeSchemaEntry {
                name: "UnifiedPatch",
                schema_fn: || schema_for!(but_core::UnifiedPatch),
            },
            TypeSchemaEntry {
                name: "TreeChange",
                schema_fn: || schema_for!(but_core::ui::TreeChange),
            },
            TypeSchemaEntry {
                name: "TreeChange",
                schema_fn: || schema_for!(but_core::ui::TreeChange),
            },
            TypeSchemaEntry {
                name: "TreeChanges",
                schema_fn: || schema_for!(but_core::ui::TreeChanges),
            },
            TypeSchemaEntry {
                name: "CommitDetails",
                schema_fn: || schema_for!(crate::diff::json::CommitDetails),
            },
            TypeSchemaEntry {
                name: "WorktreeChanges",
                schema_fn: || schema_for!(but_hunk_assignment::WorktreeChanges),
            },
            TypeSchemaEntry {
                name: "HunkAssignmentRequest",
                schema_fn: || schema_for!(but_hunk_assignment::HunkAssignmentRequest),
            },
            TypeSchemaEntry {
                name: "AssignmentRejection",
                schema_fn: || schema_for!(but_hunk_assignment::AssignmentRejection),
            },
            TypeSchemaEntry {
                name: "DiffSpec",
                schema_fn: || schema_for!(but_core::DiffSpec),
            },
            TypeSchemaEntry {
                name: "InsertSide",
                schema_fn: || {
                    // InsertSide is a simple enum with variants "above" and "below" (camelCase serialized)
                    serde_json::from_str::<schemars::Schema>(r#"{
                        "type": "string",
                        "enum": ["above", "below"],
                        "description": "Describes where relative to the selector a step should be inserted"
                    }"#).expect("valid schema JSON")
                },
            },
            TypeSchemaEntry {
                name: "RelativeTo",
                schema_fn: || schema_for!(crate::commit::ui::RelativeTo),
            },
            TypeSchemaEntry {
                name: "UICommitCreateResult",
                schema_fn: || schema_for!(crate::json::UICommitCreateResult),
            },
            TypeSchemaEntry {
                name: "UIMoveChangesResult",
                schema_fn: || schema_for!(crate::json::UIMoveChangesResult),
            },
            TypeSchemaEntry {
                name: "UICommitInsertBlankResult",
                schema_fn: || schema_for!(crate::json::UICommitInsertBlankResult),
            },
            TypeSchemaEntry {
                name: "UICommitRewordResult",
                schema_fn: || schema_for!(crate::json::UICommitRewordResult),
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
