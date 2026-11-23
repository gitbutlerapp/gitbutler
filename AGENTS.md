# GitButler Agent Instructions

GitButler is a Rust/Svelte/React/TypeScript monorepo.

## Repo Map

- `crates/` - Rust crates.
- `crates/but-gpui/` - the GPUI desktop client.
- `apps/desktop/` - Tauri/Svelte desktop app.
- `apps/web/` - Svelte web app.
- `apps/but-ui-pantry/` - the pantry, the design system's site.
- `packages/` - shared TypeScript packages, including the SDK.
- `packages/ui-svelte/` - the components the desktop and web apps use.
- `packages/but-ui/`, `packages/but-ui-react/`, `crates/but-ui-gpui/` - the design system:
  tokens, React components, and their GPUI counterparts.
- `e2e/` - Playwright, WebdriverIO, and blackbox end-to-end tests.

## Commits and PRs

Do not leave agent creditation in PRs or Commit messages. These are disallowed
in GitButler repos.

## The Pantries

- "The pantry" is the design system's site: the design notes and every
  component in its states. Run it with `pnpm -F @gitbutler/but-ui-pantry dev`
  (port 6007). It's hosted at <https://pantry.gitbutler.com> from master, and
  a pull request that changes it gets a Vercel preview.
- "The GPUI pantry" is the GPUI client's window of the shared controls. Drive
  it with the `gpui-screenshots` skill (`drive.sh run --stories`).

## Working Style

- Before declaring shared behavior done, check each applicable surface and contract
  (desktop, the GPUI client, web, CLI/TUI, SDK types, and docs) and update it or
  explicitly determine that it is unaffected.
