# Empty states

**One component, in ⚛️ Core: "Empty state".** An illustration slot, a title, a
body line, and an actions slot. Its Figma description carries the same rules;
change one and change the other.

**It is for a surface that is empty, once the app knows it is.** Never a
loading state. A surface that has content but can't act yet is not empty
either; see [Blocked states](../patterns/blocked-states.md).

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

## Illustrations

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
