# Design notes

The visual language of every GitButler app built on `@gitbutler/ui-react`,
desktop or web: enough to make an on-brand choice without opening Figma. The
rules hold for any app; the examples are GitButler's own surfaces. What one app
decides for itself lives in its own design notes (Lite's are
`apps/lite/DESIGN.md`); the tooling that enforces the rules lives in
`AGENTS.md` beside this file.

The Figma library is ⚛️ Core, the library for every app. 💎 Core is a
different library, for the Svelte desktop app.

## Components

**Build from the library.** Every control users touch — a button, a switch, a
segmented toggle, a popup — already has a component in `@gitbutler/ui-react`
(`packages/ui-react/src/`), with a spec in ⚛️ Core or a Storybook story,
published at <https://master--6ab536f5f40e41db628ccf1b.chromatic.com>. Use it
even when hand-styling in the feature's CSS module would be quicker: a control
styled locally drifts. Look up props and the import rather than guessing, in
the Storybook manifest,
<https://master--6ab536f5f40e41db628ccf1b.chromatic.com/manifests/components.json>,
or its MCP server, `https://master--6ab536f5f40e41db628ccf1b.chromatic.com/mcp`.

**If the library lacks it, think twice, then ask.** Check whether a variant or
prop of an existing component fits — a small size, an icon-only mode — and if
nothing does, ask the designer before building.

**When you can't ask, flag it.** Build the smallest thing that works. The PR
description gets a "New UI" note: what was needed, which library components
were tried and why none fit, and where the new one lives. File a Linear ticket
in GitButler Internal (GB) assigned to Pavel Laptew (@pavel) with that note,
the PR link and a screenshot if you can. If you can't reach Linear, say so in
the note and the PR author files it.

**A custom control needs a reason.** The commit or a comment says what the
library could not do and why that mattered here. Without it, a one-off is a
bug waiting for a redesign.

**New components are documented.** Anything that graduates into
`@gitbutler/ui-react` gets a story and, once the designer has drawn it, a
Figma spec.

## Voice

Every string — tooltips, row hints, empty states, toasts — reads as one person
talking.

**Plain and warm.** The word a colleague would use across a desk: "Forget"
over "Remove credential", "checks your remotes" over "polls the upstream",
"bring them back" over "unarchive" inside a sentence. Git's terms only where
the thing has no other name and the user meets it in git anyway — commit,
branch, worktree, rebase — and never git's phrasing: "linked git worktrees" is
git's, "the repository's other worktrees" is ours.

**Say what it does, not what it is.** A hint starts with the verb — "Shows",
"Checks", "Skips" — and says what happens when the thing is on. A cost goes in
the same breath: "Slows dragging."

**Talk to the user, never about the system.** "before you drop it", "your
remotes", "you have 5 branches". Not "the user", not "the system", and no
passive like "will be shown" that hides who does what.

**Same word for the same thing, everywhere.** Where the button says Archive,
the hint says archived. A path through the app is "Project → Worktrees",
"Settings → AI", never "the project's Worktrees page".

**One thought per string, and no string repeats its neighbour.** A label names
the thing; its hint says what it does; a toast title says what happened and
its description carries the detail.

**Sentence case, and a full stop only where there is a sentence.** Labels,
tooltips, snackbars and the two lines of an empty state are fragments: no full
stop. Hints and toast descriptions are sentences and take one. Nothing takes
an exclamation mark.

**A number is a number.** "5 branches", "3h ago", "1 of 4" — never "several"
or "some" when the count is known.

## Emphasis

**Gray highlights, pop points.** To make a control read as interactive, or
lift one button above its neighbours, give it a solid gray ground. Pop is the
rarest color in the app, meaning "this one, out of all of these": spend it only
when a surface carries many actions and one is _the_ action.

**At most one pop per surface.** If two things pop, neither does; demote the
first to gray before adding a second.

**Semantic color is chosen by meaning, not by weight.** Danger, warn and safe
say what a thing _is_ and sit outside the gray-to-pop ladder: a danger button
can be the only button on screen.

### Button variants

Ghost and outline are the two quiet buttons, at the same level; gray and pop
raise one above the rest. Default to a quiet one.

- **`outline`** — the default; a `Button` with no `variant` is one. For a
  button on open ground where nothing else marks it as a target.
- **`ghost`** — no ground, no border. For actions inside a container: a row, a
  toolbar, a card header, a popup.
- **`gray`** — solid gray ground. Lifts one button above its neighbours
  without spending color. It highlights; it is not a primary action.
- **`pop`** — the accent ground. The primary action of the whole surface, at
  most one.
- **`danger`** — for an act the user cannot take back: deleting, discarding,
  hard-resetting. Chosen by consequence, outside the ladder.

**Mixing the quiet two.** Ghost and outline side by side tell kinds of control
apart, never rank them; whichever is rarer in the group reads as distinct. In
the pull request toolbar, Edit and the overflow menu are ghosts, the Auto-merge
toggle is an outline because it holds state, and Merge is the single pop.

**The inverted pair.** `ghost-inverted` and `outline-inverted` are for a
button on an inverted ground — a selected row, where the text turns to
`--text-1-invert` — not for dark mode, which the tokens handle. A row can apply
these styles through CSS on selection rather than the variant, avoiding a
re-render.

### Links

**A link looks like a link.** Text that opens a page is underlined, in
`--text-2`, the underline at 40% of the text color, going solid on hover as the
text lifts to `--text-1`. Nothing else changes. `TextLink` is that link; the
pull request number in a branch row and in the pull request panel are the
reference.

**Never dress a link as a button.** No ghost or outline wrapper, no ground, no
shared control: a status badge next to a number is two things, the badge and
the link. If it seems to need a button look, it wants a button or a plain
link.

**The exception states its reason.** Where the underline may not work — a link
that is the whole of a card, one inside a line of inline chips — the CSS that
drops it says why in a comment, as for a removed focus ring. A silently
unmarked link is a bug.

**Every link leaves the app, and the arrow says so.** It opens in the browser
from a desktop app, in a new tab from a web app, and ends in an arrow the
height of the text's caps, hung off the text without a space so the underline
stops at the word. `TextLink` draws it inline at a 1px stroke and nothing else
should; ↗ isn't typed because the text fonts don't carry it at every weight.

## States

**Every interactive component has a hover and a focus state.** A button, row,
tab, field, menu item or clickable badge: if it responds to a click or a key,
hover says "this reacts" and focus says "the keyboard is here". Hover alone is
invisible to anyone not on a mouse. Neither is optional or a separate ticket.

**Hover is a ground, not a cursor.** The cursor never signals clickability
(see Cursors). `Button` takes its variant's
`--button-hover-bg`, a list row `--list-item-hover-bg`, anything else a gray
wash at `--opacity-bg-hover`. A disabled control shows no hover.

**Focus is the one ring.** `--focus-ring` is the only focus outline. The global
stylesheet puts it on every `button` and `a` under `:focus-visible`; a
component that draws its own — a field, a switch, a segmented toggle — uses the
same token, never a literal or the browser's accent ring. Buttons and rows use
`:focus-visible`, so a click leaves no ring; a text field uses `:focus`, so a
field being edited looks edited.

**The ring can move, but not vanish.** `outline: none` only when focus is shown
elsewhere — a tree item highlighting its row, a popup handing focus to its
first control — with a comment saying so; otherwise it is a bug.

**Hover and focus transition; they don't snap.** They ride the fast tier (see
Motion). Ground and text changes take `--transition-button`; a ring takes
`outline-color var(--transition-fast)`, a dim
`opacity var(--transition-fast)`. Name the property, never `all`, which picks
up layout and lags. Nothing hover- or focus-related uses the medium tier except
an icon giving way to another icon (see Motion), and nothing writes a duration
by hand.

## Radius

**Nested corners are concentric.** Outer radius equals inner radius plus the
padding between: a card at `--radius-card` with 4px of padding holds a control
at `--radius-card` minus 4px, not the same radius or a token picked for the
control alone. Radius tokens come from ⚛️ Core; when the subtraction doesn't
land on one, compute it with `calc()` from the outer token and say so, rather
than eyeballing a literal.

## Minimums

**A hit area is never under 16px.** A control can draw smaller — a chevron, a
close cross, a diff line number — but responds to at least 16px on each side.
Extend the target with padding or a pseudo-element, not by growing the glyph,
and don't let two extended targets overlap.

**11px is for small UI, and nothing goes smaller.** Badges, counts, tags,
keycaps and a file's status letter can set 11px; labels, body text, captions
and hints are 12px or more. Nothing goes below 11px: if a ⚛️ Core token is
smaller, the token is wrong. Something that only works under 11px should be a
tooltip, an icon, or left out.

## Line breaks

**No runts, no widows.** A line ends where the sentence lets it, not where the
box ran out. Cutting the copy, `text-wrap: pretty` and `text-wrap: balance` are
all fine; pick by the text and the room around it.

**Judge by the gap.** A hint that overruns its measure by two words wanted to
be one line: cut it. One well into a second line can stay two, evened by
`balance`, unless that leaves two short lines against a wide gap. When neither
copy nor wrap mode fills the space, reword the text or move it.

**Where the wrap is set.** Row hints set `pretty`, keeping a single word off
its own line; empty states set `balance`, since centred text reads best as two
even lines. Change it per surface, never globally.

## Cursors

**The host picks the cursor: the arrow on the desktop, the hand on the web.**
A desktop app keeps the arrow over buttons, menus and rows, and may offer the
hand as a setting. A web app takes the hand always.

**One property carries the choice.** The host sets `--control-cursor` on its
root — a web app to `pointer`, a desktop app from its setting — and loads
`control-cursor.css`, which applies it in one rule to buttons, links,
`summary`, `select`, a `label` that owns a control, and the roles Base UI
renders as a span or div: button, checkbox, switch, radio, tab, option and the
menu items. It also gives a disabled control `not-allowed`, so no component
does.

**Components don't choose a cursor.** No `cursor: pointer`, no pinned
`cursor: default`, and no reintroducing the hand by resetting a `<button>` —
its browser default is already the arrow. A clickable outside that list (a
list row, a folded card, a minimap badge, a diff line number) takes
`cursor: var(--control-cursor)` itself. Interactivity is shown by hover (see
States).

**Gesture cursors are the exception.** They change regardless of the host:
`text` over editable text, `grab` and `grabbing` while dragging, and the resize
cursors on a splitter.

## Motion

**Two speeds, both tokens.** Every duration comes from design-core.
`--transition-fast` (80ms) is for a control changing state in place: hover and
press on a button, a focused field's outline, a small opacity fade.
`--transition-medium` (150ms) is for something that moves or changes shape: a
switch thumb, a chevron turning, a section folding, the minimap fading in. No
hand-written durations; if neither tier fits, add a tier in Figma.

**Popups are the medium tier with a curve.** `--transition-popup` aliases
medium, and `--easing-popup` is the one tokenised curve: a hard ease-out that
lands without overshoot, so a modal, dropdown or popover arrives rather than
drifts in. They always go together —
`transform var(--transition-popup) var(--easing-popup)` — and a modal's
backdrop, a sibling that can't inherit, takes the same pair. Popups close the
way they open.

**Easings are keywords.** Outside popups nothing names a curve: the tiers ride
the browser's default `ease`, and a place that wants another shape writes
`ease-out` after the duration. Don't tokenise `ease`; it would export as a
longer cubic-bezier that says less. In Figma, `ease` is a custom bezier of
0.25, 0.1, 0.25, 1; Ease in, Ease out and Ease in and out match the CSS
keywords of the same name.

**Feel comes from the curve before the tier.** A medium transition that seems
slow wants `ease-out`, not the fast tier.

**Loops and holds are not transitions.** The spinner and the fresh-change
pulse are keyframe animations with their own timing; the pause before a
"Copied" label reverts is a delay in code. Neither takes a token.

**Anything that moves respects reduced motion.** A fold that changes height
turns its transition off under `prefers-reduced-motion: reduce`. A hover color
needs no such rule.

**An icon that becomes another icon crossfades.** Copy becoming a tick, plus
becoming a check, a placeholder becoming a camera under the pointer: both icons
stay in the DOM, one over the other (a shared grid cell or an absolutely
positioned wrapper), each transitioning `opacity, scale, filter` on the medium
tier with `ease-out`. The leaving one shrinks to `scale(0.25)`, fades to `0`
and blurs to `4px`; the arriving one does the reverse. It is a transition,
not a keyframe, so it reverses cleanly mid-swap, for result and hover swaps
alike.

**The rules live in two places.** The token descriptions in ⚛️ Core carry the
same tiers and pairings; change one and change the other.

## Icons

**Source.** Icons come from the ⚛️ Core Figma library. Don't draw new ones, and
don't borrow from 💎 Core or the shared Svelte UI package, a different set for
the Svelte desktop app.

**Grid and weight.** Icons are drawn 16×16 on a 16px grid with 1.5px strokes,
constant in screen pixels, so a 16px and a 24px icon read at the same weight.
Keep coordinates on the pixel grid and geometry inside the `0 0 16 16` frame.

**Color.** Icons are monochrome and inherit the text color around them, so one
asset works in light, dark, hover, disabled and accent buttons. Never give an
icon its own color; to change it in a state, change its container's.

**Sizing.** Size is owned by CSS (`--icon-size`, default 16px), not the asset.
Don't size an icon by editing the SVG.

**The exception: file icons.** `packages/ui-react/src/file-icons/` holds
language/filetype glyphs in their brand colors (the Rust gear, the TypeScript
square), the only icons that don't inherit `currentColor`. Use them for files
and file-shaped things only.

**A person without a picture gets their glitch, never a blank.** `Avatar` and
`ProfileImage` are one design at two sizes: the person's picture, else the
real Gravatar photo for their email, else their glitch — a quiet pattern of big
blocks in their colour's next step on their colour, both picked from their
email or login so a person looks the same everywhere. The glitch also shows
while a picture loads. No generated faces: Gravatar URLs from the server and
the backend ask for the real photo only.

## Tooltips

**Short.** A label, not a sentence: two to five words, sentence case, no full
stop. The popup caps at 240px and wraps; two lines of prose belong in the UI, a
popup, an empty state, or the docs.

**Never repeat the trigger's own label.** "Commit" over a _Commit_ button is
noise. Add something — a shortcut, why it is disabled, the full value behind a
truncation — or have no tooltip. The Commit button's tooltip is off while its
label is visible and on only when it collapses to an icon.

**What a tooltip is for.** Three jobs, and not much else:

- **Naming an icon-only control.** In the imperative — "Copy branch name",
  "Hide form", "Toggle line wrapping".
- **Revealing what didn't fit.** The truncated path, the branch name, the
  absolute time behind "3h ago", the counts behind a stats badge — the value
  itself, not a description of it.
- **Saying why something is disabled.** "No changes to commit", "Set up AI in
  Settings → Application → AI", in place of the normal tooltip while the reason
  applies; the control stays hoverable with `focusableWhenDisabled`. Only for a
  reason that is one detail of the surface; when it is the whole story, it goes
  in the label — see Empty states.

**Shortcuts go in the `kbd` slot, not the text.** Not "Fetch (⌘R)": pass the
hotkey as `Tooltip`'s `kbd` for keycaps, with `kbdScope` when it is bound to a
pane — a shortcut that does nothing from where the user stands is worse than
none.

**A tooltip is never the only way to know.** Screen readers don't announce it
and touch can't reach it. An icon-only button also gets an `aria-label`, which
the tooltip repeats. Nothing a user must read to proceed lives only in a
tooltip, and nothing inside one is clickable.

**Say it the way the rest of the app says it.** See Voice: the friendly word,
and the wording of the button that does the same thing.

## Fields

**A form field has a label.** Always, above the field — "Personal access
token", "Signing key", "Account email" — for every value the user has to think
about: settings, credentials, the integration and signing forms. `Field.tsx`
has `FieldLabelStyles` for the label and `FieldControlStyles` for the input.

**A name already on the surface is not given twice.** Each field needs one name
the eye and the screen reader both find. A field at the end of a settings row
is named by the row's label — "Description", "Auto-fetch frequency" — tied with
`htmlFor`; so is one under a table column heading, or in a card whose title
names its single field. `FieldLabelStyles` is for fields nothing else names, as
in forms that stack several in one strip.

**The placeholder shows the shape, when the shape needs showing.** If the user
could get the format wrong — a token, a key fingerprint, a custom API URL, a
path to a signing program — show a valid value: `GLPAT-XXXXXXXXXXXXXXXXXX`,
`723CCA3AC13CF28D`, `https://api.openai.com/v1`. An email, a name or a branch
name needs none; leave the field empty.

**A placeholder is not a label.** It vanishes once the user types and a screen
reader never has it: "Account email" as a placeholder with nothing above is a
missing label. Nothing the user needs lives only there; a token's scope, where
to generate it, what happens on save go in a hint under the field.

**An example is a value, not a caption.** A token's prefix and length, a key's
fingerprint, a full URL, a path — not the label again ("Enter your token") or a
description in words. A saved secret shows dots (`••••••••`), since its value
never comes back.

**The exceptions are fields that are the surface.** A search box, a filter row,
the command palette, a comment or reply composer: the field is the whole
control, so the placeholder does the talking — "Search for branches…", "Filter
files", "Write a reply…" — with an `aria-label` for the name. Once such a field
sits in a form with a save button, it is a form field and gets its label.

## Empty states

**One component, in ⚛️ Core: "Empty state".** An illustration slot, a title, a
body line, and an actions slot. Its Figma description carries the same rules;
change one and change the other.

**It is for a surface that is empty, once the app knows it is.** Never a
loading state.

**A filter that matched nothing is empty too, and says so.** In a panel with
room — the branches tab — it takes the block: the title names the miss, the
body quotes what missed (the search, the filters, or both), and the one action
shows everything again, since the filters live in a native menu the block
can't point at. A short strip keeps the line where its rows would be, next to
the filter. A picker's list takes `PopupEmpty`: the `papers` drawing over one
line reporting the miss, no title, the line closer under the drawing, no
counterweight. A list empty before anything was typed gets the cactus and says
what that means — "Nothing left to apply" — not "found". A dropdown no wider
than its trigger, like the commit target combobox, keeps the plain line.

### Illustrations

**Each one means something; pick it by meaning,** never for variety:

| Illustration | Size    | Means                                                                                   |
| ------------ | ------- | --------------------------------------------------------------------------------------- |
| `cactus`     | 96×82   | A list with nothing in it: no branches, no machines, nothing yet                        |
| `papers`     | 98×86   | Looked and found nothing, or a state the app can't name: a search, a filter, an address |
| `id-card`    | 130×100 | Signing in, accounts, identity                                                          |
| `terminal`   | 79×59   | The command line                                                                        |
| `waving`     | 186×215 | Good news in a large view: all good, nothing to do. Large views only                    |

**Always at 1:1.** Never scale one. With no room, leave the drawing out; the
block works without it.

**`waving` is the big one, and the happy one.** It needs a large view — a
details pane or a whole page — and good news. Never in a sidebar, a popup or
anything narrow, and never for a miss or an error: those take `papers`.

**Never a stand-in that looks like content.** Gray avatar circles and text bars
at rest read as stuck loading. Say what fills the section with a control — the
PR panel's Reviewers and Labels show an "Add reviewers" button — which a
skeleton never shows.

**Centred, and only in a panel with room for it.** A short strip — the
uncommitted list above its commit form — takes a single muted line inset to the
column its rows would occupy instead. The component centres itself in the
height it is given, falling back to the top when the panel is too short so its
head stays above the scroll origin. The host gives it that height: a pane that
fills its column.

**An empty section inside a page takes the same block.** A settings list with
nothing in it — no active worktrees, a feature turned off — uses `EmptyState`
without the illustration, framed on the recessed ground so it reads as the
page's state, not another row, with the counterweight off. There is no second
empty pattern: no title-and-hint card in a page's own CSS.

**Centred optically, which is not the same as centred.** The block's weight
sits low, so the component and the frontend both carry 60px of bottom padding
(padding, not margin, so centring lifts the ink by half). Keep the two in step
with the block's proportions.

**The title names the state; the body says what happens next.** One short line
each, sentence case, no full stop. The body gives what the user can't see: what
the next action will do, or the live answer behind the emptiness — a count, a
branch name, a time. "You have 5 branches to pick from", not "There is nothing
here".

**The block caps at 320px.** It is a block, not a banner. The cap is on the
component and is the measure for the copy; nothing inside sets a narrower one.

**Both lines wrap balanced.** They get `text-balance` within the 320 measure.
Figma has no equivalent, so lines there are broken by hand.

**At most two buttons, and never `pop`.** The surface's accent is spent
elsewhere. Gray marks the likelier of two, outline the other; a lone button is
outline. Rarer routes stay in the panel header's controls.

**A button is not always owed.** Where the app handles the state itself —
committing with no branches creates one — any button is a shortcut and is not
highlighted.

**An empty state is not the answer to a missing step.** Before designing a
"can't do this yet" state, ask whether the app should take the step: a PR from
a never-pushed branch just pushes and then creates, as desktop and the CLI do.
Design the state only when the step is genuinely the user's — committing, say.

**Blocked is not empty.** A surface with content that cannot act yet keeps its
content: the PR form on a branch with no commits still takes a title, a
description and a draft toggle, kept per branch; only its action waits. Nothing
here gets the block; not yet gets a held control that says why; not loaded gets
neither.

**A held control says why, and where depends on what else is on the surface.**
When the reason is the surface's whole story, it goes in the label, visible
without hover, in place of the action: "No pull request" on the branch tabs,
"No commits yet" on the form — "No X" or "No X yet", short. When the surface
already shows the situation, a tooltip is enough (see Tooltips): the merge
button blocked by checks listed right above it. That needs a control that stays
hoverable while disabled — `Button`'s `focusableWhenDisabled`, which
`DropdownButton` relies on; a plain disabled button says it in the label.

**Whether the reason will pass decides the entry point.** Keep a surface
reachable when the user's next ordinary action clears the block: an empty
branch is one commit from a PR, so its tab stays live and the form explains
itself. Disable the entry point only when the reason is permanent for that
view: an unapplied branch cannot open a PR, so its tab segment says so.

## Toasts and snackbars

Two ways of saying what just happened; choose by **where the news belongs**,
not how bad it is.

**A snackbar is a sentence next to the thing it is about.** One glyph, one line,
no title, floated over the surface that caused it — `Snackbar.tsx`, ⚛️ Core
node `1706-1682`. News that won't fit in a line read without stopping isn't a
snackbar. Seat it where the operation's own controls stood.

**A toast is a card in the corner of the window.** A title, a description that
can hold real content — a list of rejected paths, an error message — and
buttons, in a 250px stack at the bottom right. It is for news that outlives its
source: a background failure, a half-succeeded operation, an uncaught error.

**Pick by whether the surface is still there.** If the user is in front of the
thing that failed, use a snackbar. If the screen may have moved on, the news
needs the corner and a title; errors from mutations and the React root always
take the corner.

**Pick by whether it needs reading twice.** A snackbar goes after about five
seconds, or early on a click anywhere on it. A toast can hold a paragraph, a
bulleted breakdown and a retry, and waits. Anything to copy, act on or reread
is a toast.

**Nothing routine gets either one.** A success the UI already shows — the
commit in the list, the branch on screen — needs no announcement. Use one only
when the result is invisible, partial, or refused.

**The verdict is carried by the glyph, not the surface.** The three snackbar
variants share ground and border; `info`, `danger` and `safe` differ only in
the leading icon. No colored fill: news that needs more weight is a toast.

**A snackbar's way out is optional; a toast's is not.** Give a snackbar
`onDismiss` only when it stays until dealt with; it then grows a divider and a
close button. One on a timer has none. Toasts always carry Dismiss, plus at
most one action.

**Say it the way the rest of the app says it.** See Voice. A snackbar is one
sentence, no full stop. A toast title names what happened in a short line —
"Some changes were not committed" — and the description carries the detail.

**Both announce themselves to screen readers, differently.** A snackbar is
`role="status"` and waits its turn, except `danger`, which is `role="alert"`
and interrupts. Toasts get theirs from the toast viewport. A state the user
must act on belongs in the UI itself.

## Markdown

How rendered markdown — a pull request description, a comment — looks in any
app. Lite's kit and renderer are in `apps/lite/DESIGN.md`.

**The rhythm is 12, 16, 4.** 12px between text blocks; 16px around anything
with an edge — a table, a code block, a quote bar, an image, a rule — because a
box lacks text's leading; 4px between list items, nested lists included. A
heading takes 24px above (twice the text gap), 20px for h3 and below, and 8px
below, collapsing into the next block's margin. Margins collapse, so a box next
to a paragraph gets 16, not 28.

**Type.** Body/13 on a 160% line. H1 is 18, H2 16, H3 14, all semibold on a
130% line so a wrapped heading reads as one. Levels four to six stay at body
size, semibold only; Figma has no component for them, and a description that
needs a fourth level needs fewer levels.

**Every link leaves the app, and the arrow says so.** Each is a `TextLink` (see
Links), ending in the arrow hung off the text without a space. Prose links keep
the kit's blue, so their hover is the underline going solid, not the text
lifting.

**A box gets an edge.** Code blocks and inline code sit on `--bg-2`, at
`--radius-card` and `--radius-button` respectively, in mono 12. A blockquote is
a 3px `--border-2` bar with `--text-2` text. An image takes a 1px
`--card-border` ring inset inside `--radius-card` corners, so a white
screenshot has an edge on a white panel, and opens externally on click. A
table's cells are bordered in `--border-3` under a `--bg-2` header row.
