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

**The inverted pair.** `ghost-inverted` and `outline-inverted` are for a
button on an inverted ground — a selected row, where the text turns to
`--text-1-invert` — not for dark mode, which the tokens handle. A row can apply
these styles through CSS on selection rather than the variant, avoiding a
re-render.
