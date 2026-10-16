# Cursors

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
[States](../foundations/states.md)).

**Gesture cursors are the exception.** They change regardless of the host:
`text` over editable text, `grab` and `grabbing` while dragging, and the resize
cursors on a splitter.
