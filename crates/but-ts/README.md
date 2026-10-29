# but-ts

`but-ts` generates the TypeScript declarations of [`@gitbutler/but-sdk`](../../packages/but-sdk/README.md) from JSON schemas of Rust types.

## How it works

1. A crate registers a type with `but_schemars::register_sdk_type!(Type)`; the type derives `schemars::JsonSchema`.
2. `but-ts` links `but-api` and its dependencies and collects every registration (`collect_all_schemas`).
3. It converts each schema into a TypeScript type alias.
4. It writes them, sorted by name, to the file given with `--output`, replacing its content.

A registered type that references a type nobody registered, or two registrations with the same schema name, fail generation.

## Add a type

1. Derive `schemars::JsonSchema` on the Rust type, behind the crate's `export-schema` feature like its neighbours.
2. Call `but_schemars::register_sdk_type!(Type);` next to it.
3. Regenerate the SDK as its README says, and commit the changed declaration files.
