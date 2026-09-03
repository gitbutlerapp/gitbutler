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

## Tooltips

**Short.** A tooltip is a label, not a sentence. Aim for two to five words,
sentence case, no full stop. The popup caps at 240px and wraps, but a tooltip
that needs two lines of prose is usually explaining something the UI should
have made obvious on its own — or it belongs in a popup, an empty state, or the
docs.

**Never repeat the trigger's own label.** A tooltip that says "Commit" over a
button reading _Commit_ is noise on every hover. If a control already says what
it does, the tooltip has to add something — a shortcut, the reason it is
disabled, the full value behind a truncation — or not exist. The Commit button
does this literally: its tooltip is disabled while its label is visible and
turns back on only when the button collapses to an icon.

**What a tooltip is for.** Three jobs, and not much else:

- **Naming an icon-only control.** The icon carries the meaning, the tooltip
  spells it out. Use the imperative — "Copy branch name", "Hide form", "Toggle
  line wrapping".
- **Revealing what didn't fit.** The truncated path, the branch name, the
  absolute time behind "3h ago", the counts behind a stats badge. Here the
  tooltip is the value itself, not a description of it.
- **Saying why something is disabled.** A disabled control can't explain
  itself, so its tooltip does: "No changes to commit", "Set up AI in Settings →
  Application → AI". Swap the hint in for the normal tooltip while the reason
  applies.

**Shortcuts go in the `kbd` slot, not the text.** Don't write "Fetch (⌘R)" —
pass the hotkey and let `TooltipPopup` render the keycaps. Pass `kbdScope`
alongside it when the hotkey is bound to a pane: a shortcut that does nothing
from where the user is standing is worse than no shortcut at all.

**A tooltip is never the only way to know.** It needs hover, so it doesn't
exist for keyboard or screen readers, and it's out of reach of touch. An
icon-only button gets an `aria-label` as well — the tooltip repeats that name,
it doesn't supply it. Nothing a user must read to proceed lives only in a
tooltip, and nothing inside one is clickable.

**Say it the way the rest of Lite says it.** Tooltips get the same plain, warm
wording as every other string — the friendly word over git's own term, and the
same wording as the menu item or button elsewhere that does the same thing.

## Empty states

**One component, in ⚛️ Lite Core: "Empty state".** An illustration slot, a
title, a body line, and an actions slot. Its description in Figma carries the
same rules as this section; change one and change the other.

**It is for a surface that is genuinely empty, at rest.** Not a loading state —
"not loaded" is not the same as "nothing to report", and a panel that claims an
emptiness it hasn't checked yet will flash the wrong words on every open. Not a
filter that matched nothing either: that belongs in a line where the list would
be, next to the filter that caused it.

**Centred, and only in a panel with room for it.** A short strip — the
uncommitted list above its commit form — takes a single muted line inset to the
column its rows would occupy, not this. Panels resize, so a centred block needs
`justify-content: safe center`: plain centring pushes the top of the block out
of reach above the scroll origin when the splitter comes down.

**Centred optically, which is not the same as centred.** The block's weight
sits low — two lines of type and a row of buttons under a light illustration —
so centring it on its geometry reads as sitting below the middle. The component
carries 60px of bottom padding to correct for it, and the frontend carries the
same: padding rather than a margin, so that centring the box moves the ink,
lifting what you see by half of what you add. Keep the two in step; if the
block's proportions change, the counterweight is what changes with them.

**The title names the state; the body says what happens next.** One short line
each, sentence case, no full stop. The body's job is the thing the user can't
see — what the next action will do, or the live answer behind the emptiness: a
count, a branch name, a time. "You have 5 branches to pick from" earns its
place; "There is nothing here" repeats the title and the picture both.

**The block caps at 320px.** It is a block, not a banner, and the details pane
it can land in is over a thousand pixels wide. The cap is on the component, so
no host has to remember it, and it is the measure the copy is set in — nothing
inside sets a narrower one.

**Both lines wrap balanced.** They get `text-balance`, and 320 is what gives
them something to balance within. Centred text with a full line and a two-word
orphan under it reads as broken, which is the whole reason for the rule. Figma
has no equivalent, so lines there are broken by hand — the component's
description says so.

**At most two buttons, and never `pop`.** The surface's accent is already spent
on its primary action elsewhere — Start commit sits directly above the stacks
panel — and if two things pop, neither does. Gray marks the likelier of two,
outline takes the other; a button on its own stays outline. Rarer routes to the
same place stay in the panel header's controls rather than crowding the block.

**A button is not always owed.** Where the app handles the state on its own —
committing with no branches creates one — the button is a shortcut and should
read as one, and a body line promising the automatic path shouldn't sit under a
highlighted button arguing the opposite.

## Toasts and snackbars

Two ways of saying what just happened, and the choice between them is about
**where the news belongs**, not how bad it is.

**A snackbar is a sentence next to the thing it is about.** One glyph, one
line, floated over the surface that caused it — `Snackbar.tsx`, ⚛️ Lite Core
node `1706-1682`. It has no title and no room for one: if the news won't fit in
a line the reader can take in without stopping, it isn't a snackbar. The
workspace uses it for a refused operation, seated in the toolbox lane where the
operation's own controls stood, so the answer arrives where the user was
already looking.

**A toast is a card in the corner of the window.** A title, a description that
can hold real content — a list of rejected paths, an error message — and
buttons, in a 250px stack at the bottom right. It's the surface for news that
outlives the place that produced it: a background failure, an operation that
half-succeeded, an uncaught error from anywhere in the app.

**Pick by whether the surface is still there.** If the user is standing in
front of the thing that failed, say it there — a snackbar keeps the cause and
the consequence in one glance. If the news would land on a screen that has
moved on, or the user could reasonably be somewhere else by now, it needs the
corner and it needs a title to say what it is about. Errors from mutations and
from the React root take the corner for exactly this reason: nothing else knows
where they came from.

**Pick by whether it needs reading twice.** A snackbar states an outcome and
goes; five seconds is the workspace's measure, and a click anywhere on it ends
it early. A toast can hold a paragraph, a bulleted breakdown, and a retry, and
it waits. Anything the user may want to copy, act on, or read a second time is
a toast.

**Nothing routine gets either one.** A success the UI already shows — the
commit that appeared in the list, the branch that is now on screen — needs no
announcement. Reach for one of these only when the result is invisible, partial,
or refused.

**The verdict is carried by the glyph, not the surface.** All three snackbar
variants wear the same ground and the same border; `info`, `danger` and `safe`
differ only in the leading icon, so a run of them reads as a row of statements
rather than a traffic light. Don't add a colored fill to make one louder — if
it needs more weight than a line, it needs to be a toast.

**A snackbar's way out is optional; a toast's is not.** Give a snackbar
`onDismiss` only when it will sit there until dealt with — it then grows a
divider and a close button, and the row grows with it. One that leaves on a
timer carries no close button at all: the only close button on screen should
belong to whatever the user still has in hand. Toasts always carry Dismiss,
plus at most one action beside it.

**Say it the way the rest of Lite says it.** Same plain, warm wording as
tooltips and empty states. A snackbar is one sentence, sentence case, no full
stop. A toast title names what happened in a short line — "Some changes were
not committed" — and the description carries the detail; don't split one
thought across the two.

**Both announce themselves to screen readers, differently.** A snackbar is
`role="status"` and waits its turn, except `danger`, which is `role="alert"`
and interrupts. Toasts get theirs from the toast viewport. Neither is the only
way to know something: a state the user must act on belongs in the UI itself,
not in a surface that leaves.
