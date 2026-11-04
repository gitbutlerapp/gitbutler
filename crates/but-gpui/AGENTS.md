# GPUI client instructions

These apply to `crates/but-gpui`, on top of `crates/AGENTS.md`. The README is for people
running and developing the app, and stays short: don't describe behaviour, key bindings or the
code's layout in it, and leave it alone when behaviour changes.

Look at the UI with `scripts/drive.sh` (the `gpui-screenshots` skill), never by starting the
app: `cargo run` and the plain binary open a window on the user's screen and take their keys.
The skill also covers fixtures and measuring performance and power.

## Fonts

- Use only the font families the app embeds (`assets/fonts`, `theme.ui_font` and
  `theme.mono_font`). Any other resolves to whatever the machine has installed, and an
  installed variable font with an optical size axis (`opsz`) lays out many times slower. Only
  fonts the user picks leave them: `theme.diff_font` and `theme.terminal_font`, checked with
  `ui::digit_advance` first. Never load a user's font with `add_fonts`, which keeps a variable
  font at its default instance.
- A character the embedded Inter lacks is drawn from another font, heavier and off the
  baseline. Before writing a new symbol into the UI, check it's in `assets/fonts` and add it
  from the full Inter if not. `assets/fonts/README.md` has the recipe.

## Frame budget

- GPUI draws every view again, caches or not, when focus moves and when input switches between
  mouse and keyboard, so each view's own cost matters even when it caches.
- A view observing another redraws on its every notify. Notify only what changed: a change
  only the diff shows shouldn't go through `Store::set_prefs`, which redraws the whole window.
- A cached view that misses its cache renders all its descendants with caching off, so a
  cached child inside a view that notifies often saves nothing.
- Reading an entity in `render` has the window draw on its every notify, even if it's never
  mounted. Keep what a row shows of something that notifies often where the row already looks,
  as `Terminals::title` does.
- Text shaping dominates frames with new content, and each text run is shaped on its own, so
  fewer text elements per row is the lever. GPUI keeps a shaped line only while the same text
  is drawn the frame after, so text that changes a little every frame is shaped in pieces that
  keep, as the terminal shapes each run of a row.
- Layout costs per element, and a flex row of many small ones costs most: place them with
  `absolute` insets. Word tooltips in the tooltip's builder, not in render.
- Put any list that can grow in `uniform_list` or `list`, and don't derive per-row state for
  every row each frame.

## Power

Users keep the app beside or behind their editor for hours while agents work, so what it does
unseen is what a laptop's battery pays for.

- While no part of the window shows (`Store::hidden`), work that only keeps the screen current
  waits: watcher reloads collect in `Store::unseen`, and `Store::set_visible` reads them once it
  shows. New watcher-driven or timed work takes the same path, as the mesh poll does. Work
  something else reads slows down instead, as the forge poll does.
- Operations the user starts hold off App Nap while they run (`prevent_app_nap` in
  `Store::start`). Refreshes don't, so a background app's run on efficiency cores.

## GPUI traps

- Updating or reading a view while it's being updated panics with "already being updated". The
  views `AppView` owns reach it only through `AppHandle::update_later`, which runs once the
  update running now is over. A store `done` callback runs while the store is being updated, so
  it mustn't read the store.
- `window.context_stack()` panics during render. Read key contexts in an event handler.
- A binding that a text field could also take needs `&& !TextInput` in its context, or typing
  the letter runs it. Bind shortcuts shared by both halves to `(Navigator || Diff)` rather than
  `Workspace`, which modals sit inside.
- GPUI focuses the nearest focusable parent on mouse-down, so clicking inside `Detail` can take
  the keys from a list page. Hand focus back, and never to an element that isn't drawn, or keys
  reach nothing. A modal keeps what had the keys and gives them back.
- A drag's payload stays as the drag began. Drop targets read `Store::dragging`, which follows
  reloads.
- A cached view redraws only on its own notify. `window.focus` doesn't notify it, and a drag let
  go over a target that refuses it ends without any redraw.
- GPUI calls an element unhovered once the pointer moves with the button down on it, and starts
  a drag only past 2px, so a drag handler attached only while hovered is gone by then unless the
  press keeps it (`grip_held`).
- After a key press GPUI counts the pointer as over nothing until it moves, so a drop right after
  Escape is skipped.
- `list` clips to its own bounds, not to each row, so a line drawn on a row boundary is painted
  over by the next row. Wrap it in `deferred()`.
- Something drawn later (`deferred`, `window.defer_draw`) inside another is drawn, and takes the
  pointer, past the list it's in. Nest `ui::fold::on_top`, which passes its mask on.
- `svg()` draws in one colour and doesn't inherit the text colour, so set one. `img` loads
  asynchronously, and an image that arrives late never repaints a cached view. Release image
  textures with `cx.drop_image`. An image with several frames redraws its view and parents at
  the display's rate, so a GIF keeps its first frame only.
- Taffy stretches only children whose height is `auto`. In a row with only a min-height,
  `h_full()` is 0, so use `self_stretch()`. Flex text needs `min_w_0()` to shrink or wrap.
  `flex_1` has a zero basis and lets a name shrink to nothing, so use `flex_grow(1.)` and give
  lesser items `flex_shrink(1000.)`. Margins collapse through a card's background, so use padding.
- A `cx.notify()` during prepaint is dropped, so defer it. `window.refresh()` does nothing during
  paint, so use `on_next_frame`. An `invisible()` element has no hitbox, so use opacity 0.
- `AppView` draws `DiffPane` absolutely over the detail pane, below
  `Detail::view_header_height`. Anything that changes the height above the file list, like tabs
  or a banner, must be counted there, or the diff covers it and takes its clicks.
- In a split, what acts reads the selection, the side with the keys. What the detail and the
  diff draw and load reads `Store::shown`, and resets on `StoreEvent::ShownChanged`, not
  `SelectionChanged`.
- A cached view draws again when its bounds or its clip change, and a `list` measures every row
  again when its width does, so a pane opens and shuts by sliding at its full width
  (`ui::fold::Slide`), not by resizing.
- An element's own mouse listeners run only while its hitbox is hovered. What follows the
  pointer over the whole window registers with `window.on_mouse_event` from a `canvas`, as the
  scrollbar does.
- `InteractiveText` maps a click on centered text (`text_center`) as if each line were
  left-aligned, so its links miss. Let the parent center a block that fits its text instead.

## GitButler semantics

- Only `src/backend` touches GitButler crates. `backend::run` starts its work when called, not
  when awaited, and blocking work can't be cancelled, so debounce before calling it. A panic in
  it becomes a "GitButler task failed" error. A panic on the UI thread crashes the app.
- Key UI state by identity, never by index, sha or byte offset: commits by `Commit::key`, their
  change ID, and hunks by their old-side range, since an agent's edit above shifts every index.
- A menu, prompt, picker, form or drag holds the ref name or commit id it opened on, which an
  agent can rename or rewrite before it's used. Follow it (`follow_by_key`,
  `StoreEvent::Renamed`), or close or refuse once it's gone; never act on the stale one. What
  undo can't take back never follows a rewrite, and refuses instead.
- A confirmation that can't be undone (a linked worktree's discard or delete, a merge into the
  target) carries what it asked about, as a file's `modified` time or a branch's tip, and refuses
  if that moved meanwhile. Commit, amend and discard of hunk and line selections do the same, as
  an agent's edit slides them.
- An operation is an `Intent` in `src/backend`: the ids the user acted on, applied by
  `Store::apply`. One that writes the workspace does so in one `Run::locked` call, one
  transaction or `_with_perm` call, so it is one oplog entry and one ⌘Z step. `Run::watched` is
  only for work that takes GitButler's lock itself. Fetches, pushes and the forge's merges call
  neither, and record no undo step.
- A project runs one operation at a time, until the reload after it lands, and `Store::apply`
  turns down another asked for meanwhile; nothing queues. Fetches, pushes and deleting a linked
  worktree (`OpKind::beside`) run beside it. Tell operations apart by `Intent::kind`, never by
  their label.
- An operation that makes or deletes a branch names it in its snapshot (the transaction's
  snapshot refs, else `from_details_with_refs_and_perm`), or undo leaves the branch behind or
  doesn't bring it back. One the oplog can't undo calls `keep_last_snapshot`.
- ⌘Z undoes only the app's own operations, and only while one is still its project's undo
  target, so it never takes back an agent's or the CLI's work. The oplog records the workspace
  and the main checkout only, so undo refuses anything that touched a linked worktree.
- Every forced push except Push anyway fetches first and refuses if the remote moved: counts,
  titles and change IDs all missed a teammate's amend. Update anyway and Pull anyway go ahead
  only on the conflicts the user accepted.
- Nothing calls the network to render: PR and CI data come from the forge cache the workspace
  load already reads, and what it doesn't keep the store asks for as its tab shows.

## Metrics

Like the mesh, metrics must never break or slow anything else.

- Its code stays in `backend/metrics.rs` and `backend/metrics/crashes.rs`. Capture only where the
  user acts, never in render, loads, the watcher or a loop. Failures and errors of one kind go
  out once a minute (`limit`).
- A property the desktop app sends too keeps its name and values (`totalLanesInWorkspace`,
  `theme`, `error_code`), so one chart covers both apps.
- Nothing sent may name a branch, file, project or anything the user wrote. An operation's label
  that names one needs `Label::new` with a fixed name for the metrics, which the compiler asks
  for. Errors go through `redact`.

## Mesh

The mesh shows the user's other machines' work. It's optional, and it must never break or slow
anything else.

- Its code stays in `backend/mesh.rs`, `state/mesh.rs`, `navigator/mesh.rs`,
  `detail/machine.rs` and `settings/mesh.rs`. Everything else reaches it through a
  `Selection::Mesh` or `Section::Mesh` arm, `store.mesh`, the `Store::mesh_*` methods or
  `StoreEvent::MeshRead`, with at most a dispatch arm.
- The app only calls the `but-mesh` CLI, as the mesh lives in a private repository. Gate
  anything new on `store.mesh.connection()`: without `but-mesh`, or signed out, only the
  Settings row shows.
- Every mesh call goes through `backend::run` with one of the timeouts in `backend/mesh.rs`,
  never on the UI thread. Network work never runs as a project's operation, which would hold up
  commits, pushes and updates.
- Background failures log at debug, keep the last good read and are tried again. Only what the
  user asked for, signing in or out and Apply here, shows an error.

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
`#[cfg(target_os = "macos")]`. A Mac never compiles the `cfg(windows)` or Linux code; CI lints
and tests it.

- On a Mac without Xcode, building the client stops at `error: tool 'metal' not found`. Pass
  `--features gpui_platform/runtime_shaders` to `cargo check`, `build`, `test` and `clippy`.
- Keys live in `assets/keymaps`, in Zed's keymap format, written with `secondary-`, which is ⌘ on
  macOS and Ctrl elsewhere. `base.json` holds only the keys macOS gives every app; everything the
  app gives a meaning to, even ⌘C or ⌘Z, goes in the file of each `keymap::Keymap`. The last
  binding for an action is the key menus and tooltips show.
- Write keys in copy as macOS does (`⌘↩`) and pass the text through `ui::keys_text`, which
  shows `Ctrl+Enter` on Windows and Linux.
- Name platform things through one place: `ui::REVEAL` for the file manager,
  `std::env::consts::OS` for `but-api`'s per-platform lists, `EXE_SUFFIX` for a program's name.
- Windows and Linux have no menu bar, so every command in `cx.set_menus` also needs a palette
  entry.
- Git paths use `/`. Before showing, copying or revealing one joined onto a checkout, replace
  it with `MAIN_SEPARATOR_STR`.
- A program the app starts gets `CREATE_NO_WINDOW` on Windows, so no console window opens with
  it.
- The headers are the window's title bar. Wrap anything clickable in a header in
  `ui::above_drag` so it keeps its clicks.
- On macOS the click that brings the window in front only does that. A capture listener that
  acts, as `on_mouse_down_out` often does, skips it with `ui::brings_window_in_front`, unless it
  should act on it, as a menu closing does.
- Tests that need Unix APIs, as file modes or `sh`, carry `#[cfg(unix)]`.

## Accessibility

- Build controls from `ui`'s helpers (`button`, `switch`, `dropdown`, `segmented`, `list_row`,
  `checkbox`). Each gives VoiceOver a role and a name, and the first four are tab stops. GPUI
  reports only elements with both an id and a `role`, so a hand-built clickable `div` needs
  `.role()` and `.aria_label()`, and `ui::tab_stop` when it's a control in a dialog.
- In a prompt or a modal Tab goes round that dialog's tab stops, and `open_modal` gives a new
  modal this. Tab scrolls the control it reaches into view only inside
  `ui::scrollbar::scrolled`, so a dialog's scrolling list goes in one.
- A colour comes from `Theme`, and a new one is a step on design-core's scales
  (`packages/design-core/tokens/tokens.css`). Text needs 4.5:1 and a control's marks 3:1,
  on `bg_1` and `bg_2` in both themes.

## Conventions

- Match the rules in `packages/ui-react/design/`. Adapt Tauri and but.dev rather than copy
  them, and leave out what GPUI has no data for.
- Conflicts are `warn`, never danger red. One accent button per surface. Controls keep the
  arrow cursor, as the desktop does, and a focused field rings in gray (`focus_border`), not
  pop. A disabled control says why in its tooltip. Tooltips are a few words, with the shortcut
  in its key slot, and a right-click item that a key also does shows it, built with
  `MenuItem::keyed` or `MenuItem::action`. Snackbars are fragments without a full stop.
  Misleading copy is a bug.
- The app is reactive, so no reload buttons. Nothing added may make it slower.
- A part the Lite interface leaves out is a `Part`, asked through `Prefs::shows`, never a
  `bool` of its own in `Prefs`.
- Nothing outside tests may panic, as the app runs as GitButler itself: clippy denies unwrap,
  expect and indexing or slicing that can fail. When an index is stale, render code skips what
  it would have drawn (`.get(ix)?`), and code that acts returns early or errs. Neither invents
  a value with `unwrap_or_default` where empty isn't truly right. `#[expect(lint, reason)]`
  is for what's provably in range.
- What an agent does must not move the row under the pointer: the navigator anchors it
  (`sync_list`), and its order by activity holds while the pointer is over it.
- Motion reads `clock::now()`, never `Instant::now()`, so a script can stop it and draw any
  moment of it. Don't stretch a motion's duration to see it.
- Dropped on purpose, so ask before adding them back: writing in PR conversations (replying,
  reacting, resolving) and managing PRs beyond create, merge and editing the title and
  description, editing a commit in the app (the resolver and `but resolve` cover it), moving a
  commit one step, updating one stack, Locate for a moved project, diff zoom and the palette's
  theme toggle.
- Format a file on its own with `rustfmt --edition 2024 <file>`: given `lib.rs` or a `mod.rs`,
  rustfmt formats every module under it too, which matters when others edit the crate at once.

## Motion

How a mode with the keys (moving, squashing) shows itself as the user steps through it.
Check a motion against these with `film` (Checking a motion in the `gpui-screenshots` skill).

- What a mode takes (dimmed, in a dashed outline) holds still while the user steps. It never
  changes colour, opacity or border as the outline moves, whether at its own rows or
  elsewhere, at the first frame or as the outline arrives.
- The outline showing where it would go is the only thing that moves, drawn on top of the
  rows. It slides between rows, and to fit a group (a branch and its commits, picked commits)
  it changes shape by moving its top and bottom edges apart. It is never swapped for another
  highlight when it lands, and nothing under it changes so it can land.
- Picked commits act as one group, as a branch does: where they started is one stop, and the
  outline grows around all of them there.
- Use the marks the modes already have: the outline, dimming, dashes and colour. Don't add
  chips, badges or boxes beside a row to say what a mode does; the status bar says it.
- What a mode takes keeps its mark wherever the outline is. A motion fix that takes a mark
  away is a regression.
