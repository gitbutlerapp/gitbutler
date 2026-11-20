# GitButler Agent Instructions

GitButler is a Rust/Svelte/React/TypeScript monorepo.

## Repo Map

- `crates/` - Rust crates.
- `apps/desktop/` - Tauri/Svelte desktop app.
- `apps/web/` - Svelte web app.
- `apps/but-ui-pantry/` - the pantry, the design system's site.
- `packages/` - shared TypeScript packages, including the SDK.
- `e2e/` - Playwright, WebdriverIO, and blackbox end-to-end tests.

## The Pantries

- "The pantry" is the design system's site: the design notes and every
  component in its states. Run it with `pnpm -F @gitbutler/but-ui-pantry dev`
  (port 6007).
- "The GPUI pantry" is the GPUI client's window of the shared controls. Run it
  with `cargo run -p but-gpui --features stories --bin gitbutler-gpui-pantry`.

## Working Style

- Before declaring shared behavior done, check each applicable surface and contract
  (desktop, web, CLI/TUI, SDK types, and docs) and update it or explicitly
  determine that it is unaffected.

## Scoped Instructions

- For Rust work under `crates/`, follow `crates/AGENTS.md`.
- For desktop work under `apps/desktop/`, follow `apps/desktop/AGENTS.md`.
- For the React component library under `packages/but-ui-react/`, follow
  `packages/but-ui-react/AGENTS.md`.
