# Meta lines

The line under a header's title, and a row's meta: small facts, each a glyph
and its words (`MetaPart`) or a glyph and a number (`MetaCount`).

**Colour says what it asks of you.** Amber is work waiting on you (behind,
uncommitted, an agent waiting), blue is work to send (unpushed), green is an
agent at work or a branch this one follows, red is what failed. The rest are
gray.

**A header names what it counts; a row doesn't.** A header has room a sidebar
row lacks, so each part says what it counts, and stays short:

- A count takes one word after the number: 4 unpushed, 3 behind, 12 files,
  2 branches, 1/3 in stack.
- A state with no count is two or three words, a time last: Agent waiting 2m,
  Following branch-b. A plain state that needs nothing, such as Pushed, takes
  no part.
- No sentences, no articles, no "you" or "for". Anything longer goes in the
  part's tooltip.

A row's `MetaCount` keeps only the glyph and the number.

**A narrow pane keeps what the glyph can't say.** In a pane under 640px wide, a
part drops to what its glyph doesn't already say, as a row does: 4 for
4 unpushed, 1/3 for 1/3 in stack, branch-a for Followed by branch-a, 2m for
Agent waiting 2m. A part with no glyph keeps its words, and so does Needs force
push, which its glyph alone doesn't say. The tooltip still says it in full.

**An underline means it opens something.** A part that opens what it describes
is underlined, dotted, in its own colour: Following, Followed by, a branch's
uncommitted files, a machine's last change, a repo's age. A part that only
explains on hover has none.

**Checks describe the latest commit.** Checks passed, Checks running, 2 checks
failed (or the one check's name: test-macos failed). When the branch moved
after they ran, they're Checks outdated, and the tooltip names both commits. A
narrow pane keeps the glyph and a count. A branch with a pull request shows its
checks in the pull request's tab, not on its meta line.

**A rewritten branch needs a force push.** It shows Needs force push in gray,
where unpushed would stand, in both widths; the tooltip says why: rewritten
here, so pushing replaces the remote.

**A machine shows its CLI.** After the uptime on its header: CLI 0.2.1662 in
gray when it matches the latest release, CLI 0.2.1521 outdated in amber when a
newer one is out (a narrow pane drops "outdated"). The tooltip says the latest
version.

**A row says where its work is, only when the page doesn't.** A plain part with
no underline, since the row is what opens the work. Nothing on a worktree or
branch page, the branch on a repo page, repo › branch on a machine page with
more than one repo, and nothing when the title names the place. It stands last
and is what truncates, never a fact before it; a narrow pane keeps the branch.

**A running row says who and what.** Who comes first, since every row there is
running and only the doer differs: the agent's logo and name, or a code glyph
and Command for a check or plain command. What it's doing now comes before the
place: its current command, in mono beside a code glyph. A narrow pane keeps
the logo, and the command's first word or two.
