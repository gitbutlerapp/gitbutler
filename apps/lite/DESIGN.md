# Lite design notes

The visual language of Lite (`apps/lite/ui`): the conventions you need to make
an on-brand choice without opening Figma. Rules here are about how the UI
should look and read. The tooling that enforces them — scripts, generated
files, commands — lives in `apps/lite/AGENTS.md`.

## Emphasis

**Gray highlights, pop points.** Gray is the workhorse: when a control needs to
read as interactive, or one button needs to sit above its neighbours, give it a
solid gray ground. Pop is the accent, and it is the rarest color in the app —
it doesn't mean "important", it means "this one, out of all of these". Spend it
only when a surface carries many actions and one of them is _the_ action.

**At most one pop per surface.** If two things pop, neither does. A screen that
seems to need a second one usually needs its first one demoted to gray.

**Semantic color is chosen by meaning, not by weight.** Danger, warn and safe
say what a thing _is_, so they sit outside the gray-to-pop ladder entirely — a
danger button can be the only button on screen.

### Button variants

Ghost and outline are the two quiet buttons and sit at the same level as each
other; gray and pop are the two ways to raise one above the rest. Reach for a
quiet one unless there is a reason not to.

- **`ghost`** — no ground, no border. The default, and by far the most used.
  For actions inside something that is already a container: a row, a toolbar,
  a card header, a popup.
- **`outline`** — a ghost with an edge. For a button on open ground, where
  nothing else marks it as a target.
- **`gray`** — solid gray ground. Lifts one button above the ones around it
  without spending color. This is how you highlight; it is not a primary
  action.
- **`pop`** — the accent ground. The primary action of the whole surface, and
  at most one of them. See above.
- **`danger`** — for an act the user cannot take back: deleting, discarding,
  hard-resetting. Chosen by consequence, so it is not part of the ladder.

**Mixing the quiet two.** Ghost and outline can sit side by side in one group,
and the difference then tells kinds of control apart rather than ranking them.
The pull request toolbar does this: Edit and the overflow menu are ghosts, the
Auto-merge toggle is an outline, and Merge is the single pop — the outline
marks the control that holds state, not a louder action. It works in either
direction; whichever of the two is rarer in that group is the one that reads as
distinct. Use it to separate kinds, never to imply one action matters more.

**The inverted pair.** `ghost-inverted` and `outline-inverted` are the same two
buttons for a button sitting on an inverted ground — a selected row, where the
fill flips and the text turns to `--text-1-invert`. They are not a dark-mode
thing; dark mode is handled by the tokens. Note that selected rows reach these
styles through CSS in `Row.module.css` rather than by passing the variant, so
selection can restyle without a re-render.

## Icons

**Source.** Icons come from the ⚛️ Lite Core Figma library. Don't draw new
ones, and don't borrow from 💎 Core or the shared Svelte UI package — those are
a different set for a different app.

**Grid and weight.** Icons are drawn 16×16 on a 16px grid with 1.5px strokes.
Stroke width is constant in screen pixels, so a 16px icon and a 24px icon read
at the same visual weight and sit correctly next to text of any size. Keep
coordinates on the pixel grid; keep all geometry inside the `0 0 16 16` frame.

**Color.** Icons are monochrome and inherit the text color of whatever they sit
in — one asset works in light and dark, in hover and disabled states, and in
accent-colored buttons. Never give an icon a color of its own. If an icon needs
to look different in a state, change the color of its container.

**Sizing.** Size is owned by CSS (`--icon-size`, default 16px), not by the
asset, so an icon scales with the row, button, or type it belongs to. Don't
size an icon by editing the SVG.

**The exception: file icons.** `ui/src/components/file-icons/` holds
language/filetype glyphs that carry their own brand colors (the Rust gear, the
TypeScript square). They are deliberately full-color and are the only icons
that don't inherit `currentColor`. Use them for files and file-shaped things
only — never as general-purpose UI icons.
