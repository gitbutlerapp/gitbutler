# GitButler panel plugin (DSH)

A DeepSeek Harness dynamic plugin that renders the real GitButler Lite workspace
(uncommitted files, stacks/branches, commits, context menus) in the harness's
details column, backed by the repo's own Lite code through the harness host.

This plugin is **not an npm package** — it is a runtime Cordis plugin defined in
a DSH session. These files are its source so it can be installed anywhere the
repo is present.

## What it needs

- A checkout of this repo **with the harness foundation and this branch**:
  - `apps/lite/harness/` (browser + node builds + this `deepseek/` dir) and the harness
    host CLI — `apps/lite/harness/deepseek/cli.mjs`
  - the `but-sdk` NAPI binding built for the machine (`pnpm -F @gitbutler/but-sdk build`)
- The panel bundle built once: `pnpm -F @gitbutler/lite build:harness`
  (produces `apps/lite/dist/harness/browser.js` + `lite.css`, which the plugin serves)
- A DSH session whose workspace is this repo (the plugin resolves the repo root
  from the session's workspace registry).

## Installing (this is the whole recipe)

In a DSH session whose workspace is the repo, tell the agent:

> Install the GitButler panel plugin: read `apps/lite/harness/deepseek/host.js`
> and `apps/lite/harness/deepseek/client.js` and define them as the host and
> client halves of a new Cordis plugin, then run it.

The agent runs `cordis_define` (paste the two files as `code.host` /
`code.client`) and `cordis_run`. **You never type or paste anything yourself** —
the files in the repo are what the agent reads.

Then click **GitButler** in the session header (or "Open panel" in the plugin's
run card). The panel mounts in the details column.

## What it does

- Real Lite workspace UI in the details column: uncommitted files, stacks,
  branches, commits — driven by the real SDK through the harness host.
- Event-driven refresh: real SDK watcher events push through a long-poll bridge
  (no polling of state).
- Right-click a file: the real Lite context menu (JS popup), with a plugin-only
  **Open in GitButler** item that deep-links the running app to that file, and
  **Open in VS Code** via `vscode://`.
- Hotkeys: `1`/`2` focus the two lists; arrow keys move through the focused one.
- Read/write: mutations (discard, absorb, commit, …) forward to the real SDK.

## Updating

- Repo-side changes (Lite source, harness bundle): rebuild with
  `pnpm -F @gitbutler/lite build:harness`, then restart the plugin run
  (`cordis_run` with mode `run` on the same package) so the served bundle refreshes.
- Plugin-side changes: edit `host.js` / `client.js`, then re-define a new package
  and `cordis_run --mode update`.
