# Sidebar trees

The sidebar's tree: a machine, its repositories, their worktrees, and the
branches, stacks and commits in them, one row a line.

**Levels group the rows, never lines between them.** A repository's worktrees
sit one after another with nothing between them: each worktree's glyph starts
its group, and its rows hang one level under it. A stack's branches sit one
level under its `StackCaption`, with nothing around them either.

**A row folds one way.** A row that holds others always has a glyph, a plain
dot when it names nothing of its own, and its chevron takes the glyph's place
under the pointer and on keyboard focus. A row that holds nothing has no
chevron. A spinner or a conflict's warning keeps its place under the pointer:
it says what the pointer came to see. A file tree's folders fold the same way.

**Guides say how the rows hang together.** One guide a level ties each row to
the row it sits under. The last row under its parent ends that guide with a
round tail into its glyph, and the rows below it drop the guide. A git object
that isn't last, a stack, a branch or a commit, meets the guide with a short
stub; a row that only groups others, such as a worktree or its uncommitted
changes, doesn't. A row can't see the rows after it, so a host works every
row's guides out with `treeGuides`, from the rows it shows, never by hand; a
file tree takes them the same way.
