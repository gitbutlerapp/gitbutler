# Menus

**A menu is two columns.** The labels, as wide as the longest, then what
closes the rows (a shortcut, a count, a submenu's chevron), as wide as the
widest of those, 16px apart. A row with nothing at its end still keeps the
second column clear, so a long label never runs under another row's
shortcut.

**A menu's choice is ticked at the row's start.** A `MenuRadioGroup` ticks its
current row in a gutter before the row's glyph, and every row and heading in
that menu steps in by the gutter so the labels start on one line. A row's end
is the shortcut's, and a submenu's chevron's: a tick there would push the
shortcut of the row it marks out of line with the rest, and sit far from the
word it marks. A menu with no choice in it has no gutter.

**A select's list is ticked at the row's end.** `Select`'s list opens over its
field with the chosen row where the field's text was, so a row's glyph and
label sit where the field shows them and the tick sits where the field's
chevron was. Its rows have no shortcuts or submenus to make room for, and a
gutter would push the list out past the field's left edge.

So which side a tick goes on follows what opened the list, not what the choice
is: a theme picked from a header's menu is ticked at the start, the same theme
picked from a settings row's field at the end.

The window's panel layout menu, Menu's `PanelLayout` story, is the example: a
choice of four layouts, two of them with shortcuts.
