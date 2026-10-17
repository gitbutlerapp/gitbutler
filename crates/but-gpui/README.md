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

The left side lists every project. Expanded projects show their uncommitted changes, stacks, branches and commits, and their row counts uncommitted files (amber), unpushed commits (blue) and commits the target has that you don't (amber). A file watcher keeps expanded projects current; collapsed ones aren't loaded at startup or watched. A green "active" dot marks a project whose files or commits changed in the last two minutes, and a teal dot marks commits you haven't opened yet. Commits the app itself rewrote don't count as new.

The right side shows whatever is selected, under a breadcrumb whose project and branch link back to them. Drag the right edge of the navigator or of a file list to resize it, and double-click the edge to reset it; the widths are remembered.

- **Uncommitted changes.** Every changed file as a diff, each marked "Not reviewed", "Reviewed" or "Needs review" (changed after you reviewed it). ⌘Y marks the file under the cursor reviewed, folds it and moves to the next file still to review. ↑ ↓ step through hunks, space and shift-space page through the diff, and ⌘⌥Z throws a hunk away, with an Undo button in case you didn't mean it. The navigator shows how many files are left to review. ⌘↩ or "Commit…" opens the composer, where you check files or single hunks and pick a branch. Click a changed line's numbers to take just that line out, or, in a hunk that's out, to bring just that line in. The sparkle in the message box writes the message with AI from the checked changes, streaming it in; anything already in the box guides it. A file a merge left with conflict markers, as an update can, shows as conflicted, counted on Uncommitted changes in the navigator, and starts unchecked in the composer, as a commit refuses it until its menu marks it resolved, taking the file as it is on disk; discarding it takes it back to its last commit.

  A changed image (PNG, JPEG, GIF, WebP, BMP or TIFF) shows before and after, side by side.

  The button beside a changed file's line counts shows the unchanged lines around its hunks, so the whole file reads top to bottom; press it again to hide them.

  Diffs mark the words that changed within a line, and each hunk is titled with the function or type it sits in (`impl Counter › pub fn bump(&mut self, by: i32) -> i32`) rather than its `@@` line numbers. Both come from [deltoids](https://github.com/juanibiapina/deltoids), which parses the two sides with tree-sitter.
- **A commit.** Its title, with the rest of the message behind the button beside it, its files and its diff. Edit the message, uncommit it or discard it. Its right-click menu also adds an empty commit above or below, starts a new branch ending at it, moves it into a new branch of its own, rewraps a message with lines too long for `git log`, and copies its ID, change ID or message. ⌥↑ and ⌥↓ move it one step, into the next branch of its stack at either end. ⌘-click picks more commits along with it and ⇧-click picks a run of them; the picks' right-click menu squashes them into the oldest, or into the one right-clicked, in its place, or uncommits them all.
- **A conflicted commit.** It says so above its files, and Resolve… (also on its right-click menu) lists every conflict with the current side, what the commit now sits on, next to the incoming side, the commit's own change. Accept current, incoming or both for each, or all at once, or leave the rest to AI; conflicts left unpicked stay for later. Resolving rewrites the commit and can be undone. Edit by hand (in the resolver or the commit's menu) checks the commit out in the working directory instead, conflict markers and all, for any editor or agent to resolve; the project page then lists its files as conflicted or resolved beside their diff, and Save rewrites the commit and moves the commits above it onto the result. Right-click a file there to open it, or to mark it resolved when the lines that look like markers belong in it. Until the edit is saved or stopped, the app turns down other operations on the project, apart from fetching. A branch with conflicted commits says how many on its page, and its Resolve button starts with the oldest in its stack, which can be in a branch below, since resolving it rebases the ones above.
- **A branch.** Its commits and combined diff, and Push. Double-click it, or press ←, to hide its commits behind a count, and again, or →, to show them; clicking a stack's caption does that for all its branches. When its remote has commits it doesn't, as when a teammate or an agent elsewhere pushed, its page lists them and Pull brings them in with its own commits on top, asking first when that would leave its commits conflicted; its menu can also take the remote's version instead. A branch with others stacked on it can't pull yet. Its right-click menu adds a branch on top or below, an empty commit on top, squashes all its commits into one, takes it out of its stack, updates just its stack from the target, or unapplies it. The bottom branch of a stack can land on the target, its commits going straight onto it and pushed, skipping review; from the top branch the whole stack lands. Both ask first, as a push can't be undone.
- **A branch's pull request.** A card on the branch's page: its title, number, state, labels, how its CI checks stand (a failed one by name, opening why it failed), and who's been asked to review it, with a button to open it.
- **A project.** Where it is, when it last fetched (and why a fetch failed), a nudge to connect an account when it's on a forge none is connected for, and its target's history: the commits new on the target first, then the ones your workspace already has, each with the pull request that landed it, paging further back on request. Select one to see its diff. Fetch and Update (rebase every stack onto the target's new commits) are in the header. Update first works out whether rebasing would conflict any commits or uncommitted changes, and asks before going ahead when it would.
- **Branches.** Next to History at the end of each project: the branches the workspace doesn't have applied, local ones first, then the ones only a remote has, like a teammate's, with a filter by name or author. Each shows what it changes against the target, as its pull request would, and Apply adds it to the workspace (a remote branch gets a local branch first). Local branches can be deleted. The selected branch lists its commits under it: select one to see it on its own, then drag it onto a branch or commit in the workspace, or press Cherry-pick (also in its right-click menu) and ⌘V there, to copy just that commit in.
- **Linked worktrees.** With Linked worktrees on in Settings (GitButler's experimental worktree support), each worktree of a project, like one an agent works in, gets a row above Branches: its name, the branch it has checked out, how many files it changed, and a green dot while its files changed in the last two minutes, as an agent at work does (the project counts as active then too). Select it to watch its uncommitted changes as they're written, reviewed file by file like your own (⌘Y, Mark all reviewed, and the row counts what's left); e opens a file from the worktree in your editor. Its branch's name links to what it has committed, on the Branches page, where a branch a worktree has checked out offers Open worktree instead of Apply or Delete. Its menu reveals or copies its folder, or archives it to hide it. Worktrees that exist when the setting goes on start archived, and Show an archived worktree… in the project's menu brings one back.
- **History.** The last row of each project lists what GitButler did to it, newest first, including what agents ran from the CLI. Select an operation to see the files it changed, then restore the project to how it was before it; the restore can be undone too. "Take snapshot" records the project as it is now, to come back to later.

Moving things works by drag and drop or by ⌘X on the source and ⌘V on the destination:

| Move | onto | does |
|---|---|---|
| a commit | a branch | moves it to the top of that branch |
| a commit | another commit | moves it above that commit; hold ⌥ to squash instead, ⇧ to put it below |
| a commit | uncommitted changes | uncommits it |
| a commit | New branch, below the stacks while dragging | moves it into a new branch of its own (also in its menu) |
| uncommitted files | a commit | amends that commit |
| uncommitted files | a branch | picks that branch and those files for the next commit |
| a commit's file | another commit | moves the change between commits |
| a commit's file | uncommitted changes | uncommits just that file |
| a branch | another branch | stacks it, with the branches above it, on top of that one |
| a hunk of uncommitted changes | a commit or branch | as its file would, with just that hunk |
| a hunk of a commit | another commit, or uncommitted changes | moves just that hunk, or uncommits it |

Drag a hunk by its header in the diff. Right-click a hunk or file header there for the same moves as a menu, plus, in a commit, Split into a new commit (the hunk moves into a new commit right above, in one undoable step) and Discard from commit.

"Cherry-pick" in a commit's menu picks it up the same way, and ⌘V puts a copy on top of a branch or above a commit, leaving the original where it is.

Settings (⌘,) has a search box that finds a setting in any section. General picks light, dark or the system appearance (the palette also switches between light and dark), how large code shows in diffs, the editor files open in and the terminal; Git sets how often expanded projects fetch in the background; AI picks the provider (OpenAI, Anthropic, Ollama or LM Studio), GitButler's key or your own, and the model; Accounts signs in to GitButler and adds GitHub (in the browser, or a token), GitLab and Bitbucket accounts. All three are shared with GitButler's other apps. Project renames the project, changes its target branch (with no branches applied), protects force pushes (a push that would drop commits someone else pushed stops and offers to push anyway), turns commit signing on and tests it, and removes the project from GitButler. Press e in the diff to open the file under the cursor in your editor at its hunk; the first time, a menu asks which editor. A project's menu opens it in the editor or the terminal.

On a project hosted on a forge, a branch without a pull request offers Create pull request in its header and menu: a title and description start from the branch's only commit, or the repository's template, and the branch is pushed first if it needs it. Write with AI fills the title and description from its commits. A branch with an open pull request adds Merge (merge, squash or rebase, chosen in the dialog). Click a branch's CI glyph to check again.

Branch rows show a pull request's state (open, draft, merged or closed) and number, and a glyph for its checks. The app reads these from GitButler's forge cache, which it refreshes in the background every minute and after each push, using the GitHub account GitButler already has.

⌘F finds text in the diff, marking every match and stepping through them with Enter and ⇧Enter, unfolding a folded file to reach one. / (or the magnifier above the file list) filters the file list and the diff to paths with every word typed in them; Escape clears it. Long lines run off the edge unless wrapping is on (⌥Z, or Settings). ⌘B shows diffs side by side, the old lines beside the new, and back.

While a file scrolls, its header stays at the top of the diff until the next file pushes it off. Lists and diffs show a scrollbar while the pointer is over them; drag its thumb, or press the track to jump. Text in a diff selects like text anywhere: drag across lines, double-click a word, triple-click a line, shift-click to extend, and ⌘C copies it without the `+` and `-`. File lists can show a tree of folders instead of a flat list, with the button beside the file count; folders holding nothing but one folder merge into one row (`src/backend`). Right-click a folder to cut, mark reviewed, discard or uncommit everything under it.

While something is cut or dragged, every row it could land on says what landing there would do ("Amend", "Move above", "Squash into" with ⌥ held, "Move below" with ⇧), and the rest step back. A commit selected in the app stays selected when an agent rewords or rebases it, as long as it keeps its change ID.

Every change can be undone with ⌘Z, and the snackbar names what was undone, including operations an agent ran from the CLI. Changes show at once: what an operation takes away leaves the lists, and a new commit appears dimmed on its branch, until the reload after it shows the real result. Another operation asked for meanwhile waits its turn; the status bar names the one running, how long it's taking once that's noticeable, and how many wait. Quitting before they finish asks first. ⌘C copies the selection: a commit's id, a branch's name, or the path of the file under the diff cursor. Right-click any row for the rest, or press ⌘K for the command palette, which lists every command that applies where you are, with its keys. The status bar shows the few keys that matter for what has focus.

With no projects yet, the window says what the app is for and offers to add or clone a repository. File → Clone Repository… asks for a URL and a folder, clones it there and adds it. A project whose folder moved stays listed and says so, with Locate… to point it at the new place, or Remove. A project with no target branch offers to pick one from its remote branches.

The Help menu links the docs, the release notes and where to report an issue, shows the folder GitButler keeps its settings in, and opens the command palette as the list of shortcuts. When a commit or amend has to leave changes out, the error says why for each, in plain words.

| Anywhere | |
|---|---|
| ⌘K | command palette (⌘⇧P works too) |
| ⌘P | go to a branch by name, in any project |
| ⌘1, ⌘2 | focus the navigator, focus the changes |
| ⇧Esc | zoom the diff to the whole window |
| . | show or hide the navigator |
| ⌘↩ | commit… |
| ⌥⌘↩ | while committing: amend the newest commit of the target branch instead |
| ⌘Z, ⇧⌘Z | undo, redo |
| ⌘X, ⌘V | cut the selection, paste it onto the selected row (⌥⌘V squashes a commit instead of moving it, ⇧⌘V puts it below) |
| ⌘N | new branch |
| ⌥⇧F, ⌥⇧R | fetch, update from the target |
| ⌘R, ⌘O | reload, add a project |
| ⌘, | settings |

| In the navigator | |
|---|---|
| ↑ ↓ or j k | move through the tree |
| ⇧↑ ⇧↓ | previous or next branch or project |
| Home End, gg ⇧G | first or last row |
| ← → or h l | collapse or expand: a project, or a branch's commits; ← on a commit goes to its branch |
| r, b, ⇧P | rename, new branch on top, push |
| o | open the pull request |
| ⌘⌫ | discard the selected commit or delete the selected branch |
| ⌥⌘⌫ | uncommit the selected commit |
| ⌥↑ ⌥↓ | move the selected commit up or down |

| In the changes | |
|---|---|
| ↑ ↓ | previous or next hunk |
| j k | next or previous file |
| ⌘Y | mark the file reviewed and go to the next one |
| z | fold or unfold the file |
| e | open the file in your editor, at the hunk |
| ⌥⌘Z, ⌘⌫ | discard the hunk, discard the file |
| ⌘F, ⌘G, ⇧⌘G | find in the diff, next match, previous match |
| ⌥Z | wrap long lines, or stop |
| ⌘B | side by side, or one above the other |
| f | hide or show the file list beside the diff |
| / | filter the files by path |

## How it's built

- `src/backend` is the only code that touches GitButler crates, and `src/backend/annotate.rs` the only code that touches deltoids. It turns `but-api` results into plain structs (`model.rs`) and diffs into hunks of lines (`diff.rs`), and runs every call on a tokio pool with large stacks, since GitButler's graph walks and push futures overflow the small stacks of GPUI's worker threads in debug builds.
- `src/state` holds the projects, selection, what's been cut and what you've reviewed or opened. `load.rs` loads and watches workspaces, `background.rs` fetches and asks the forge on its own, and `seen.rs` tracks what you've looked at; `mutation.rs` runs one mutation at a time and queues the rest; `saved.rs` keeps review and read state in `gpui-state.json` in the app config directory.
- `src/navigator.rs` (its menus in `navigator/menu.rs`) and `src/detail/` (its file lists in `file_list.rs`) are the two halves of the window; `src/transfer.rs` decides what a drop or paste does, so both input methods agree.
- `src/edits.rs`, `src/pr.rs`, `src/resolve.rs` and `src/projects.rs` hold the actions on commits and branches, pull requests, conflicts and projects; `src/settings/` is the Settings window.
- `src/palette.rs` is the command palette, `src/overlays.rs` the menus, prompts, modals and snackbars above the window, and `src/status_bar.rs` the strip along the bottom.
- `src/ui` has the small components GPUI doesn't ship: buttons, a text field, a scrollbar and diff rows.

It is a member of the main workspace, so `cargo check --workspace` catches `but-api` changes that break it. GPUI is a git dependency on Zed that brings around 900 crates and needs Rust 1.98. Only macOS builds the client. Elsewhere GPUI needs system libraries the workspace's Linux and Windows CI don't install, so `main` is a stub there and GPUI isn't compiled. Nothing else in the workspace depends on this crate.

## Development

- `GITBUTLER_GPUI_APPEARANCE=dark` (or `light`) overrides the system appearance.
- `E2E_TEST_APP_DATA_DIR=<dir>` points GitButler's app data and settings at a sandbox.
- Build with `cargo build -p but-gpui --features dev-script` and set `GITBUTLER_GPUI_SCRIPT=<file>` to replay clicks, drags and keystrokes and save screenshots of the window. `src/dev.rs` documents the commands.
