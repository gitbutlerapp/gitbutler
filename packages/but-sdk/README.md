# @gitbutler/but-sdk

`@gitbutler/but-sdk` is the local npm package that exposes GitButler Rust APIs to JavaScript/TypeScript through a native Node add-on.

## How it works

The generated SDK surface is produced in two parts and merged into a single declaration file:

1. **N-API bindings (functions)**
   - Rust functions marked with `#[but_api(napi)]` in `but-api` generate `*_napi` exports via the `but_api` proc macro.
   - `but-napi` links those generated exports and is built by `napi-rs` into a `.node` binary.
   - `napi-rs` generates, per flavor folder:
     - `gitbutler-sdk.{PLATFORM}.node` (Node native add-on)
     - `index.js` (runtime loader + JS bridge)
     - `index.d.ts` (N-API function declarations)

2. **Schema-derived TypeScript types (structs/enums)**
   - `but-ts` collects registered JSON schemas from `but-api` (`but_api::schema` module) and emits TS type aliases.
   - These emitted type aliases are appended to the flavor folder's `index.d.ts`.


This package ships only generated artifacts (`browser.js` plus each flavor's `index.js`, `index.d.ts`, `*.node`) and re-exports them through package `exports`.

## Workspace-projection flavors

Mutation APIs return a `WorkspaceState` whose projection depends on the
`graph-workspace` cargo feature of `but-api` (see its feature docs). The SDK
ships one fully self-contained build per flavor:

- `@gitbutler/but-sdk` (alias `@gitbutler/but-sdk/linear`, from
  `src/generated/linear/`): built without the feature; `WorkspaceState`
  carries the legacy `RefInfo`-based `headInfo` projection.
- `@gitbutler/but-sdk/graph` (from `src/generated/graph/`): built with
  `--features graph-workspace`; `WorkspaceState` carries the graph-based
  `graphWorkspace` (`DetailedGraphWorkspace`) projection.

Each folder has its own `.node` binary, loader, and declarations, so importing
an entry point loads the matching native build — types and runtime cannot
disagree for napi consumers. Both folders' `index.js`/`index.d.ts` are committed
and CI-checked.

## Generate bindings and types

Per flavor, always run the napi build before the type generation:

```bash
# linear flavor (src/generated/linear/)
pnpm --filter @gitbutler/but-sdk build:napi
pnpm --filter @gitbutler/but-sdk build:types
# graph flavor (src/generated/graph/)
pnpm --filter @gitbutler/but-sdk build:napi:graph
pnpm --filter @gitbutler/but-sdk build:types:graph
```

Or run the combined script, which builds both flavors:

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

You should see a new `*Napi` export in `src/generated/linear/index.d.ts` (and `src/generated/graph/index.d.ts` for the graph flavor).

### Add a new generated TypeScript type

1. Make sure the Rust type derives `schemars::JsonSchema`.
2. Register the type in `crates/but-api/src/schema.rs` using `TypeSchemaEntry`.
3. Re-run:

```bash
pnpm --filter @gitbutler/but-sdk build:types
```

You should see a new `export type ...` in `src/generated/linear/index.d.ts` (and `src/generated/graph/index.d.ts` for the graph flavor).

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
import { listProjectsStatelessNapi } from '@gitbutler/but-sdk';

export function listProjects() {
	return listProjectsStatelessNapi();
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

See `src/generated/linear/index.d.ts` (and `src/generated/graph/index.d.ts` for the graph flavor) for the complete generated API and type surface.

## See also

- [but-napi](../../crates/but-napi/README.md)
- [but-ts](../../crates/but-ts/README.md)

## License

FSL-1.1-MIT - See [LICENSE.md](../../LICENSE.md) for details.
