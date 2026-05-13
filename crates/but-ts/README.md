# but-ts

`but-ts` generates TypeScript type declarations from JSON schemas registered through
the SDK schema inventory that gets linked via `but-api`.

## High-level

This crate is the schema-to-TypeScript half of the `@gitbutler/but-sdk` declaration pipeline:

- `napi-rs` writes function bindings (`*Napi`) and corresponding type declarations into `index.d.ts`.
- `but-ts` appends Rust-struct/enum-based type aliases into the same `index.d.ts`.

## Why is this needed?
Napi RS can generate TypeScript types for structs and enums as well, but it would require us to tag them with the `#[napi]` macro & re-export the type from `but-napi`.

1. This would become a bit messy. Especially taking into account that we have a custom setup in which the `but_api` macro is the one marking the functions as *napi export targets*.
2. We already have another system in place for TypeScript-from-Rust generation for the desktop application.

This crate allows us to fully control how and where the types are generated.

## How does this compare to TS-RS
TS-RS generates the types as part of tests, which is a bit awkward and we've had to add workarounds for them.
This crate gives us full flexibility in how the types are generated, and to which output file.

In order to add the missing types in the Napi-RS-generated declaration file, we needed to be able to append the types to it. Which is something we couldn't easily do with TS-RS without breaking the current setup.

This crate is intended to be the single solution for TypeScript type generation.

## How it works

1. `but-ts` links `but-api` so all API modules and their schema registrations are reachable.
2. Transport schemas are registered into `inventory` as `but_schemars::SchemarEntry`:
   - `#[but_api(...)]` marks exported API functions and (for schema export builds) enforces that
     complex transport types are declared through `#[but_transport]`.
   - `#[but_transport]` applies transport defaults (serde casing/derive + schemars derive)
     and auto-registers the type for SDK export.
   - `but_schemars::register_sdk_type!(Type)` can still be used for non-transport helper types.
3. `but-ts` collects the inventory (`collect_all_schemas`).
4. It converts schema definitions into TypeScript declarations.
5. It writes/appends output to the target file (`--output`).

## Build entrypoint

From repository root, use the `but-sdk` scripts:

```bash
pnpm --filter @gitbutler/but-sdk build:types
```

To regenerate both bindings and types in the recommended order:

```bash
pnpm --filter @gitbutler/but-sdk build
```

## Add a new generated type

For transport DTOs used by `#[but_api]` endpoints:

1. Declare the DTO with `#[but_api_macros::but_transport]` (or `deserialize` variant when needed).
2. Keep crate feature wiring consistent with transport macros:
   - include an `export-schema` feature in the crate that declares DTOs
   - depend on `but-api-macros` in that crate
3. Re-run `pnpm --filter @gitbutler/but-sdk build:types`.
4. Confirm it appears in `packages/but-sdk/src/generated/index.d.ts`.

For non-transport types that still need SDK schema export:

1. Derive `schemars::JsonSchema`.
2. Register with `but_schemars::register_sdk_type!(Type)`.
3. Re-run `pnpm --filter @gitbutler/but-sdk build:types`.

## Validation

After regenerating types, validate with:

```bash
pnpm --filter @gitbutler/but-sdk check
```

Optional runtime/type sanity validation:

```bash
pnpm --filter @gitbutler/but-sdk testTypes
```

## See also

- [but-sdk](../../packages/but-sdk/README.md)
- [but-napi](../but-napi/README.md)
