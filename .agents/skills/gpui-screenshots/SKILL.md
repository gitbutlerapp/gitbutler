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

Folds, slides and rows moving to a new order are drawn at a clock a script can stop, so any
moment of a motion can be drawn and looked at. `film` films the input after its `--`:

```text
film /tmp/fold 200 8 0 40 340 540 -- click 306 66
```

That stops the clock, saves the frame before the input as `before.png`, runs the input, and
saves a frame every 8ms for 200ms of that region (`000.png`, `001.png`, …), then runs the
clock again. The input is one script line. For a drag, `press` and the first `move`s go before
the `film`, and the `move` that sets the motion off goes after its `--`, so the before frame
already shows the drag under way.

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
end, and one place changing back and forth a flicker. Read the lines before the images, then
the sheet, then single frames for a closer look. Film a region: a whole window's frames are
too small in the sheet to read.

To film after inputs of your own, `clock stop` first, before the input that starts the motion,
then `film` without `--`; `clock run` runs the clock again from where it stopped. `tick 8`
moves a stopped clock on by hand, so `move`s and `shot`s can go between ticks.

Motion reads `crate::clock::now()`, never `Instant::now()`, and so must any animation you
write, or it runs on the real time while the rest stands still. What GPUI animates itself
(`with_animation`, `with_spring`, as a fold's chevron and spinners do) and timers keep the
real time, so they jump between frames: a tooltip's delay, and a drag's edge scroll, which
goes on scrolling near a list's top or bottom while the clock is stopped, so keep a filmed drag
clear of the edges. Don't stretch a duration in the code to see a motion slowly.

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
