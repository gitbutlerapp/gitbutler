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
- **`info`, `warning`, `success`** — `outline` in a colour: the colour's text,
  an edge of it at 40%, and under the pointer a faint see-through tint
  (`--but-*-outline-hover`). For the next step of a coloured count (see
  [Meta lines](meta-lines.md)). On a light page's selected row, whose solid
  fill is dark, the edge goes to 55%, as `outline-inverted`'s goes to 35% from
  `outline`'s 26%: a faint light line on a dark fill reads fainter than the
  same dark line on a light one. Blue's goes to 70% there, as it reads darker
  than amber and green, so that it matches theirs. A dark page's selection is
  light and keeps 40% and 26%.

**A coloured outline is a count's next step, and only that.** Push beside an
unpushed count is `info`, Update beside a behind count is `warning`, Merge once
the checks pass is `success`: the step takes the colour of what it acts on, so
the two read as one. Anywhere else a button doesn't take a status: going ahead
despite a risk is said in the words around the button, and an act that lands
the work on a surface is pop. Danger takes pop's place rather than standing
beside it, so a surface still has one solid coloured button.

**Two weights of a colour.** Every colour has the same two: soft, a pale tint
(step 3) with coloured text (step 11); and solid (step 9, with its
`foreground`) for checked controls, pop and `danger-solid`. A badge labels and
a button acts, so a pop badge is soft while the pop button is solid.

**Soft comes two ways, by size.** A mark or a control that sits on a ground (a
badge, a status note, the soft `danger` button) takes `--but-*-soft`, which is
see-through: it comes to step 3 on the page and blends with a card or a bar
under it, so it reads as one tint wherever it sits. An area (a banner, a
callout, a tile) is a ground itself and takes `--but-*-surface`, step 3 opaque,
so a whole strip of it stays clean on any ground. Both carry
`--but-*-soft-foreground`.

**Mixing the quiet two.** Ghost and outline side by side tell kinds of control
apart, never rank them; whichever is rarer in the group reads as distinct. In
the pull request toolbar, the overflow menu is a ghost, the Auto-merge toggle
is an outline because it holds state, and Merge is the single pop.

**Sizes.** Buttons on one row share a size, whatever their variant, so the
row reads as one line of controls. A view header's pop action and its `⋯`
menu are both `regular`. An icon-only button is no exception: it takes its
neighbours' size, not a smaller one because it holds less. `medium` is for a
dense header, a card's or a pane's, and for buttons beside a small
`SegmentedControl` or a medium `ToggleGroup`, which share its height. `small`
is for a row denser than that, such as a list row or a file card's header, and
every button in that row is small. Fields stay `regular` whatever their
neighbours: a field shorter than a regular button is too small to type in. So a
button on a field's line is `regular` too, the field's height, never `small`:
Save beside a name, Cancel and Add beside a token, the filter menu beside a
list's filter. One inside the field, as its clear ×, is `small`, within it.

**Icons: what it is before the label, where it goes after.** An icon before
the label names the action — a plus on New branch, a refresh arrow on Refresh
— and is read first, so a toolbar scans by its icons and a stack of buttons
lines them up on one edge. Give a button one only when it helps the eye find
it. An icon after the label says what the click leads to: a down chevron opens
a menu or a picker, a right arrow goes to the next step, an up-right arrow
leaves the app, and a shortcut's `Kbd` sits there too. A button can carry one
of each, as a filter's funnel, its label and a chevron, but never two on one
side, and an icon that names the action never goes after the label.

**The inverted pair.** `ghost-inverted` and `outline-inverted` are for a
button on an inverted ground — a selected row, where the text turns to
`--but-inverse-foreground` — not for dark mode, which the tokens handle. A row can apply
these styles through CSS on selection rather than the variant, avoiding a
re-render.
