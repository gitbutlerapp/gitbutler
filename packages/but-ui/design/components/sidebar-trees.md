# Sidebar trees

The sidebar's tree: a machine, its repositories, their worktrees, and the
branches, stacks and commits in them, one row a line.

What each row shows at its end, and what overrides what, is in one table:
[What a sidebar row shows](sidebar-rows.md).

**Levels group the rows, never lines between them.** A repository's worktrees
sit one after another with nothing between them: each worktree's glyph starts
its group, and its rows hang one level under it. A stack's branches sit one
level under its `StackCaption`, with nothing around them either.

**A card's header stands apart.** The row that heads a card, a repository's
`RepoItem` or a machine's `MachineItem`, sits in the same block as the rows
under it, with a `RowDivider` between: a faint line inset from the rows' edges,
not a header section ruled off across the card. The parts of one card divide
the same way, as a machine's repositories or a repository's machines. The rows
under a header start at the top level, not a step in: the line says they
belong to it, so no guide hangs from its chevron. Folded, the card is the
header alone, with no line.

**A line across the card parts a drawer's groups.** A drawer's card, as the
hidden projects or the terminals, rules each group off from the next across the
whole card. Inside a card, a line is always a `RowDivider`, so the two never
read as the same.

**A row folds one way.** A row that holds others shows its chevron, always, in
a column of its own before its glyph, and the guide of the rows under it hangs
from the chevron's middle. A row that could hold others and holds nothing yet,
such as a branch without commits, keeps a faint dot in the chevron's place, so
it lines up with the rows that fold. A row that never holds others, a commit or
uncommitted changes, has its glyph under its parent's glyph instead, 6px after
its last guide, as a glyph sits 6px after a chevron; at the top level, with no
parent, its glyph takes the chevrons' column. A commit sits on its branch's
line instead (below). A file tree's folders and files line up the same way.

**Guides say how the rows hang together.** One guide a level ties each row to
the row it sits under. The last row under its parent ends that guide with a
round tail into its glyph, and the rows below it drop the guide. The other rows
leave the guide plain, with no stub toward them. A row can't see the rows after it, so a host works every
row's guides out with `treeGuides`, from the rows it shows, never by hand; a
file tree takes them the same way.

**A chain sits on its line.** A stack's branches build on each other, and so do
a branch's commits, so they don't hang off their parent's guide as side-by-side
rows do: each sits on it as a node (`treeGuides`' `chained`), and the line ends
at the last one's mark, with no tail. A stack's branch is a 6px dot there, with
no number or label, and keeps its own dot or chevron and glyph after it. A
commit's mark takes its glyph's place on its branch's line: hollow while it's
only here, solid once pushed, in its commit token, or its merge or warning
glyph, 7px. The line stops 2px short of each mark either side, in the guides'
one colour, so every connector in the tree is the same line. A remote's new
commits, under their heading, keep the tree's guides.

**The row's end says where it stands.** Its counts sit there and stay, and its
next step follows them, always shown, never only under the pointer. A step that
acts on a count takes that count's place, as its number and glyph in a small
button of its colour: Push for ↑, Update for ↓. The rest of what the row can do
is in its menu, which opens with a right-click, never from a ⋯
([Context menus](../patterns/context-menus.md)).
A row that changed since it was last opened ends in a dot after its counts.
Once a tree tracks that, every row keeps the dot's room, so the counts line
up down the list.

**A row in a split says so at its very end.** Every row a split shows, the one
it was made on and each whose tile is in it, carries its `SplitPreviewMap`: a
small map of the tiles with that row's own inked, in a cell of its own after
the counts and the next step. While a tree shows any split, every row keeps that
cell's room, as with the unread dot, so the maps line up down the list and so
do the counts. Under the pointer it opens a `SplitPreview`
of what each tile shows, by name and state, never their content, beside the
sidebar rather than over it, and the split's other rows light up (`linked`), so
the rows that go together read as one. It is a mark, not a button: a row itself
opens the split again.

**A commit's glyph takes its state's token, in every client, always.** React,
GPUI and 🍑 But UI draw a commit's glyph from its commit token in the table
(GPUI's `theme.commit_local` and its kin, Figma's `commit/local` and its kin),
never from a role, a ramp or a hex, even
when the token resolves to the same colour today. Each token aliases the role
its count takes, so a commit and the count above it agree, and a state's colour
changes in that one token, for every client at once, with no search for the
places that draw it.

| Commit                      | Glyph         | Token                     | Resolves to                                                      |
| --------------------------- | ------------- | ------------------------- | ---------------------------------------------------------------- |
| Only here, not pushed       | `commit`      | `--but-commit-local`      | `--but-info-mark`, blue: work to send, as ↑                      |
| Pushed                      | `commit-fill` | `--but-commit-remote`     | `--but-muted-foreground`, the row's glyph                        |
| On its remote, not here yet | `commit-fill` | `--but-commit-upstream`   | `--but-warning-mark`, amber, as ↓                                |
| Integrated into the target  | `commit-fill` | `--but-commit-integrated` | `--but-purple-mark`, as a merged request                         |
| Conflicted, pushed or not   | `warning`     | `--but-warning-mark`      | amber, a conflict's mark ([Conflicts](../patterns/conflicts.md)) |

**What a branch's remote has comes in under a heading.** "New on origin" sits one
level under the branch, over its own commits, with the incoming commits under
it: the remote alone in its name, the whole ref in its tooltip. Its glyph takes
`--but-commit-upstream` too, so the heading and its commits change together, and
Update takes its count's place. A merge among them takes `branch-merge` in
`--but-commit-upstream`.

**Who sends them shows once, on the row that hides them.** A commit from someone
else shows their picture at the row's end, never in the glyph's place. A folded
row stacks the pictures of what it hides, after its counts: a folded branch, a
folded heading, a folded merge. Open, it leaves them to the rows under it, so a
face never shows twice.

**The target's new commits aren't rows.** The repository's ↓ says how many, with
Update in its place, and a page lists them. The tree lists the work in the
workspace, not the target's.
