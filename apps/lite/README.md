# GitButler Lite

GitButler Lite is an Electron desktop client backed by GitButler's native Rust SDK.

## Development

Run commands from the repository root:

```console
$ pnpm dev:lite
```

This starts the Vite renderer on port 5173 and Electron with the Chrome DevTools Protocol on port 9222. Renderer changes use hot module replacement; Electron or native SDK changes require a restart.

Useful commands:

```console
$ pnpm -F @gitbutler/lite check
$ pnpm -F @gitbutler/lite test
$ pnpm -F @gitbutler/lite test:e2e
$ pnpm -F @gitbutler/lite demos
```

## Structure

- `ui/` — React renderer, routes, and application state
- `electron/` — Electron main process, preload, and IPC boundary
- `e2e/` — Playwright tests and repository fixtures

## Frontend

The renderer is TypeScript and React 19, built with Vite and the React Compiler. TanStack Router owns navigation, TanStack Query owns server/native data, and Redux Toolkit owns local application state.

## Process boundary

The renderer has no direct Node.js or native module access. Privileged operations are exposed as the typed `window.lite` API through Electron's context-isolated preload.

SDK endpoints are derived in `electron/src/ipc.ts` from `@gitbutler/but-sdk`; Electron-owned capabilities are declared there explicitly. The main process loads the native SDK and handles the IPC calls.

After changing Rust bindings or generated SDK endpoints, rebuild the SDK and fully restart Lite:

```console
$ pnpm -F @gitbutler/but-sdk build
$ pnpm dev:lite
```

The development and bundle scripts build and copy the Git askpass helper automatically.
