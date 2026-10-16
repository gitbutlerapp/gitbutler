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

The left side lists every project. Expanded projects show their uncommitted changes, stacks, branches and commits; collapsed ones only show counts for uncommitted files (amber), unpushed commits (blue) and commits the target has that you don't (amber). A file watcher keeps expanded projects current.

The right side shows whatever is selected:

- **Uncommitted changes.** Check files or single hunks, pick a branch and commit with ⌘↩.
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

Every change can be undone with ⌘Z. Right-click any row for the rest.

| Keys | |
|---|---|
| ↑ ↓ or j k | move through the tree |
| ← → | collapse or expand a project |
| ⌘F | filter projects |
| ⌘⇧N | new branch |
| ⌘⇧P | push the selected branch |
| ⌘⇧R | fetch |
| ⌘R | reload |
| ⌘O | add a project |
| ⌘⌫ | discard the selected commit or checked files, or delete the selected branch |

## How it's built

- `src/backend` is the only code that touches GitButler crates. It turns `but-api` results into plain structs (`model.rs`) and runs every call on a tokio pool with large stacks, since GitButler's graph walks and push futures overflow the small stacks of GPUI's worker threads in debug builds.
- `src/state.rs` holds the projects, selection and what's been cut, and runs one mutation at a time.
- `src/navigator.rs` and `src/detail.rs` are the two halves of the window; `src/transfer.rs` decides what a drop or paste does, so both input methods agree.
- `src/ui` has the small components GPUI doesn't ship: buttons, a text field and diff rows.

It is a Cargo workspace of its own. GPUI is a git dependency on Zed that needs Rust 1.98.1 and brings around 900 crates, so keeping it here leaves the main `Cargo.lock`, toolchain and CI untouched. Deleting this directory removes the app.

## Development

- `GITBUTLER_GPUI_APPEARANCE=dark` (or `light`) overrides the system appearance.
- `E2E_TEST_APP_DATA_DIR=<dir>` points GitButler's app data and settings at a sandbox.
- Build with `--features dev-script` and set `GITBUTLER_GPUI_SCRIPT=<file>` to replay clicks, drags and keystrokes and save screenshots of the window. `src/dev.rs` documents the commands.
