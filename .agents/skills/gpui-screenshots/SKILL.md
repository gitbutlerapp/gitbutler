---
name: gpui-screenshots
description: Use whenever you need to see, click through or screenshot the GPUI client (crates/but-gpui), to check a UI change works, reproduce a UI bug, or show what a change looks like. Drives the app from a script in a window that never shows on the user's screen, against throwaway app data. Never start the app with `cargo run` or the plain binary to look at it.
---

`crates/but-gpui/scripts/drive.sh` builds the client with `--features dev-script` and runs it
from a script. The window never shows, the app stays out of the Dock and never takes the
user's keys, and it runs against app data of its own, so none of the user's projects or
settings are touched. Starting the app any other way opens a window on the user's screen.

## A repository to show

The app writes to what it opens: GitButler's workspace, refs, the oplog. Never `add` the
checkout you're working in, or any repository of the user's. Make a throwaway one:

```bash
repo=$(mktemp -d)/repo
git init -q -b main "$repo"
git -C "$repo" -c user.name=Tester -c user.email=t@example.com commit -q --allow-empty -m "Initial commit"
echo hello > "$repo/notes.txt"
```

Build whatever state the change needs (branches, commits, uncommitted edits) with `git` here,
or later with `sh` steps, which run while the app watches.

## Fixtures

- To try a real repository, use an APFS clone (`cp -c -R`) with every remote pointed at a
  missing path, and copy sqlite files with their `-wal` and `-shm`. Delete it afterwards.
- Run `git update-index --really-refresh` in every repository copied or extracted for a run. A
  fresh copy leaves its index entries racily clean, so status hashes every file on every read.
- Adoption archives every linked worktree it finds the first time, so mark the project adopted
  before adding worktrees, or they vanish from the navigator.
- To show PRs offline, point `origin` at a GitHub URL, turn auto-fetch off, set
  `vb_stack_heads.pr_number` and add the review to `forge_reviews`, as camelCase JSON with the
  `struct_version` from `but-forge`. One malformed row fails the whole workspace load. With no
  account the refresh fails before any request, which shows the stale state too.
- To make an operation slow enough to race, turn on signing with a `gpg.program` script that
  sleeps, then prints a dummy signature. Keep the racing steps in one script run.
- Try mesh failures with a fake `but-mesh` first on the `PATH`, which the app searches before
  `~/.local/bin`: one that sleeps tests the timeouts and that the project stays free for
  operations, and one that exits non-zero or prints garbage tests the fallbacks. A run's app
  data is its own but the mesh isn't: with the real `but-mesh` signed in, a run reports its
  repositories to your mesh.

## One run

Write a script, one command per line (`src/dev.rs` lists them all), and run it:

```text
add /path/to/repo        # the folder Add a repository would be given
wait 1000                # let the first loads land
shot /tmp/before.png
click 400 184            # window coordinates, see below
key cmd-k
type history
shot /tmp/after.png
```

```bash
crates/but-gpui/scripts/drive.sh run script.txt
```

It prints what the app reported (failed steps as `script line N: …`, prompts as `PROMPT …`,
warnings and errors) and where the app data and logs went. The run quits when the script
ends and is killed after `--timeout` seconds (300). Read the shots to check them.

## A step at a time

To explore, or to find where to click from what's on screen:

```bash
crates/but-gpui/scripts/drive.sh start
crates/but-gpui/scripts/drive.sh step "add /path/to/repo" "wait 1000"
crates/but-gpui/scripts/drive.sh step "click 400 184" "key down"
crates/but-gpui/scripts/drive.sh stop
```

Each `step` runs its arguments as script lines, saves a shot, and prints its path (under
`target/gpui-drive/shots`) after anything the app reported and a `KEYS` line. A session quits
on its own after `--timeout` seconds (1800); always `stop` it when done. One session per
checkout.

## Where the keys go

`KEYS Workspace > Navigator > TextInput` says what typing reaches: here a text field in the
navigator. Check it before `type` or a single-letter `key`. In a list or the diff, letters run
shortcuts, so a query typed there makes branches (b), pushes (⇧P), or opens the user's editor
on their screen (e) instead. After `add` the navigator has the keys, so click
the field you mean first. In a `run` script, a `keys` line prints the same.

## Coordinates

The window is 1360×860 in logical pixels, which is what `click`, `move`, `drag` and `scroll`
take, and a shot is saved at that size: where something shows in it is where to click it.
`shot path x y w h` saves just that region, given in the same coordinates, at the display's
full resolution (2× on a Retina Mac) for a closer look, so a point read off it is the region's
origin plus half its pixel offset. `size W H` resizes the window.

## Inputs

- Each step settles for 120ms, so a delay shorter than that never shows; stretch the constant
  to check one. A hover registers only after a second small `move`.
- The window draws only when the script asks, so anything that needs a second frame shows in
  the next step's shot, not this one's. A palette choice that goes nowhere, as New terminal,
  runs once the next frame is drawn, so type what it's for in the next step, not the same one.
- `click` presses and lets go within one frame. `InteractiveText`, which links in text use,
  starts listening for the let-go only on the frame after the press, so click those with
  `press`, then `release`. `dclick` draws a frame between its clicks, so the second click's
  handler sees what the first one changed.
- `drop` moves the pointer before letting go, and that move redraws. `release` lets go in place,
  which catches a drag end that forgets to redraw. `drag` presses and moves with no frame
  between; `press`, then `move`s a step apart, drags with frames between, as a hand does. Make
  the first move 1px: a bigger one starts the drag before hover changes.
- Rows move under the pointer: a drag inserts drop zones, and an optimistic stand-in adds a row.
  Take a `shot` mid-drag with `hold` and aim from it.
- `fclick` clicks as the click that brings the window in front, which on macOS does only that.
- Prompts use GPUI's fallback renderer, which ignores Escape and Enter, so click their buttons.
  It clips long detail, which each prompt also prints as `PROMPT message | detail`.
- Enter in the composer's commit title commits, and Tab goes to its body, where Enter adds a
  line. A pull request's title goes on to its body on Enter, and ⌘↩ saves.
- The window starts shown, though macOS reports it hidden. `hide` and `show` steps set it, as
  covering the window and bringing it back would.

## Animations

Folds, slides and rows moving to a new order are drawn at a clock a script can stop, so any
moment of a motion can be drawn and looked at. `film` films the input after its `--`:

```text
film /tmp/fold 200 8 0 40 340 540 -- click 306 66
```

That stops the clock, saves the frame before the input as `before.png`, runs the input, and
saves a frame every 8ms for 200ms of that region (`000.png`, `001.png`, …), then runs the
clock again. The input is one script line. For a drag, `press` and at least two small `move`s
go before the `film`, and the `move` that sets the motion off goes after its `--`. GPUI starts
a drag on one move and sends it moves only from the next, so with fewer, frame 000 shows the
drag taking hold rather than the motion.

`sheet.png` has all the frames in a grid, a row at a time, each labelled with its time. Each
frame's line says what changed from the one before, the before frame for frame 000:

```text
FILM 000 t=0ms changed 5.3% in 1 place: 311x32 at 14,50 (38830px Δ208)
FILM 019 t=152ms changed 9.0% in 489 places: 319x10 at 10,270 (6420px Δ37), 319x10 at 10,252 (6414px Δ37), …
FILM 020 t=160ms unchanged
```

Each place is a cluster of changed pixels within 2px of one another, the largest first: its
size at its place in window coordinates, how many pixels of the frame it has, and how much
the most changed of them changed in a channel, out of 255. The app draws a frame the same
way every time, so a change is never noise. A change at frame 000 is a jump as the motion
starts, `unchanged` partway a stall, a change after the motion should be done a snap at its
end, and one place changing back and forth a flicker. The line marks a change at frame 000
`JUMP`, and a change after the film had held still `SNAP`, and the last line repeats both. A
`JUMP` also holds what the input changes at once, as a click's selection, and a `SNAP` a load
that lands late, so they say where to look, not that something's wrong. Read the lines
before the images, then the sheet, then single frames for a closer look. Film a region: a
whole window's frames are too small in the sheet to read.

To film after inputs of your own, `clock stop` first, before the input that starts the motion,
then `film` without `--`; `clock run` runs the clock again from where it stopped. `tick 8`
moves a stopped clock on by hand, so `move`s and `shot`s can go between ticks.

Motion reads `crate::clock::now()`, never `Instant::now()`, and so must any animation you
write, or it runs on the real time while the rest stands still. What GPUI animates itself
(`with_animation`, `with_spring`, as a fold's chevron and spinners do) and timers keep the
real time, so they jump between frames: a tooltip's delay, and a drag's edge scroll, which
goes on scrolling near a list's top or bottom while the clock is stopped, so keep a filmed drag
clear of the edges. Don't stretch a duration in the code to see a motion slowly.

## Checking a motion

Before filming, write down what each frame may change. Usually that's only the thing that
moves, and as it slides only its edges, as thin strips. Frame 000 may also show what the
input changes at once by design, as a click's selection. Everything else matches the frame
before, and the last frame matches the settled state. Then:

- Read every FILM line. Don't grep for chosen frames or the last line: the change you didn't
  expect is in a frame you left out.
- Account for every place in every line, the `JUMP` and `SNAP` lines first. A place that
  isn't on your list is a bug. Don't report it as inherent, or as a trade-off because another
  mode does it too. Fix it, or ask the user before calling the work done.
- After each fix, check everything that mustn't change, not only what the user pointed at.
  A fix that changes a colour, an opacity or a border under the moving thing breaks the rest.
- Keep the steps for every state in one script and film them all after each change. For a
  mode, that's each kind of thing it takes (a commit, picked commits, a branch, uncommitted
  changes, files) leaving where it started, stepping between targets, and coming back. In
  each still frame, check every mark is still there.
- When the user sends frames, film the same steps and find those frames before changing code.
- Say a motion is fixed only after filming the user's own steps. Put whatever changed outside
  the moving thing first in the reply, not last.

## One control on its own

A change to a shared control in `ui` (a button, switch, checkbox, segmented control) shows
in the stories' window, each in its states side by side, with no repository to make. Pass
`--stories` to `run` or `start`, and `GITBUTLER_GPUI_STORY=<name>` to open on one story; its
toolbar switches light and dark and the interface scale. Add the variant a change makes to
its story in `crates/but-gpui/src/stories` and shoot that.

## Showing a choice

When the user picks between looks, build each one and give them the shots' paths side by
side, not a sketch in text. A sketch hides what they react to: how tall it is, how loud, and
what it covers.

## Measuring performance

- Measure with `run --release`. `perf <label> <input>` times the input until loads settle and
  prints `stall`: the longest the UI thread was busy meanwhile, frames included. Nothing draws
  while it's busy, so keep it under 8.3ms for 120Hz. The harness draws a frame right after the
  input and again once loads land, running `on_next_frame` callbacks first as a display link
  would. `perf startup` counts from launch, so make it the first step.
- The plain and dev-script builds write the same `target/release/gitbutler-gpui`, and the plain
  one ignores the script and hangs. Check with
  `strings target/release/gitbutler-gpui | grep -c GITBUTLER_GPUI_SCRIPT`, and when done rebuild
  the plain one with `cargo build --release -p but-gpui -p gitbutler-git --bins`, so the askpass
  helper is there for pushes.
- Running the binary yourself with `GITBUTLER_GPUI_SCRIPT`, guard it with a kill of your own:
  the script waits for loads without a timeout, quit waits for running operations, and macOS
  has no `timeout`. Kill only the process you started.
- macOS naps an app whose window isn't visible, and after about 30 seconds every step slows by
  2-3x. The dev script opts out with `NSProcessInfo` (`src/dev.rs`). Keep that in any harness
  that drives the app off screen, and distrust a slowdown that starts partway through a run.
- Profile with `sample <pid> <seconds> -file out.txt`. Look at the main thread, mostly idle
  between steps, and read CPU from the self samples, past the wait frames (`__psynch_cvwait`,
  `mach_msg`, `kevent`).
- Stall numbers swing by 30-50% with machine load. Check `uptime`, run the old and new binaries
  in turns, and compare means before calling a change a regression.
- A dev-script build idles at 6-10% CPU, as the stall watcher wakes the UI thread every
  millisecond. Measure idle CPU, memory and power on the plain build.

## Measuring power

- Judge battery by energy, not CPU time or `top`'s estimate: `proc_pid_rusage` with
  `RUSAGE_INFO_V6` reads a process's `ri_energy_nj` and its performance-core time without root.
  Its times are mach ticks, so convert them with `mach_timebase_info`.
- Measure on the plain build with the screen unlocked, in each state a user's window is in:
  frontmost, visible behind another app, and hidden. `osascript` with System Events sets them:
  `set frontmost of` or `set visible of` the process found by its unix id, or activate another
  app. A locked screen makes every app a background one. With an agent writing to a
  repository, the app drew about 500 mW frontmost and 6 mW hidden.

## Notes

- `cargo` must be on the `PATH`. If it isn't, run inside the repository's dev shell:
  `nix develop --command crates/but-gpui/scripts/drive.sh …` (needed for `run` and `start`;
  `step` and `stop` don't build).
- `GITBUTLER_GPUI_APPEARANCE=dark` (or `light`) before the command picks the theme.
- Each run and session starts with app data of its own, so on the GitButler keymap. A
  `keymap tui` step right after `add` picks the TUI's, as Settings › Keyboard would, without
  clicking through Settings.
- `run --release` builds with optimizations, for `perf` and `hold` timings; see Measuring
  performance.
- The window is never key, so `on_focus` and `on_blur` listeners never fire, a rename doesn't
  end on a click elsewhere, and carets don't blink. `on_focus_lost` fires as usual. Check those
  by hand.
- Copy writes the real clipboard, and Settings › AI, or searching settings, reads the keychain.
  Add projects with `add`: Add project's folder picker would open on the user's screen.
- The log target is `but_gpui`, as in `RUST_LOG=but_gpui=debug`.
