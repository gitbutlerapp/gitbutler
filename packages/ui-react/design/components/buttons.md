# Buttons

Ghost and outline are the two quiet buttons, at the same level; gray and pop
raise one above the rest (see [Emphasis](../foundations/emphasis.md)). Default
to a quiet one.

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

**Sizes.** Buttons on one row share a size, whatever their variant, so the
row reads as one line of controls. A view header's pop action and its `⋯`
menu are both `regular`. An icon-only button is no exception: it takes its
neighbours' size, not a smaller one because it holds less. `small` is for a
row denser than a toolbar, such as a list row or a file card's header, and
every button in that row is small.

**The inverted pair.** `ghost-inverted` and `outline-inverted` are for a
button on an inverted ground — a selected row, where the text turns to
`--text-1-invert` — not for dark mode, which the tokens handle. A row can apply
these styles through CSS on selection rather than the variant, avoiding a
re-render.
