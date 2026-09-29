# States

**Every interactive component has a hover and a focus state.** A button, row,
tab, field, menu item or clickable badge: if it responds to a click or a key,
hover says "this reacts" and focus says "the keyboard is here". Hover alone is
invisible to anyone not on a mouse. Neither is optional or a separate ticket.

**Hover is a ground, not a cursor.** The cursor never signals clickability
(see [Cursors](../foundations/cursors.md)). `Button` takes its variant's
`--button-hover-bg`; a list row, a tab and anything else without a fill of its
own `--bg-hover`, or `--bg-hover-invert` on a dark surface. A disabled control
shows no hover.

**Focus is the one ring.** `--focus-ring` (1.5px of `--border-focus`) is the
only focus outline. The global stylesheet puts it on every `button` and `a`
under `:focus-visible`; a component that draws its own — a field, a switch, a
segmented toggle — uses the same token, never a literal or the browser's accent
ring. Buttons and rows use
`:focus-visible`, so a click leaves no ring; a text field uses `:focus`, so a
field being edited looks edited.

**The ring can move, but not vanish.** `outline: none` only when focus is shown
elsewhere — a tree item highlighting its row, a popup handing focus to its
first control — with a comment saying so; otherwise it is a bug.

**Hover and focus transition; they don't snap.** They ride the fast tier (see
[Motion](../foundations/motion.md)). Ground and text changes take
`--transition-button`; a ring takes `outline-color var(--transition-fast)`, a
dim `opacity var(--transition-fast)`. Name the property, never `all`, which
picks up layout and lags. Nothing hover- or focus-related uses the medium tier
except an icon giving way to another icon (see Motion), and nothing writes a
duration by hand.

**List rows snap.** A row's hover and its selection share one ground, and the
selection moves with the arrow keys, so a transition would trail the cursor
down the list. Rows change their ground at once, hover included.
