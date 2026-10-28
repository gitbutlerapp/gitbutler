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
- A character the embedded Inter lacks is drawn from another font, heavier and off the
  baseline. Design-core's Inter has only 230, so before writing a new symbol into the UI, check
  it's in `assets/fonts` and add it from the full Inter if not, as the key symbols were.
- The files are cut 60 lighter than the design-core weights they stand in for, because GPUI
  draws thicker stems than Chrome. Compare against but.dev before changing a weight: render both
  at the same scale (headless Chrome with `--force-device-scale-factor=2` and a harness `shot`
  of a region, which keeps the display's scale) and compare stem thickness, not screenshots
  taken at different scales.

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
- The harness window never shows, so macOS draws it only when the script asks. Anything that
  needs a second frame shows in the next step's screenshot, not this one's.
- Profile with `sample <pid> <seconds> -file out.txt`. Look at the main thread: a scripted run
  idles between steps, so most samples there are idle. A thread's sample count is how long it
  lived, blocked or running, so read CPU from the self samples, past the wait frames
  (`__psynch_cvwait`, `mach_msg`, `kevent`).
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
- Layout costs per element, and a flex row of many small ones costs most. The Branches
  overview's thirty day cells a lane were a quarter of that page's frame; placing them with
  `absolute` and `relative` insets halved that, and wording their tooltips in the tooltip's
  builder rather than in render saved a little more.
- Put any list that can grow in `uniform_list` or `list`. A thousand plain rows cost 36ms a
  frame, against 1.6ms in `uniform_list`. Don't derive per-row state for every row each frame:
  a 20,000-line diff stalled until the line grip was worked out only for the row under the
  pointer.

## Power

Users keep the app beside or behind their editor for hours while agents work, so what it does
unseen is what a laptop's battery pays for.

- While no part of the window shows (`Store::hidden`: covered, minimized, on another Space, the
  display asleep), work that only keeps the screen current waits: watcher reloads collect in
  `Store::unseen`, and `Store::set_visible` reads them once it shows. New watcher-driven or timed
  work takes the same path, as the mesh poll does, reading once on show. Work something else
  reads slows down instead: the forge poll asks every fifth minute, since `but status` and the
  MCP server read its cache. A sliver of window
  counts as visible, as macOS counts it.
- Operations the user starts hold off App Nap while they run (`prevent_app_nap` in
  `Store::start`), so a push goes on at full speed when they switch away. Refreshes take the
  priority macOS gives the app: a background app's run on efficiency cores, for more CPU time
  and less energy. Holding it off for every reload measured up to 2.8 times the energy.
- Judge battery by energy, not CPU time or `top`'s estimate: `proc_pid_rusage` with
  `RUSAGE_INFO_V6` reads a process's `ri_energy_nj` and its performance-core time without root.
  Its times are mach ticks, so convert them with `mach_timebase_info`.
- Measure on the plain build with the screen unlocked, in each state a user's window is in:
  frontmost, visible behind another app, and hidden. `osascript` with System Events sets them:
  `set frontmost of` or `set visible of` the process found by its unix id, or activate another
  app. A locked screen makes every app a background one. With an agent writing to mise, the app
  drew about 500 mW frontmost and 6 mW hidden.

## GPUI traps

- Updating or reading a view while it's being updated panics with "already being updated". The
  views `AppView` owns reach it only through `AppHandle::update_later`, which runs once the
  update running now is over, so the app and a view can't update each other that way. Work that
  depends on what the app does goes in its closure. A store `done` callback runs while the store
  is being updated, so it mustn't read the store.
- `window.context_stack()` panics during render. Read key contexts in an event handler.
- A binding that a text field could also take needs `&& !TextInput` in its context, or typing
  the letter runs it (P once pushed). Bind shortcuts shared by both halves to
  `(Navigator || Diff)` rather than `Workspace`, which modals sit inside. `DiffPane` is a
  sibling of the detail's `Diff` context, so its find field has its own.
- GPUI focuses the nearest focusable parent on mouse-down, so clicking a tab inside `Detail`
  takes the keys from a list page. Hand focus back, and never to an element that isn't drawn;
  if a focused one goes anyway, keys reach nothing, so `on_focus_lost` sends them to the navigator,
  or to the diff while a message is being written, as fewer letters act there (not r or b). A
  modal keeps what had the keys and gives them back. Whether the file list is drawn is
  `Detail::files_shown(window, cx)`, which a narrow window turns false, not `files_hidden` alone.
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
  cached view. Release image textures with `cx.drop_image`. `img` plays an image with
  several frames by notifying the view drawing it every frame while the window is active,
  which redraws that view and its parents at the display's rate, so a description's GIF
  keeps its first frame only.
- Taffy stretches only children whose height is `auto`. In a row with only a min-height,
  `h_full()` is 0, so use `self_stretch()`. Flex text needs `min_w_0()` to shrink or wrap.
  `flex_1` has a zero basis and lets a name shrink to nothing, so use `flex_grow(1.)` and give
  lesser items `flex_shrink(1000.)`. Margins collapse through a card's background, so use padding.
- A `cx.notify()` during prepaint is dropped, so defer it. `window.refresh()` does nothing during
  paint, so use `on_next_frame`. An `invisible()` element has no hitbox, so use opacity 0.
- `AppView` draws `DiffPane` absolutely over the detail pane, at `HEADER_HEIGHT` plus
  `Detail::view_header_height`. Anything that changes the height above the file list, like tabs
  or a banner, must be counted there, or the diff covers it and takes its clicks. Side-column
  content wider than the column slides under the diff the same way.
- `InteractiveText` maps a click on centered text (`text_center`) as if each line were
  left-aligned, so its links miss. Let the parent center a block that fits its text instead.

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
  anyway run the dry run again and go ahead only on the conflicts the user accepted. Counting
  them by the workspace's commits misses a pull's incoming ones, so `integrate_remote` returns
  their change IDs; any conflicted commit the main checkout doesn't know may be a linked
  worktree's that was conflicted already.
- The queue tells operations apart by `Overlay::kind` (`OpKind` in `state/mutation.rs`), never by
  their label, which is only what the user reads. One a double-click would ask for twice, and
  whose repeat would only fail, is `OpKind::once`, which drops the repeat while it waits or runs.
  Resolve refuses while one is under way instead, as the conflicts left are numbered anew after it.
- The oplog records the workspace and the main checkout only. Undo and History restore refuse
  anything that touched a linked worktree, in older snapshots too. Commits in a linked worktree
  skip hooks, which would run against the main checkout's files. Branches move only within their
  own side. An operation that makes a branch records its snapshot with
  `from_details_with_refs_and_perm` naming it, or undo leaves the branch behind.
- ⌘Z follows the user, not the selection. `Store::undo_steps` takes the oplog entry of every
  operation the app runs, in any project, ordered by when it was asked for, and ⌘Z undoes the
  newest only while it's still its project's undo target, so it never takes back an agent's or
  the CLI's work. Undo and Redo are `OpKind::Undo` and `OpKind::Redo` and keep the lists
  themselves; an operation the oplog can't undo calls `keep_last_snapshot`, which drops it.
- An oplog restore puts the whole worktree back, so `history::undo_snapshot` first checks for
  files written and branches moved after the operation, leaving out the spans the app's own
  local operations ran in (`Store::ran`, read when the undo runs, so a queued one sees those
  before it; pushes and fetches record none, as they write only what the remote has, and a
  Git commit logged in a second wholly after the operation counts even inside a later one's,
  as the app never logs "commit"; with no log of HEAD, as
  with core.logAllRefUpdates off, undo refuses rather than guess): a move
  Git logged then is `Undone::Behind`, a file written then `Undone::Newer` for ⌘Z to ask
  about; `redo_snapshot` checks the same from the undo's own snapshot. A step stays on the
  list unless it was undone, `Undone::Behind` says something came after it, or
  `Undone::Already` says something else undid it.
- An operation's `done` runs with `Store::staying` set when the user has moved to another
  project, which makes `Store::select` do nothing: what it would select, and the picks a select
  clears, stay where the user is, and `card_title` names the operation's project.
- What undo can't take back (a linked worktree's discards) takes its commit with
  `Remap::exactly`, refusing one an agent rewrote since it was asked about, where `Remap::get`
  would follow it.
- A snapshot taken with one branch checked out outside the workspace keeps no stacks: only the
  branch's ref and tip under `checkout/`, and the refs below it under `additional-refs/`. History
  reads those branches' commits from the repository, down to the snapshot's target.
- PR and CI data come from the forge cache the workspace load already reads, and nothing calls
  the network to render. What the cache doesn't keep, whether a PR can merge and its
  conversation, the store asks for as the PR tab shows, again each time the forge is asked. CI checks refresh only for applied branches, so cached checks on an
  unapplied branch are stale.

## Metrics

Like the mesh, metrics must never break or slow anything else.

- Its code stays in `backend/metrics.rs` and `backend/metrics/crashes.rs`; the rest calls
  `backend::metrics::*` at a few places: launch and quit in `lib.rs`, `Store::start` for each
  operation, `Store::failed` for each error card, `Store::count_view` for what's opened,
  `ProjectEntry::failed` for a project that stops reading, signing in and out, and the Metrics
  card's switches in `settings/metrics.rs`.
- A property the desktop app sends too keeps its name and values (`totalLanesInWorkspace`,
  `theme`, `error_code`), so one chart covers both apps.
- Nothing sent may name a branch, file, project or anything the user wrote. An operation's label
  that names one needs `Label::new` with a fixed name for the metrics, which the compiler asks for;
  a fixed `&'static str` label is its own name. Errors go through `redact`.
- Capture only where the user acts, never in render, loads, the watcher or a loop. Failures and
  errors of one kind go out once a minute (`limit`).

## Mesh

The mesh shows the user's other machines' work. It's optional, and it must never break or slow
anything else.

- Its code stays in five files: `backend/mesh.rs` runs the `but-mesh` CLI and reads its JSON,
  `state/mesh.rs` holds `Mesh` with its poll, sign-in and apply, `navigator/mesh.rs` draws
  each card's checkouts elsewhere and folds them, `detail/machine.rs` fetches and shows what's
  selected there, and `settings/mesh.rs` is the Mesh row in Settings. Everything else reaches
  it through a `Selection::Mesh` or `Section::Mesh` arm, `store.mesh`, the `Store::mesh_*`
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

- Look at the UI with `scripts/drive.sh` (the `gpui-screenshots` skill), never by starting the
  app: `cargo run` and the plain binary open a window on the user's screen and take their keys.
  It builds the dev-script binary, runs a script against app data of its own, and keeps a
  session to step through. Its window never shows, the app stays out of the Dock and never
  comes in front, and a run quits when its script ends. `drive.sh run --release` measures.
- The plain and dev-script builds write the same `target/release/gitbutler-gpui`. The plain one
  ignores `GITBUTLER_GPUI_SCRIPT`, and the run hangs. Check with
  `strings target/release/gitbutler-gpui | grep -c GITBUTLER_GPUI_SCRIPT`, and rebuild the plain
  one with `cargo build --release -p but-gpui -p gitbutler-git --bins` when done, so the askpass
  helper is there for pushes.
- The script waits for loads to settle without a timeout and quit waits for running operations,
  so `drive.sh` kills a run after its `--timeout`. Running the binary yourself, guard it with a
  kill of your own, as macOS has no `timeout`. Kill only the process you started.
- Each step settles for 120ms, so a delay shorter than that never shows. Stretch the constant to
  check one. A hover registers only after a second small `move`.
- `dclick` draws a frame between its clicks, as a hand's double-click has several, so the second
  click's handler sees what the first one changed, not what the step started with.
- `click` presses and lets go within one frame. `InteractiveText`, which links in text use, starts
  listening for the let-go only on the frame after the press, so click those with `press`, then
  `release`.
- `drop` moves the pointer before letting go, and that move redraws. `release` lets go in place,
  which catches a drag end that forgets to redraw. `drag` presses and moves with no frame
  between; `press`, then `move`s a step apart, drags with frames between, as a hand does.
  Make the first move 1px, as a hand's is: a bigger one starts the drag before hover changes.
- Prompts use GPUI's fallback renderer, which ignores Escape and Enter, so click its buttons. It
  clips long detail, which each prompt also prints as `PROMPT message | detail` (`grep -A2 PROMPT`).
- Enter in a message field adds a line, and ⌘↩ saves.
- To make an operation slow enough to race, turn on signing with a `gpg.program` script that
  sleeps, then prints a dummy signature. Keep the racing steps in one script run.
- Rows move under the pointer: a drag inserts drop zones, and an optimistic stand-in adds a row.
  Take a `shot` mid-drag with `hold` and aim from it. A whole-window shot is at the window's
  scale, so its pixels are click coordinates; a region keeps the display's, so divide by it and
  add the region's origin.
- Typing reaches what `keys` names, and `drive.sh step` prints it after each step. In a list or
  the diff, letters run shortcuts, and after `add` the navigator has the keys, so click a field
  before typing in it.
- The log target is `but_gpui`, as in `RUST_LOG=but_gpui=debug`. The log also goes to the
  desktop app's log folder (`backend::logs::dir()`), which in a sandbox is
  `$E2E_TEST_APP_DATA_DIR/logs`.
- The harness window never shows, which macOS reports as hidden, so a scripted run ignores
  the real visibility and starts shown. `hide` and `show` steps set it, as covering the window
  and bringing it back would.
- The harness window is never key, as the app never comes in front, and GPUI runs focus and
  blur listeners only in a key window: `on_focus` and `on_blur` never fire, so a rename doesn't
  end on a click elsewhere, and carets don't blink. `on_focus_lost` fires as usual. Check those
  by hand.
- AppKit's own windows, as Add project's folder picker, would still open on the user's screen,
  so add projects with `add`.

## Sandboxes and fixtures

- Run against an `E2E_TEST_APP_DATA_DIR` sandbox, as `drive.sh` does. Opening Settings > AI, or searching settings,
  reads the keychain and prompts. Copy menu items write the real clipboard.
- Adoption archives every linked worktree it finds the first time, so mark the project adopted
  before adding worktrees, or they vanish from the navigator.
- To show PRs offline, point `origin` at a GitHub URL, turn auto-fetch off, set
  `vb_stack_heads.pr_number` and add the review to `forge_reviews`, as camelCase JSON with the
  `struct_version` from `but-forge`. One malformed row fails the whole workspace load. With no
  account the refresh fails before any request, which shows the stale state too.
- To try a real repository, use an APFS clone (`cp -c -R`) with every remote pointed at a missing
  path, and copy sqlite files with their `-wal` and `-shm`. Delete it afterwards.
- Run `git update-index --really-refresh` in every repository copied or extracted for a run. A
  fresh copy leaves its index entries racily clean, so status hashes every file on every read,
  which tripled the CPU an agent's writes cost.

## Sizes

- Give every size in `ui::dp(n)`: `n` pixels as designed, so ui-react's numbers go in as they
  are. It is in rems, and the window's rem follows the interface scale (⌘+ and ⌘−), so the
  whole app zooms as a browser does. Clippy refuses plain `gpui::px`; `ui::fixed` is for what
  must stay put, as a hairline, the room for the traffic lights, the scrollbar or window bounds.
- Sizes the app keeps for layout, as pane widths and measured header heights, are in design
  pixels too. Divide what GPUI measures by `Theme::scale` before keeping it, as `measured` and
  `resize_pane` do, or `dp` scales it twice. Where code needs real `Pixels`, as for scrolling,
  pointer maths or a list's row estimate, use `Theme::scaled`.
- A `ListState` keeps its rows' heights until its width changes, so `AppView::set_ui_scale`
  remeasures the lists. A new list needs the same.

## Platforms

The client runs on macOS, Windows and Linux. AppKit code sits behind
`#[cfg(target_os = "macos")]`. A Mac never compiles the `cfg(windows)` or Linux code; the
Windows CI job lints and tests the one, the Linux lint and test jobs the other.

- Keys live in `assets/keymaps/default.json`, in Zed's keymap format, written with `secondary-`,
  which is ⌘ on macOS and Ctrl elsewhere. Keys only some platforms have, as Hide, go in a section
  with `"os"`. Within a context the later binding wins, and the last binding for an action is the
  key menus and tooltips show, so add the one to show last. Text fields' own editing keys stay in
  `ui::text_input`, as their actions take arguments the file can't build.
- Write keys in copy as macOS does (`⌘↩`) and pass the text through `ui::keys_text`, which
  shows `Ctrl+Enter` on Windows and Linux. `ui::keycap` and the status bar's hints already do.
- Name platform things through one place: `ui::REVEAL` for the file manager,
  `std::env::consts::OS` for `but-api`'s per-platform lists, `EXE_SUFFIX` for a program's name.
- Windows and Linux have no menu bar, so every command in `cx.set_menus` also needs a palette
  entry.
- Git paths use `/`. Before showing, copying or revealing one joined onto a checkout, replace
  it with `MAIN_SEPARATOR_STR`, as the file list's menu does.
- A program the app starts gets `CREATE_NO_WINDOW` on Windows, as `backend::mesh` does, so no
  console window opens with it.
- The headers are the window's title bar: drawn over macOS's, which AppKit leaves to them
  (`app_owns_titlebar_drag`), and in place of one elsewhere. `ui::drags_window` makes them
  drag the window and gives a double click the platform's action, and the app draws the
  window's buttons where `ui::draws_window_controls` says: on Windows, and on
  Linux where the desktop leaves the frame to the app (GNOME on Wayland), where
  `ui::window_frame` also draws the shadow that resizes the window. Wrap anything clickable in
  a header in `ui::above_drag` so it keeps its clicks.
- Tests that need Unix APIs, as file modes or `sh`, carry `#[cfg(unix)]`.

## Accessibility

- Build controls from `ui`'s helpers (`button`, `switch`, `dropdown`, `segmented`, `list_row`,
  `checkbox`). Each gives VoiceOver a role and a name, and the first four are tab stops. GPUI
  reports only elements with both an id and a `role`, so a hand-built clickable `div` needs
  `.role()` and `.aria_label()`, and `ui::tab_stop` when it's a control in a dialog.
- In the window Tab moves between panes. In a prompt or a modal (`dialog` in `overlays.rs`,
  the `Dialog` key context) it goes round that dialog's tab stops, and `open_modal` gives a new
  modal this. Clicking a tab stop leaves the keys where they were, as clicking a macOS button
  does. Tab scrolls the control it reaches into view only inside `ui::scrollbar::scrolled`, so
  a dialog's scrolling list goes in one.
- A colour comes from `Theme`, and a new one is a step on design-core's scales (`tokens.css` in
  `@gitbutler/design-core`). Text needs 4.5:1 and a control's marks 3:1, on `bg_1` and `bg_2`
  in both themes. Design-core's own text-safe, text-danger and fill-pop fall short in light,
  so `Theme` takes the next darker step.

## Conventions

- Match Lite's wording and structure and the rules in `packages/ui-react/design/`. Adapt Lite,
  Tauri and but.dev rather than copy them, and leave out what GPUI has no data for.
- Conflicts are `warn`, never danger red. One accent button per surface. Controls keep the
  arrow cursor, as the desktop does, and a focused field rings in gray (`focus_border`), not
  pop. A disabled control says why in its tooltip. Tooltips are a few words, with the shortcut in its key slot, and a right-click item that a key also does shows it, built with `MenuItem::keyed` or `MenuItem::action`. Snackbars are
  fragments without a full stop. Misleading copy is a bug.
- The app is reactive, so no reload buttons. Nothing added may make it slower.
- Nothing outside tests may panic, as the app runs as GitButler itself: clippy denies unwrap,
  expect and indexing or slicing that can fail. When an index is stale, render code skips what
  it would have drawn (`.get(ix)?`), and code that acts returns early or errs. Neither invents
  a value with `unwrap_or_default` where empty isn't truly right. `#[expect(lint, reason)]`
  is for what's provably in range.
- What an agent does must not move the row under the pointer: the navigator anchors it
  (`sync_list`), and its order by activity holds while the pointer is over it.
- Dropped on purpose, so ask before adding them back: writing in PR conversations (replying,
  reacting, resolving) and managing PRs beyond create and merge, editing a commit in the app (the resolver and `but resolve` cover it),
  moving a commit one step, updating one stack, Locate for a moved project, diff zoom and the
  palette's theme toggle.
- Format a file on its own with `rustfmt --edition 2024 <file>`: given `lib.rs` or a `mod.rs`,
  rustfmt formats every module under it too, which matters when others edit the crate at once.
