# What a sidebar row shows

One page to check a row against: what each row of a sidebar tree can show at
its end, in which order, and when one thing takes another's place. The reasons
are in [Sidebar trees](sidebar-trees.md), [Meta lines](meta-lines.md) and
[Conflicts](../patterns/conflicts.md); this page only says what goes where.

**Every row reads the same way.** Start padding (the unread dot), guides and
chevron, glyph, name, then its end: counts in their order, then the next step.
Nothing appears or swaps under the pointer.

## The end, row by row

A row shows a count only when it isn't zero, and only what the rows under it
don't already say.

| Row               | Folded                                                           | Open                                                     |
| ----------------- | ---------------------------------------------------------------- | -------------------------------------------------------- |
| `MachineItem`     | its repositories (or worktrees, grouped by repository), its age  | nothing; as a card's header, no age either               |
| `RepoItem`        | uncommitted, ↑, ↓, its worktrees when several, its age           | one worktree's ↓ and age; nothing over several worktrees |
| `WorktreeItem`    | uncommitted, ↑, ↓, its age                                       | ↓ and its age                                            |
| `UncommittedItem` | how many files changed                                           | (it doesn't fold)                                        |
| `BranchItem`      | conflicts, CI, ↓, ↑ or ⟜, uncommitted, its age; the authors of ↓ | conflicts drop, ⟜ drops; ↓ moves to its heading          |
| `StackCaption`    | what the hidden branches hold: ↑, the latest age, agents         | nothing                                                  |
| `UpstreamItem`    | the authors, ↓                                                   | ↓                                                        |
| `CommitItem`      | a merge: its commits' authors                                    | a commit from someone else: its author                   |

A branch's counts read conflicts, CI, ↓ behind, ↑ unpushed (or ⟜ commits),
uncommitted, then the age. A repository's and a worktree's read uncommitted, ↑,
↓, then the age (see Open questions). An agent working or waiting takes the
age's place.

## The next step

At most one step per row, after the counts, always shown. A step that acts on a
count takes that count's place, as the number and its glyph in a button of the
count's colour.

| Step      | In place of        | On                                                   |
| --------- | ------------------ | ---------------------------------------------------- |
| Update    | ↓, amber           | a repository, a worktree, a folded branch, a heading |
| Push      | ↑, blue            | a branch, a stack                                    |
| Resolve   | (after the counts) | a conflicted branch                                  |
| Create PR | (after the counts) | a pushed branch without a request                    |
| Merge     | (after the counts) | a branch whose checks passed                         |
| Delete    | (after the age)    | a linked worktree whose branches all merged          |

## What overrides what

- **Conflicts beat Push.** A conflicted branch keeps its ↑ as a plain count; its
  step is Resolve.
- **Empty beats everything.** A branch with no commits reads `empty` and nothing
  else: no counts, no step, no unread dot.
- **↑ or ⟜, never both.** A step in ↑'s place counts as ↑.
- **The most folded row takes the pictures and the dot.** Authors and the
  unread dot show on the folded branch, else the folded heading, else the folded
  merge, else on each commit. A face or a dot never shows twice, and unfolding
  reads nothing.
- **Open rows leave counts to the rows under them.** An open branch drops ⟜ and
  conflicts; an open machine or repository card header drops its age.

## Glyph colours

A commit's glyph and the heading over upstream commits take the commit tokens,
in every client; the table is in [Sidebar trees](sidebar-trees.md). Every other
glyph keeps the row's ink, except a pull request's state on its branch.

## The unread dot

In the row's start padding, before the guides, in quiet grey: something on the
row changed since it was last opened. It moves nothing, so the counts line up
whether a row has one or not.

**The dot sits on the most folded row, as the pictures do.** A folded branch
carries it for any new commit inside; open, the dot moves to those commits.
Unfolding reads nothing: a commit is read when it's opened itself, and once a
branch has nothing new inside, its dot goes, folded or not. An empty branch
never has one.

## Never together

Check a drawing or a story against these:

- Push, or ↑, on an empty branch
- Push on a conflicted branch
- ↑ and ⟜ on one row
- the same author's picture on a folded row and on a row under it
- a dot on an open branch for commits that show their own
- Update on an open branch (its heading has it)
- a step that shows only under the pointer
- a ⋯ on a row (its menu opens with a right-click)

## Open questions

- **One count order.** Branch rows put ↓ first; repository and worktree rows put
  uncommitted first. One order for every row would let the counts line up by
  kind down a tree.
- **The end gap of a button.** A button at a row's end sits 8px in, as counts
  do, with 4px above and below. Setting it 4px in would make its gaps even but
  step the end column in and out, and Figma can't draw it without a toggle on
  every row.
