# GitButler on GPUI

An experimental native GitButler client built on [GPUI](https://gpui.rs), the UI framework behind Zed. It calls `but-api` in-process, so there is no Node runtime, IPC or JSON between the UI and GitButler.

## Run it

```bash
cd apps/gpui
cargo build -p gitbutler-git --bins   # once: the askpass helper that push and fetch run
cargo run
```

The project list comes from the app data of the build's channel, which is `dev` by default. Build with `CHANNEL=nightly` (or `release`) to see the projects of an installed GitButler.

## What you can do

The app is built for watching what coding agents change. Reviewing comes first, and committing by hand is there when you need it.

The left side lists every project. Expanded projects show their uncommitted changes, stacks, branches and commits; collapsed ones only show counts for uncommitted files (amber), unpushed commits (blue) and commits the target has that you don't (amber). A file watcher keeps expanded projects current. A green "active" dot marks a project whose files or commits changed in the last two minutes, and a teal dot marks commits you haven't opened yet. Commits the app itself rewrote don't count as new.

The right side shows whatever is selected:

- **Uncommitted changes.** Every changed file as a diff, each marked "Not reviewed", "Reviewed" or "Needs review" (changed after you reviewed it). ⌘Y marks the file under the cursor reviewed, folds it and moves to the next file still to review. ↑ ↓ step through hunks and ⌘⌥Z throws one away, with an Undo button in case you didn't mean it. The navigator shows how many files are left to review. ⌘↩ or "Commit…" opens the composer, where you check files or single hunks and pick a branch.
- **A commit.** Its message, files and diff. Edit the message, uncommit it or discard it.
- **A branch.** Its commits and combined diff, and Push.
- **A project.** Counts, Fetch, and Update to rebase every stack onto the latest target.

Moving things works by drag and drop or by ⌘X on the source and ⌘V on the destination:

| Move | onto | does |
|---|---|---|
| a commit | a branch | moves it to the top of that branch |
| a commit | another commit | moves it above that commit; hold ⌥ to squash instead |
| a commit | uncommitted changes | uncommits it |
| uncommitted files | a commit | amends that commit |
| uncommitted files | a branch | picks that branch and those files for the next commit |
| a commit's file | another commit | moves the change between commits |
| a commit's file | uncommitted changes | uncommits just that file |

Every change can be undone with ⌘Z. Right-click any row for the rest, or press ⌘K for the command palette, which lists every command that applies where you are, with its keys. The status bar shows the few keys that matter for what has focus.

| Anywhere | |
|---|---|
| ⌘K | command palette (⌘⇧P works too) |
| ⌘1, ⌘2 | focus the navigator, focus the changes |
| ⇧Esc | zoom the diff to the whole window |
| . | show or hide the navigator |
| ⌘↩ | commit… |
| ⌘Z, ⇧⌘Z | undo, redo |
| ⌘X, ⌘V | cut the selection, paste it onto the selected row |
| ⌘N | new branch |
| ⌥⇧F, ⌥⇧R | fetch, update from the target |
| ⌘F, ⌘R, ⌘O | filter projects, reload, add a project |

| In the navigator | |
|---|---|
| ↑ ↓ or j k | move through the tree |
| ← → or h l | collapse or expand |
| r, b, ⇧P | rename, new branch on top, push |
| ⌘⌫ | discard the selected commit or delete the selected branch |
| ⌥⌘⌫ | uncommit the selected commit |

| In the changes | |
|---|---|
| ↑ ↓ | previous or next hunk |
| j k | next or previous file |
| ⌘Y | mark the file reviewed and go to the next one |
| z | fold or unfold the file |
| ⌥⌘Z, ⌘⌫ | discard the hunk, discard the file |

## How it's built

- `src/backend` is the only code that touches GitButler crates. It turns `but-api` results into plain structs (`model.rs`) and runs every call on a tokio pool with large stacks, since GitButler's graph walks and push futures overflow the small stacks of GPUI's worker threads in debug builds.
- `src/state.rs` holds the projects, selection, what's been cut and what you've reviewed or opened, and runs one mutation at a time. Review and read state is saved to `gpui-state.json` in the app config directory.
- `src/navigator.rs` and `src/detail/` are the two halves of the window; `src/transfer.rs` decides what a drop or paste does, so both input methods agree.
- `src/palette.rs` is the command palette and `src/status_bar.rs` the strip along the bottom.
- `src/ui` has the small components GPUI doesn't ship: buttons, a text field and diff rows.

It is a Cargo workspace of its own. GPUI is a git dependency on Zed that needs Rust 1.98.1 and brings around 900 crates, so keeping it here leaves the main `Cargo.lock`, toolchain and CI untouched. Deleting this directory removes the app.

## Development

- `GITBUTLER_GPUI_APPEARANCE=dark` (or `light`) overrides the system appearance.
- `E2E_TEST_APP_DATA_DIR=<dir>` points GitButler's app data and settings at a sandbox.
- Build with `--features dev-script` and set `GITBUTLER_GPUI_SCRIPT=<file>` to replay clicks, drags and keystrokes and save screenshots of the window. `src/dev.rs` documents the commands.
