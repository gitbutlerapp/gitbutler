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
- **`pop`** — the accent, solid, as a checked checkbox or switch is. The
  primary action of the whole surface, at most one.
- **`danger`** — the soft red ground, for an act the user cannot take back:
  deleting, discarding, hard-resetting. Chosen by consequence, outside the
  ladder.
- **`danger-solid`** — danger at full strength, for the answer to a
  confirmation that destroys something (see [Modals](../patterns/modals.md)).

**Danger is the only status a button takes.** Warning, success, info and done
colour badges, banners and labels, never a button: going ahead despite a risk
is said in the words around the button, and an act that lands the work is
pop. Danger takes pop's place rather than standing beside it, so a surface
still has one coloured button.

**Two weights of a colour.** Every colour has the same two: soft (step 5, with
its `soft-foreground`) for badges, banners, tiles and the soft `danger`
button; and solid (step 9, with its `foreground`) for checked controls, pop
and `danger-solid`. `-text` is coloured text on a neutral ground, never on a
soft one.

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
`--but-inverse-foreground` — not for dark mode, which the tokens handle. A row can apply
these styles through CSS on selection rather than the variant, avoiding a
re-render.
