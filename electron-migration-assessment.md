# Migrating apps/desktop from Tauri to Electron — feasibility assessment

Question: can code from `apps/lite` be reused to move `apps/desktop` off Tauri
onto Electron? Answer: **yes — the codebase is half-prepared for this already.**
Lite's Electron shell is reusable nearly as-is, desktop's Svelte frontend is
nearly untouched, and the bridge between them mostly exists (`but-napi` +
desktop's `IBackend` abstraction).

## Why this is unusually feasible

### Desktop's frontend is already decoupled from Tauri

- Every Tauri touchpoint goes through one interface: `IBackend` (~40 methods)
  in `apps/desktop/src/lib/backend/backend.ts`.
- Two implementations exist, selected at build time via
  `VITE_BUILD_TARGET === "web"` in `apps/desktop/src/lib/backend/index.ts`:
  - `tauri.ts` (~260 lines) — wraps `@tauri-apps/api` + plugins.
  - `web.ts` (~360 lines) — HTTP POST per command + WebSocket `/ws` for
    events, against `crates/but-server` (axum). This is what Playwright e2e
    runs in CI, so it works today.
- **Zero `@tauri-apps/*` imports outside `src/lib/backend/`.** Only ~28 direct
  `.invoke(` and 8 `.listen(` call sites, mostly inside RTK-Query endpoints.
- Frontend migration = write a third `electron.ts` implementation + one branch
  in `index.ts`.

### Lite's Electron shell is frontend-agnostic

Reusable as-is (no React assumptions), all under `apps/lite/electron/src` and
`apps/lite/scripts`:

- **In-process napi transport**: main process loads the Rust backend as a
  native module (`@gitbutler/but-sdk`, built by `crates/but-napi`). No
  subprocess, no HTTP.
- **IPC bridge**: sender-validated `ipcMain.handle` + `contextBridge` preload.
- **`WatcherManager`** (`watcher.ts` + `watcher-architecture.md`): one Rust
  watcher per project, per-subscription fan-out channels, cleanup on window
  destruction/shutdown. Already multi-window-safe.
- **Auto-updater** (`updater.ts`): electron-updater + S3 feed
  (`releases.gitbutler.com`).
- **Askpass**: `prepare-askpass.mjs` builds/bundles `gitbutler-git-askpass`;
  main sets `GITBUTLER_ASKPASS_BIN`, forwards prompts over IPC.
- **Settings machinery** (`settings.ts`): atomic writes + arktype migration
  (swap the schema).
- **Packaging**: hardened electron-builder config (fuses, entitlements,
  mac/linux/win targets) in `apps/lite/package.json`.

Only React-coupled code lives under `apps/lite/ui/` and stays behind.

### One Rust source generates all three bindings

The `#[but_api]` macro (`crates/but-api-macros/src/lib.rs`) emits per command:
a Tauri variant (registered in `gitbutler-tauri`'s `generate_handler!`, ~214
commands), a JSON variant (wired into `but-server`'s routes, large subset), and
an opt-in napi variant (built into the SDK by `crates/but-napi`; lite uses ~70
ops). Exposing desktop's full command set to Electron is an annotation sweep +
SDK regen — mechanical, not design work.

### Events are already transport-agnostic

Desktop's `Broadcaster` (`FrontendEvent {name, payload}`) is duplicated
identically in `gitbutler-tauri` (→ `window.emit`) and `but-server`
(→ WebSocket frames). Same shape → frontend handlers don't care. Electron
maps it onto `webContents.send`, with lite's `WatcherManager` as the template.

## Two migration paths

**Path A — fastest proof: Electron + but-server.** Lite-derived Electron shell
loads desktop's web build (`VITE_BUILD_TARGET=web`) and spawns `but-server`;
frontend runs in its existing web mode over HTTP/WS. Works today in CI.
Downsides: subprocess/port management, a localhost HTTP surface in a shipped
app, and web mode's deliberate stubs.

**Path B — target architecture: in-process napi, lite-style.** `electron.ts`
implements `IBackend.invoke(command, params)` as one generic
`ipcRenderer.invoke` channel; main dispatches command name → SDK function
table. Desktop's generic `invoke` fits Electron _better_ than lite's ~70
bespoke channels. Events via Broadcaster → napi callback → `webContents.send`.

Sensible sequence: A then B — prove the UI in an Electron window first, then
swap the transport underneath `IBackend` without touching the UI again.

## The actual work (not free from either app)

Shell-native code in `crates/gitbutler-tauri` that dies with Tauri:

- **App menu** — `menu.rs` (12 KB) builds the native menu in Rust; rebuild
  with Electron `Menu`. (Lite only has context menus.) Frontend surface is
  small: `menu_item_set_enabled` + `menu://shortcut` events.
- **Multi-window + window state** — `window.rs`, open-project-in-new-window,
  `tauri-plugin-window-state`. Lite is single-window; watcher layer already
  copes, window creation/state persistence needs writing.
- **Deep links** (`but`/`but-dev`/`but-nightly`) and **single-instance** —
  standard Electron APIs, must be wired.
- **Updater migration** — Tauri updater (own signing/feed) → electron-updater.
  Lite's setup is the template, but release infra, signing, and migrating
  existing installs across shell technologies is the riskiest part (ops more
  than code).
- **web.ts stubs as checklist** — the deliberate web-mode stubs (`readFile`,
  disk store persistence, `relaunch`, deep links, updater, naive path
  helpers) are exactly the `IBackend` methods needing real Electron
  implementations.
- **napi annotation sweep** — extend `#[but_api(napi)]` from lite's ~70 ops to
  desktop's command set; regenerate SDK.

Watch item: command registries are hand-maintained per shell (Tauri
`generate_handler!`, but-server route list). A third list invites drift —
consider making the macro emit a registry before the transition.

## Spike results (2026-07-18)

Path A was spiked successfully in ~30 minutes with zero changes to desktop
source code:

- `but-server` (debug build) on port 6978; desktop dev server with
  `VITE_BUILD_TARGET=web VITE_BUTLER_API_BASE_URL=http://127.0.0.1:6978`
  (`.claude/launch.json` has both configs).
- A ~40-line Electron main (Electron 43 from lite's node_modules, sandboxed
  renderer, no preload) loading `http://localhost:1420`.
- Result: the full desktop workspace UI renders and functions in the Electron
  window — project list, branch lanes with commits, worktree changes, fetch,
  logged-in user — all over but-server HTTP/WS.

Findings from the spike:

- **Route-list drift is real, today**: `irc_auto_join`,
  `irc_start_working_files_broadcast` (and the auto-fetch on a stale project
  surfaced `workspace_fetch_from_remotes` erroring) are invoked by the
  frontend but not routed in but-server → "Command not found" toasts. Confirms
  the recommendation to generate the command registry from the macro rather
  than maintain per-shell lists by hand.
- The stale-project error path, server capability gating, and error toasts all
  behaved correctly in Electron — the frontend genuinely doesn't care about
  the shell.
- Next spike increment: an `electron.ts` `IBackend` with a preload bridge for
  the web-mode stubs (`readFile`, disk store, relaunch), then swap transport
  to napi (Path B) behind the same interface.

## Verdict

Lite already paid down the hard, uncertain parts: napi-in-Electron transport,
watcher lifecycle, hardened packaging, askpass. What remains is
well-understood Electron plumbing plus release-infrastructure migration.
