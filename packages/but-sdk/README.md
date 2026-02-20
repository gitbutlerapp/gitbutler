# @gitbutler/but-sdk

`@gitbutler/but-sdk` is the local npm package that exposes GitButler Rust APIs to JavaScript/TypeScript through a native Node add-on.

## How it works

The generated SDK surface is produced in two parts and merged into a single declaration file:

1. **N-API bindings (functions)**
   - Rust functions marked with `#[but_api(napi)]` in `but-api` generate `*_napi` exports via the `but_api` proc macro.
   - `but-napi` links those generated exports and is built by `napi-rs` into a `.node` binary.
   - `napi-rs` generates:
     - `gitbutler-sdk.{PLATFORM}.node` (Node native add-on)
     - `src/generated/index.js` (runtime loader + JS bridge)
     - `src/generated/index.d.ts` (N-API function declarations)

2. **Schema-derived TypeScript types (structs/enums)**
   - `but-ts` collects registered JSON schemas from `but-api` (`but_api::schema` module) and emits TS type aliases.
   - These emitted type aliases are appended to `src/generated/index.d.ts`.


This package ships only generated artifacts (`index.js`, `index.d.ts`, `*.node`) and re-exports them through package `exports`.

## Generate bindings and types

Always run generation in this order to avoid duplicate schema blocks in `index.d.ts`:

```bash
pnpm --filter @gitbutler/but-sdk build:napi
pnpm --filter @gitbutler/but-sdk build:types
```

Or run the combined script:

```bash
pnpm --filter @gitbutler/but-sdk build
```

### Why order matters

`build:types` appends schema types to the existing declaration file. Running it repeatedly without regenerating N-API declarations first can duplicate schema type sections.

## Add new bindings and types

### Add a new exported N-API function

1. In `crates/but-api`, annotate a Rust API with `#[but_api(napi)]`.
2. Ensure input/output types are serializable in a way the macro supports.
3. Re-run:

```bash
pnpm --filter @gitbutler/but-sdk build:napi
```

You should see a new `*Napi` export in `src/generated/index.d.ts`.

### Add a new generated TypeScript type

1. Make sure the Rust type derives `schemars::JsonSchema`.
2. Register the type in `crates/but-api/src/schema.rs` using `TypeSchemaEntry`.
3. Re-run:

```bash
pnpm --filter @gitbutler/but-sdk build:types
```

You should see a new `export type ...` in `src/generated/index.d.ts`.

## Validate generated output

Baseline validation:

```bash
pnpm --filter @gitbutler/but-sdk check
```

Optional extra runtime/type sanity check:

```bash
pnpm --filter @gitbutler/but-sdk testTypes
```

Recommended contributor flow after changing bindings/types:

```bash
pnpm --filter @gitbutler/but-sdk build
pnpm --filter @gitbutler/but-sdk check
# optional
pnpm --filter @gitbutler/but-sdk testTypes
```

## Usage in Electron (`apps/lite` pattern)

Use native bindings in the Electron **main process**, then expose only typed IPC APIs to the renderer.

### Main process model

```ts
import { listProjectsNapi } from '@gitbutler/but-sdk';

export function listProjects() {
	return listProjectsNapi([]);
}
```

### IPC contract with SDK types

```ts
import type { ProjectForFrontend } from '@gitbutler/but-sdk';

export interface LiteElectronApi {
	listProjects(): Promise<ProjectForFrontend[]>;
}
```

### Renderer consumes IPC API

```ts
const projects = await window.lite.listProjects();
```

This keeps native access out of the renderer while sharing the same generated TS types (`ProjectForFrontend`, `StackEntry`, `StackDetails`, etc.) across process boundaries.

## Current exported binding entry points

See `src/generated/index.d.ts` for the complete generated API and type surface.

## See also

- [but-napi](../../crates/but-napi/README.md)
- [but-ts](../../crates/but-ts/README.md)

## License

FSL-1.1-MIT - See [LICENSE.md](../../LICENSE.md) for details.
