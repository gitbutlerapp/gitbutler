# GitButler on GPUI

A native GitButler client built on [GPUI](https://gpui.rs), the UI framework behind Zed. It calls `but-api` in-process, so there is no Node runtime, IPC or JSON between the UI and GitButler. Its fucking fast.

## Run it

From the repository root, on macOS, Windows or Linux:

```bash
cargo build -p gitbutler-git --bins   # once per checkout: the askpass helper that push and fetch run
cargo run -p but-gpui
```

Each worktree builds into a `target` folder of its own, so a new one needs the helper too. `make run-but-gpui-without-xcode` builds it with the client.

On Linux, GPUI also links to libxcb, libxkbcommon and fontconfig: on Debian or Ubuntu, `sudo apt install libxcb1-dev libxkbcommon-x11-dev libfontconfig-dev`.

The project list comes from the app data of the build's channel, which is `dev` by default. Build with `CHANNEL=nightly` (or `release`) to see the projects of an installed GitButler.

## What it does

The app is built for watching what coding agents change. Reading their changes comes first, and committing by hand is there when you need it. It has:

- one navigator for every project, with its worktrees, stacks, branches and commits, kept current while agents work
- diffs of uncommitted changes, commits, branches and stacks
- committing, amending and moving commits, files, hunks and lines, by drag and drop or cut and paste
- the branches not applied, and pull requests with their checks
- conflict resolution for conflicted commits
- linked worktrees, and the checkouts on your other machines through the mesh
- Operations history, with undo and redo

## Development

- Started with `cargo run` on macOS or Linux, the status bar has a Build and restart button, which rebuilds the client and restarts it in place.
- `GITBUTLER_GPUI_APPEARANCE=dark` (or `light`) overrides the system appearance.
- `E2E_TEST_APP_DATA_DIR=<dir>` points GitButler's app data and settings at a sandbox.
- `GITBUTLER_API_URL` signs in to another GitButler server than production, as staging.
- `scripts/drive.sh run <file>` runs the app from a script of clicks, drags and keystrokes, saving screenshots, against app data of its own and in a window that never shows. `src/dev.rs` documents the commands.
