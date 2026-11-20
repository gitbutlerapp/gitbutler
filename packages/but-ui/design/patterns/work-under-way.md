# Work under way

One page to pick the mark that says something is running. Each says who is at
work and what it asks of you, so a list with several running reads at a
glance. How they move is in [Motion](../foundations/motion.md); work the user
can't watch gets a snackbar, in
[Toasts and snackbars](toasts-and-snackbars.md). All five side by side, moving,
are `MetaCount`'s Work under way story.

## The marks

| Mark                  | Component                   | It says                                  | On                                                |
| --------------------- | --------------------------- | ---------------------------------------- | ------------------------------------------------- |
| Spinner, in the glyph | `Icon` `spinner`            | an action you started here is running    | a row's `busy`, a busy snackbar                   |
| Amber spinner         | `MetaStatus` `working`      | a result you'll look at is on its way    | checks, a deploy, a sync                          |
| Robot, moving, grey   | `MetaCount` `agent-working` | an agent is at work; nothing for you     | a branch, a worktree, a terminal, a header's line |
| Robot, still, amber   | `MetaCount` `agent-waiting` | an agent is waiting for you              | the same places                                   |
| Dots, stepping, grey  | `MetaCount` `running`       | another program is at work in a terminal | a terminal's row                                  |

**Pick by who is at work.** You: the spinner in the row's glyph, where your
action runs. An agent: the robot. Any other program in a terminal: the dots,
the terminal's own spinner, braille's dots stepping round, as the GPUI client
draws it. A check or a deploy: the amber spinner, the one mark that waits for a
verdict.

**Colour says what it asks of you, as in [Meta lines](../components/meta-lines.md).** Work
that needs nothing from you is grey, so a list of busy agents and builds stays
quiet and the one waiting for you is the amber in it. The amber spinner is for
a result you'll act on, as a failing check is.

**A mark says it once.** A row shows one: its own action's spinner, or what its
agent or program is at, never both. When the work ends, the mark makes way for
its outcome (✓, ✕, or nothing), and a terminal's outcome stays until you open
it ([What a sidebar row shows](../components/sidebar-rows.md)).

## Never together

- two marks on one row for one piece of work
- the amber spinner for work that needs nothing from you, as an agent or a build
- the robot for a program that isn't an agent
- a spinner on each row a busy snackbar already covers
