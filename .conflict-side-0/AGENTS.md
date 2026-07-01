# GitButler Agent Instructions

GitButler is a Rust/Svelte/React/TypeScript monorepo.

Apply all relevant instruction files. If instructions conflict, resolve them in
this order:

1. Explicit human instructions
2. Nearest nested `AGENTS.md`
3. This file

## Repo Map

- `crates/` - Rust crates.
- `apps/desktop/` - Tauri/Svelte desktop app.
- `apps/web/` - Svelte web app.
- `apps/lite/` - Electron/React desktop app.
- `packages/` - shared TypeScript packages, including the SDK.
- `e2e/` - Playwright, WebdriverIO, and blackbox end-to-end tests.

## Working Style

- Make focused, reviewable changes.
- Inspect nearby code before introducing patterns.
- Prefer existing APIs, tests, and conventions.
- Avoid unrelated rewrites.
- Run targeted validation for the area touched.

## Scoped Instructions

- For Rust work under `crates/`, follow `crates/AGENTS.md`.
- For Lite work under `apps/lite/`, follow `apps/lite/AGENTS.md`.
