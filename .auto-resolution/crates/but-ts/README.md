# but-ts

`but-ts` generates TypeScript type declarations from JSON schemas registered in `but-api`.

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

1. `but-api` registers schema entries in `but_api::schema`.
2. `but-ts` collects the registry (`collect_all_schemas`).
3. It converts schema definitions into TypeScript declarations.
4. It writes/appends output to the target file (`--output`).

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

1. Make sure the Rust type derives `schemars::JsonSchema`.
2. Register it in `crates/but-api/src/schema.rs` with `TypeSchemaEntry`.
3. Re-run `pnpm --filter @gitbutler/but-sdk build:types`.
4. Confirm it appears in `packages/but-sdk/src/generated/index.d.ts`.

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
