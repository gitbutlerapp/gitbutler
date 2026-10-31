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
or later with `sh` steps, which run while the app watches. To try a real repository, use an
APFS clone as `crates/but-gpui/AGENTS.md` (Sandboxes and fixtures) says.

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

## Animations

Folds, slides and rows moving to a new order are drawn at a clock a script can stop. Stopped,
steps and waits no longer move them on, so any moment of a motion can be drawn and looked at:

```text
clock stop                         # first: the input starts the motion at t=0
click 306 66                       # fold a project
film /tmp/fold 200 8 0 40 340 540  # a frame every 8ms for 200ms, of that region
clock run                          # back to the real time, from where it stopped
```

`film` saves each frame as `shot` would (`000.png`, `001.png`, …), all of them in a grid in
`sheet.png`, a row at a time, and prints what changed in each from the frame before, its
bounds in window coordinates:

```text
FILM 004 t=32ms changed 22.5% in 10,166 319x371
FILM 019 t=152ms changed 0.0% in 10,80 6x6
FILM 020 t=160ms unchanged
```

Read those lines before the images: a spike is a jump, `unchanged` partway a stall, a change
after the motion should be done a snap at its end, and one region changing back and forth a
flicker. Then read the sheet, and single frames for a closer look. Film a region: a whole
window's frames are too small in the sheet to read. `tick 8` moves the stopped clock on by
hand, so `move`s and `shot`s can go between ticks, as a drag in motion needs.

Motion reads `crate::clock::now()`, never `Instant::now()`, and so must any animation you
write, or it runs on the real time while the rest stands still. What GPUI animates itself
(`with_animation`, `with_spring`, as a fold's chevron and spinners do) and timers, as a
tooltip's delay, keep the real time, so they jump between frames. Don't stretch a duration in
the code to see a motion slowly.

## Notes

- `cargo` must be on the `PATH`. If it isn't, run inside the repository's dev shell:
  `nix develop --command crates/but-gpui/scripts/drive.sh …` (needed for `run` and `start`;
  `step` and `stop` don't build).
- `GITBUTLER_GPUI_APPEARANCE=dark` (or `light`) before the command picks the theme.
- `run --release` builds with optimizations, for `perf` and `hold` timings; see Measuring
  performance in `crates/but-gpui/AGENTS.md`.
- The window is never key, so `on_focus` and `on_blur` listeners never fire and carets don't
  blink. Check those by hand.
- Copy writes the real clipboard, and Settings › AI reads the keychain. Add projects with
  `add`: Add project's folder picker would open on the user's screen.
- Scripted runs in `crates/but-gpui/AGENTS.md` has the rest: hover needs a second small
  `move`, links in text need `press` then `release`, prompts need their buttons clicked, and
  drags have their own commands.
