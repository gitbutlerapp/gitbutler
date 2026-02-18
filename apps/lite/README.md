# @gitbutler/lite

Electron + React (Vite) + TanStack Router scaffold.

## Structure

- `ui/`: renderer/frontend code
- `electron/`: main process and preload code

`ui` imports IPC API types directly from `electron/src/ipc.ts` using type-only imports for end-to-end type safety.

## Architecture decisions

### 1) App shape: split by process boundary

- Choice: keep renderer code in `ui/` and privileged Electron code in `electron/`.
- Why: Electron has two fundamentally different runtimes (browser renderer vs Node/Electron main). Splitting by runtime prevents accidental API usage across boundaries and keeps ownership clear.
- Trade-off: shared code must be explicit (via typed contracts), which adds a small amount of setup.

### 2) Renderer stack: React + TanStack Router + Vite

- Choice: use React for UI, TanStack Router for client routing, and Vite for frontend bundling.
- Why: Vite gives fast iteration and straightforward TS/ESM support; TanStack Router provides strongly typed route APIs from day one.
- Trade-off: router setup is more explicit than minimal alternatives, but better for long-term type safety.

### 3) Electron compilation: `tsc` (no extra bundler for main/preload)

- Choice: compile `electron/` TypeScript with `tsc` only (`build:electron`).
- Why: this is the smallest and most predictable setup in this monorepo, avoids introducing another bundler stack for Node-side code, and keeps sourcemap/debug behavior transparent.
- Trade-off: startup/build optimizations from tools like esbuild/tsup are deferred until they are actually needed.

### 4) Packaging: Electron Builder

- Choice: package output with `electron-builder` via the `package` script.
- Why: matches the requirement and provides a standard path to distributables.
- Why not Electron Forge: for this app, Electron Builder is the more flexible packaging layer. It gives finer control over artifact targets, file inclusion/exclusion, installer behavior, and publishing/release settings from a single configuration surface.
- Additional reasoning: Forge is excellent for batteries-included app scaffolding and plugin-driven workflows, but this project already has custom monorepo build orchestration and explicit renderer/main pipelines, so Builder's lower-level packaging control is a better fit.
- Trade-off: release/signing CI is intentionally not included yet; this scaffold focuses on local dev/build/check/package.

### 5) TypeScript project layout: separate tsconfigs per runtime

- Choice: independent TS configs for renderer and Electron (`ui/tsconfig.json`, `electron/tsconfig.json`) plus root project references.
- Why: each runtime needs different compiler semantics (`moduleResolution: bundler` in UI, `NodeNext` in Electron). Separation prevents subtle config conflicts.
- Trade-off: there are more config files, but each is simpler and runtime-specific.

### 6) IPC type safety model: contract-first in Electron

- Choice: define IPC channels and API interface in `electron/src/ipc.ts`; preload implements and exposes `window.lite`; UI consumes types through ambient declaration.
- Why: one source of truth for IPC contract ensures compile-time safety on both sides of the boundary.
- Trade-off: this couples the contract location to the Electron subtree; if it needs cross-app reuse later, it can be extracted to a shared package.

### 7) Security defaults: preload bridge + context isolation

- Choice: `contextIsolation: true`, `nodeIntegration: false`, and only expose approved API via `contextBridge`.
- Why: this is Electron’s safer default posture and limits renderer access to privileged capabilities.
- Trade-off: all privileged operations must be intentionally designed and added to the bridge.

### 8) Import strategy and linting: reuse monorepo ESLint config

- Choice: no app-local ESLint config; rely on root flat config and import rules.
- Why: keeps behavior consistent with the rest of the monorepo and avoids configuration drift.
- Trade-off: new app code must conform to global constraints (for example import ordering and no relative imports), which may require additional path aliases.

### 9) Type sharing implementation detail

- Choice: UI references Electron contract types via path alias (`#electron/*`) and type-only imports.
- Why: enables type safety across IPC wire without runtime coupling.
- Trade-off: alias wiring must stay in sync between package metadata and TS config.

## Scripts

- `pnpm --filter @gitbutler/lite dev`: run Vite + Electron
- `pnpm --filter @gitbutler/lite build`: build renderer and Electron code
- `pnpm --filter @gitbutler/lite check`: TypeScript checks for both targets
- `pnpm --filter @gitbutler/lite package`: build and package with Electron Builder

## ESLint

No app-local ESLint config is added. This app uses the monorepo root flat config in `eslint.config.js`.

## IPC contract

- API contract lives in `electron/src/ipc.ts`
- Preload exposes `window.lite`
- Renderer typings are declared in `ui/src/electron.d.ts`

## Future evolution points

- Add Electron main/preload watch/restart in dev if iteration speed becomes a bottleneck.
- Extract IPC contract into a dedicated shared workspace package only if multiple apps/processes need it.
- Add release/signing CI once distribution requirements are finalized.
