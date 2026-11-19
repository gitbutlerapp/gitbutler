# Sidebar trees

The sidebar's tree: a machine, its repositories, their worktrees, and the
branches, stacks and commits in them, one row a line.

**Levels group the rows, never lines between them.** A repository's worktrees
sit one after another with nothing between them: each worktree's glyph starts
its group, and its rows hang one level under it. A stack's branches sit one
level under its `StackCaption`, with nothing around them either.

**A row folds one way.** A row that holds others shows its chevron, always, in
a column of its own before its glyph, and the guide of the rows under it hangs
from the chevron's middle. A row that could hold others and holds nothing yet,
such as a branch without commits, keeps a faint dot in the chevron's place, so
it lines up with the rows that fold. A row that never holds others, a commit or
uncommitted changes, has its glyph under its parent's glyph instead, 6px after
its last guide, as a glyph sits 6px after a chevron; at the top level, with no
parent, its glyph takes the chevrons' column. A file tree's folders and files
line up the same way.

**Guides say how the rows hang together.** One guide a level ties each row to
the row it sits under. The last row under its parent ends that guide with a
round tail into its glyph, and the rows below it drop the guide. The other rows
leave the guide plain, with no stub toward them. A row can't see the rows after it, so a host works every
row's guides out with `treeGuides`, from the rows it shows, never by hand; a
file tree takes them the same way.

**The row's end says where it stands.** Its counts sit there and stay, and its
next step follows them, always shown, never only under the pointer. A step that
acts on a count takes that count's place, as its number and glyph in a small
button of its colour: Push for ↑, Update for ↓. The rest of what the row can do
is in its menu, which opens with a right-click, never from a ⋯
([Context menus](../patterns/context-menus.md)).
A row that changed since it was last opened ends in a dot after its counts.
Once a tree tracks that, every row keeps the dot's room, so the counts line
up down the list.
