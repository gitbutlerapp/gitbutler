# GPUI client instructions

These apply to `crates/but-gpui`, on top of `crates/AGENTS.md`.

## Fonts

- Use only the font families the app embeds (`assets/fonts`, `theme.ui_font` and
  `theme.mono_font`). GPUI looks a family up among the embedded fonts first and falls back to
  the system only when it isn't there. A family that isn't embedded then resolves to whatever
  the machine has installed. If that's a variable font, like Inter in `~/Library/Fonts`,
  CoreText calls the font daemon for every line it lays out, and frames go from about 10ms to
  70-120ms, only on that machine.
- GPUI picks a weight by choosing between font files and doesn't vary a variable font, so add
  static instances, never a variable file. `assets/fonts/README.md` has the recipe.

## Measuring performance

- Measure release builds with `--features dev-script`. `GITBUTLER_GPUI_SCRIPT` replays input,
  and `perf <label> <input>` times the input until loads settle, then one frame.
- macOS naps an app whose window isn't visible. After about 30 seconds its timers coalesce and
  its threads lose priority, so every step of a long scripted run slows down by 2-3x, though
  nothing in the app changed. The dev script opts out with `NSProcessInfo` (see `src/dev.rs`).
  Keep it in any harness that drives the app off screen, and distrust a slowdown that starts
  partway through a run.
- The harness window is off screen, so macOS draws it only when the script asks. Anything that
  needs a second frame shows in the next step's screenshot, not this one's.
- Profile with `sample <pid> <seconds> -file out.txt`. Look at the main thread: a scripted run
  idles between steps, so most samples there are idle.
