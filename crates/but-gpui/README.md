# GitButler on GPUI

An experimental native GitButler client built on [GPUI](https://gpui.rs), the UI framework behind Zed. It calls `but-api` in-process, so there is no Node runtime, IPC or JSON between the UI and GitButler.

## Run it

From the repository root, on macOS:

```bash
cargo build -p gitbutler-git --bins   # once: the askpass helper that push and fetch run
cargo run -p but-gpui
```

The project list comes from the app data of the build's channel, which is `dev` by default. Build with `CHANNEL=nightly` (or `release`) to see the projects of an installed GitButler.

## What you can do

The app is built for watching what coding agents change. Reviewing comes first, and committing by hand is there when you need it.

The left side lists every project. Expanded projects show their uncommitted changes, stacks, branches and commits; collapsed ones only show counts for uncommitted files (amber), unpushed commits (blue) and commits the target has that you don't (amber). A file watcher keeps expanded projects current. A green "active" dot marks a project whose files or commits changed in the last two minutes, and a teal dot marks commits you haven't opened yet. Commits the app itself rewrote don't count as new.

The right side shows whatever is selected, under a breadcrumb whose project and branch link back to them. Drag the right edge of the navigator or of a file list to resize it, and double-click the edge to reset it; the widths are remembered.

- **Uncommitted changes.** Every changed file as a diff, each marked "Not reviewed", "Reviewed" or "Needs review" (changed after you reviewed it). ⌘Y marks the file under the cursor reviewed, folds it and moves to the next file still to review. ↑ ↓ step through hunks and ⌘⌥Z throws one away, with an Undo button in case you didn't mean it. The navigator shows how many files are left to review. ⌘↩ or "Commit…" opens the composer, where you check files or single hunks and pick a branch.

  Diffs mark the words that changed within a line, and each hunk is titled with the function or type it sits in (`impl Counter › pub fn bump(&mut self, by: i32) -> i32`) rather than its `@@` line numbers. Both come from [deltoids](https://github.com/juanibiapina/deltoids), which parses the two sides with tree-sitter.
- **A commit.** Its title, with the rest of the message behind the button beside it, its files and its diff. Edit the message, uncommit it or discard it.
- **A branch.** Its commits and combined diff, Push, and its pull request: title, number, state and how its CI checks stand, with a button to open it.
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

Branch rows show a pull request's state (open, draft, merged or closed) and number, and a glyph for its checks. The app reads these from GitButler's forge cache, which it refreshes in the background every minute and after each push, using the GitHub account GitButler already has.

Text in a diff selects like text anywhere: drag across lines, double-click a word, triple-click a line, shift-click to extend, and ⌘C copies it without the `+` and `-`. File lists can show a tree of folders instead of a flat list, with the button beside the file count; folders holding nothing but one folder merge into one row (`src/backend`).

While something is cut or dragged, every row it could land on says what landing there would do ("Amend", "Move above", "Squash into" with ⌥ held), and the rest step back. A commit selected in the app stays selected when an agent rewords or rebases it, as long as it keeps its change ID.

Every change can be undone with ⌘Z, and the snackbar names what was undone, including operations an agent ran from the CLI. ⌘C copies the selection: a commit's id, a branch's name, or the path of the file under the diff cursor. Right-click any row for the rest, or press ⌘K for the command palette, which lists every command that applies where you are, with its keys. The status bar shows the few keys that matter for what has focus.

| Anywhere | |
|---|---|
| ⌘K | command palette (⌘⇧P works too) |
| ⌘1, ⌘2 | focus the navigator, focus the changes |
| ⇧Esc | zoom the diff to the whole window |
| . | show or hide the navigator |
| ⌘↩ | commit… |
| ⌘Z, ⇧⌘Z | undo, redo |
| ⌘X, ⌘V | cut the selection, paste it onto the selected row (⌥⌘V squashes a commit instead of moving it) |
| ⌘N | new branch |
| ⌥⇧F, ⌥⇧R | fetch, update from the target |
| ⌘F, ⌘R, ⌘O | filter projects, reload, add a project |

| In the navigator | |
|---|---|
| ↑ ↓ or j k | move through the tree |
| ← → or h l | collapse or expand |
| r, b, ⇧P | rename, new branch on top, push |
| o | open the pull request |
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

- `src/backend` is the only code that touches GitButler crates, and `src/backend/annotate.rs` the only code that touches deltoids. It turns `but-api` results into plain structs (`model.rs`) and runs every call on a tokio pool with large stacks, since GitButler's graph walks and push futures overflow the small stacks of GPUI's worker threads in debug builds.
- `src/state.rs` holds the projects, selection, what's been cut and what you've reviewed or opened, and runs one mutation at a time. Review and read state is saved to `gpui-state.json` in the app config directory.
- `src/navigator.rs` and `src/detail/` are the two halves of the window; `src/transfer.rs` decides what a drop or paste does, so both input methods agree.
- `src/palette.rs` is the command palette and `src/status_bar.rs` the strip along the bottom.
- `src/ui` has the small components GPUI doesn't ship: buttons, a text field and diff rows.

It is a member of the main workspace, so `cargo check --workspace` catches `but-api` changes that break it. GPUI is a git dependency on Zed that brings around 900 crates and needs Rust 1.98. Only macOS builds the client. Elsewhere GPUI needs system libraries the workspace's Linux and Windows CI don't install, so `main` is a stub there and GPUI isn't compiled. Nothing else in the workspace depends on this crate.

## Development

- `GITBUTLER_GPUI_APPEARANCE=dark` (or `light`) overrides the system appearance.
- `E2E_TEST_APP_DATA_DIR=<dir>` points GitButler's app data and settings at a sandbox.
- Build with `cargo build -p but-gpui --features dev-script` and set `GITBUTLER_GPUI_SCRIPT=<file>` to replay clicks, drags and keystrokes and save screenshots of the window. `src/dev.rs` documents the commands.
