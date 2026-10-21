# GPUI client instructions

These apply to `crates/but-gpui`, on top of `crates/AGENTS.md`. The README is the feature doc,
so update it, shortcuts table included, with every change in behaviour.

## Fonts

- Use only the font families the app embeds (`assets/fonts`, `theme.ui_font` and
  `theme.mono_font`). GPUI looks a family up among the embedded fonts first and falls back to
  the system only when it isn't there. A family that isn't embedded then resolves to whatever
  the machine has installed. If that's a variable font, like Inter in `~/Library/Fonts`,
  CoreText calls the font daemon for every line it lays out, and frames go from about 10ms to
  70-120ms, only on that machine.
- GPUI picks a weight by choosing between font files and doesn't vary a variable font, so add
  static instances, never a variable file. `assets/fonts/README.md` has the recipe.
- The files are cut 60 lighter than the design-core weights they stand in for, because GPUI
  draws thicker stems than Chrome. Compare against but.dev before changing a weight: render both
  at the same scale (headless Chrome with `--force-device-scale-factor=2` and a harness `shot`)
  and compare stem thickness, not screenshots taken at different scales.

## Measuring performance

- Measure release builds with `--features dev-script`. `GITBUTLER_GPUI_SCRIPT` replays input,
  and `perf <label> <input>` times the input until loads settle and prints `stall`: the longest
  the UI thread was busy meanwhile, frames included. Nothing draws while it's busy, so keep it
  under 8.3ms for 120Hz. The harness draws a frame right after the input and again once loads
  land, running `on_next_frame` callbacks first as a display link would.
- macOS naps an app whose window isn't visible. After about 30 seconds its timers coalesce and
  its threads lose priority, so every step of a long scripted run slows down by 2-3x, though
  nothing in the app changed. The dev script opts out with `NSProcessInfo` (see `src/dev.rs`).
  Keep it in any harness that drives the app off screen, and distrust a slowdown that starts
  partway through a run.
- The harness window is off screen, so macOS draws it only when the script asks. Anything that
  needs a second frame shows in the next step's screenshot, not this one's.
- Profile with `sample <pid> <seconds> -file out.txt`. Look at the main thread: a scripted run
  idles between steps, so most samples there are idle.
- A dev-script build idles at 6-10% CPU, because the stall watcher wakes the UI thread every
  millisecond. Measure idle CPU and memory on the plain build.
- Stall numbers swing by 30-50% with machine load. Check `uptime`, run the old and new binaries
  in turns, and compare means before calling a change a regression.
- `perf startup` counts from launch, so make it the first step. A `wait` before it adds to it.

## Frame budget

- GPUI draws every view again, caches or not, when focus moves and when input switches between
  mouse and keyboard. Those frames cost the sum of the views, so each view's own cost matters
  even when it caches.
- A view observing another redraws on its every notify. Notify only what changed: a change
  only the diff shows shouldn't go through `Store::set_prefs`, which redraws the whole window.
- A cached view that misses its cache renders all its descendants with caching off, so a
  cached child inside a view that notifies often saves nothing.
- Text shaping dominates frames with new content: GPUI's CoreText layout creates a sized font
  copy per text run, so fewer text elements per row is the lever, and a new diff draws a frame
  after the rest of the detail (`DiffPane`) so the two don't share one frame.
- Put any list that can grow in `uniform_list` or `list`. A thousand plain rows cost 36ms a
  frame, against 1.6ms in `uniform_list`. Don't derive per-row state for every row each frame:
  a 20,000-line diff stalled until the line grip was worked out only for the row under the
  pointer.

## GPUI traps

- A context menu's handlers run inside `AppView`'s update, so anything that updates `AppView`
  again from one panics with "already being updated". Wait with `window.defer`, as the menus in
  `detail/branches.rs` do. A store `done` callback runs mid-update too and mustn't read the store.
- `window.context_stack()` panics during render. Read key contexts in an event handler.
- A binding that a text field could also take needs `&& !TextInput` in its context, or typing
  the letter runs it (P once pushed). Bind shortcuts shared by both halves to
  `(Navigator || Detail)` rather than `Workspace`, which modals sit inside. `DiffPane` is a
  sibling of `Detail`, so its find field has its own context.
- GPUI focuses the nearest focusable parent on mouse-down, so clicking a tab inside `Detail`
  takes the keys from a list page. Hand focus back, and never to an element that isn't drawn;
  if a focused one goes anyway, keys reach nothing, so `on_focus_lost` sends them to the navigator.
- A drag's payload stays as the drag began. Drop targets read `Store::dragging`, which reloads
  follow as they follow the cut.
- A cached view redraws only on its own notify. `window.focus` doesn't notify it, and a drag let
  go over a target that refuses it ends without any redraw, so the navigator notifies on both.
- GPUI calls an element unhovered once the pointer moves with the button down on it, and starts
  a drag only past 2px. A drag handler attached only while hovered, as the line grip's is, is
  gone by then unless the press keeps it (`grip_held`).
- After a key press GPUI counts the pointer as over nothing until it moves, so a drop right after
  Escape is skipped. Escape during a drag calls `cx.stop_active_drag`, and a focused text field
  passes Escape on.
- `list` clips to its own bounds, not to each row, so a line drawn on a row boundary is painted
  over by the next row. Wrap it in `deferred()`.
- `svg()` draws in one colour and doesn't inherit the text colour, so set one. `img` keeps an
  SVG's colours but loads asynchronously, and an image that arrives late never repaints a
  cached view. Release image textures with `cx.drop_image`.
- Taffy stretches only children whose height is `auto`. In a row with only a min-height,
  `h_full()` is 0, so use `self_stretch()`. Flex text needs `min_w(px(0.))` to shrink or wrap.
  `flex_1` has a zero basis and lets a name shrink to nothing, so use `flex_grow(1.)` and give
  lesser items `flex_shrink(1000.)`. Margins collapse through a card's background, so use padding.
- A `cx.notify()` during prepaint is dropped, so defer it. `window.refresh()` does nothing during
  paint, so use `on_next_frame`. An `invisible()` element has no hitbox, so use opacity 0.
- `AppView` draws `DiffPane` absolutely over the detail pane, at `HEADER_HEIGHT` plus
  `Detail::view_header_height`. Anything that changes the height above the file list, like tabs
  or a banner, must be counted there, or the diff covers it and takes its clicks. Side-column
  content wider than the column slides under the diff the same way.

## GitButler semantics

- Only `src/backend` touches GitButler crates. `backend::run` starts its work when called, not
  when awaited, and blocking work can't be cancelled, so debounce before calling it. A panic in
  it becomes a "GitButler task failed" error. A panic on the UI thread crashes the app.
- Key UI state by identity, never by index, sha or byte offset. Commits go by change ID
  (`Commit::change_id()` falls back to the id the engine writes on a rebase), and hunks by their
  old-side range, since an agent's edit above shifts every index.
- An amend keeps the change ID and the parent and changes the tree. A rebase or a reword doesn't
  count as one. `git rebase` drops the change ID, so what's been read also goes by
  `Commit::authored`, a hash of the author, time and message, which every rebase keeps.
- A menu, prompt, picker, form or drag holds the ref name or commit id it opened on, which an
  agent can rename or rewrite before it's used. Follow it (`follow_by_key`, `Remap`,
  `StoreEvent::Renamed`), or close or refuse once it's gone; never act on the stale one.
- A confirmation that can't be undone (a linked worktree's discard or delete, Land) carries what it
  asked about, as a file's `modified` time or a branch's tip, and refuses if that moved meanwhile.
- Hunk and line selections in uncommitted files are positional, and an agent's edit slides them.
  Commit, amend and discard compare each file's `modified` time and base with the diff the user
  saw and refuse if either moved. Core takes any selection in an added file as the whole file,
  so the UI refuses partial picks there.
- Force-push protection is off by default, so a forced push is a plain `--force`. Every forced
  push except Push anyway fetches first and compares the remote tip and incoming commit ids,
  because counts, titles and change IDs all missed a teammate's amend. Update anyway and Pull
  anyway run the dry run again and go ahead only on the conflicts the user accepted.
- The oplog records the workspace and the main checkout only. Undo and History restore refuse
  anything that touched a linked worktree, in older snapshots too. Commits in a linked worktree
  skip hooks, which would run against the main checkout's files. Branches move only within their
  own side.
- A snapshot taken with one branch checked out outside the workspace keeps no stacks: only the
  branch's ref and tip under `checkout/`, and the refs below it under `additional-refs/`. History
  reads those branches' commits from the repository, down to the snapshot's target.
- PR and CI data come from the forge cache the workspace load already reads, and nothing calls
  the network to render. CI checks refresh only for applied branches, so cached checks on an
  unapplied branch are stale.

## Mesh

The mesh shows the user's other machines' work. It's optional, and it must never break or slow
anything else.

- Its code stays in five files: `backend/mesh.rs` runs the `but-mesh` CLI and reads its JSON,
  `state/mesh.rs` holds `Mesh` with its poll, sign-in and apply, `navigator/mesh.rs` draws
  each card's checkouts elsewhere and folds them, `detail/machine.rs` fetches and shows what's
  selected there, and `settings/mesh.rs` is the Mesh row in Settings. Everything else reaches
  it through a `Selection::Mesh` or `Row::MeshHeading` arm, `store.mesh`, the `Store::mesh_*`
  methods or `StoreEvent::MeshRead`. New mesh behaviour goes in those files, with at most a
  dispatch arm elsewhere.
- There's no mesh library: the mesh lives in a private repository, so the app only calls the
  CLI. Without `but-mesh`, or signed out, nothing shows but the Settings row, and nothing runs
  but one status check a minute. Gate anything new on `store.mesh.connection()`.
- Every mesh call goes through `backend::run` with one of the timeouts in `backend/mesh.rs`,
  never on the UI thread. The poll and the branch fetches make one call at a time.
- Network work never runs in a project's operation queue, where it would hold up commits,
  pushes and updates. `mesh_apply` fetches first, outside it, and applies with a read's timeout.
- Background failures (the poll, reporting a project, the branch fetches) log at debug and keep
  the last good read, and are tried again: a report on the next read, a fetch when its page is
  selected again. A failed status check while connected keeps the connection, so a network blip
  doesn't hide the rows. Only what the user asked for, signing in or out and Apply here, shows an
  error.
- The store notifies only when a read changed something, so the mesh's types keep only what's
  drawn. Its rows are the navigator's, in its `list`, so another machine's long branch costs
  nothing to draw; a checkout's view beside the diff lists at most `LISTED` uncommitted files.
- Try failures with a fake `but-mesh` first on the `PATH`, which the app searches before
  `~/.local/bin`: one that sleeps tests the timeouts and that the project's queue stays free,
  and one that exits non-zero or prints garbage tests the fallbacks. A sandbox keeps the app's
  data apart but not the mesh's: with the real `but-mesh` signed in, a sandbox run reports its
  repositories to your mesh.

## Scripted runs

- The plain and dev-script builds write the same `target/release/gitbutler-gpui`. The plain one
  ignores `GITBUTLER_GPUI_SCRIPT`, and the run hangs. Check with
  `strings target/release/gitbutler-gpui | grep -c GITBUTLER_GPUI_SCRIPT`, and rebuild the plain
  one with `cargo build --release -p but-gpui -p gitbutler-git --bins` when done, so the askpass
  helper is there for pushes.
- The script waits for loads to settle without a timeout, quit waits for running operations, and
  macOS has no `timeout`, so guard long runs with a kill of your own. Kill only the process you
  started.
- Each step settles for 120ms, so a delay shorter than that never shows. Stretch the constant to
  check one. A hover registers only after a second small `move`.
- `drop` moves the pointer before letting go, and that move redraws. `release` lets go in place,
  which catches a drag end that forgets to redraw. `drag` presses and moves with no frame
  between; `press`, then `move`s a step apart, drags with frames between, as a hand does.
  Make the first move 1px, as a hand's is: a bigger one starts the drag before hover changes.
- Prompts use GPUI's fallback renderer, which clips long detail text and ignores Escape and Enter.
  Click its buttons, and check prompt copy in the code.
- Enter in a message field adds a line, and ⌘↩ saves.
- Rows move under the pointer: a drag inserts drop zones, and an optimistic stand-in adds a row.
  Take a `shot` mid-drag with `hold` and aim from it. Coordinates read off a cropped shot need the
  crop's origin added.
- The log target is `gitbutler_gpui`, as in `RUST_LOG=gitbutler_gpui=debug`.

## Sandboxes and fixtures

- Run against an `E2E_TEST_APP_DATA_DIR` sandbox. Opening Settings > AI, or searching settings,
  reads the keychain and prompts. Copy menu items write the real clipboard.
- Linked-worktree fixtures need the `worktree_manipulation` flag. Adoption archives every
  worktree it finds the first time, so mark the project adopted before adding worktrees, or they
  vanish from the navigator.
- To show PRs offline, point `origin` at a GitHub URL, turn auto-fetch off, set
  `vb_stack_heads.pr_number` and add the review to `forge_reviews`, as camelCase JSON with the
  `struct_version` from `but-forge`. One malformed row fails the whole workspace load. With no
  account the refresh fails before any request, which shows the stale state too.
- To try a real repository, use an APFS clone (`cp -c -R`) with every remote pointed at a missing
  path, and copy sqlite files with their `-wal` and `-shm`. Run `git update-index --really-refresh`
  in it, or racily clean index entries inflate status timings. Delete it afterwards.

## Conventions

- Match Lite's wording and structure and the rules in `packages/ui-react/design/`. Adapt Lite,
  Tauri and but.dev rather than copy them, and leave out what GPUI has no data for.
- Conflicts are `warn`, never danger red. One accent button per surface. Controls keep the
  arrow cursor, as the desktop does, and a focused field rings in gray (`focus_border`), not
  pop. A disabled control says why in its tooltip. Tooltips are a few words, with the shortcut in its key slot. Snackbars are
  fragments without a full stop. Misleading copy is a bug.
- The app is reactive, so no reload buttons. Nothing added may make it slower.
- Dropped on purpose, so ask before adding them back: PR conversations and management beyond
  create and merge, editing a commit in the app (the resolver and `but resolve` cover it),
  moving a commit one step, updating one stack, Locate for a moved project, diff zoom and the
  palette's theme toggle.
- Format a file on its own with `rustfmt --edition 2024 <file>`: given `lib.rs` or a `mod.rs`,
  rustfmt formats every module under it too, which matters when others edit the crate at once.
