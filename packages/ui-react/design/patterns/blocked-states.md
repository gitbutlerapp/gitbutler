# Blocked states

**An empty state is not the answer to a missing step.** Before designing a
"can't do this yet" state, ask whether the app should take the step: a PR from
a never-pushed branch just pushes and then creates, as desktop and the CLI do.
Design the state only when the step is genuinely the user's — committing, say.

**Blocked is not empty.** A surface with content that cannot act yet keeps its
content: the PR form on a branch with no commits still takes a title, a
description and a draft toggle, kept per branch; only its action waits. Nothing
here gets the [empty state](../patterns/empty-states.md) block; not yet gets a
held control that says why; not loaded gets neither.

**A held control says why, and where depends on what else is on the surface.**
When the reason is the surface's whole story, it goes in the label, visible
without hover, in place of the action: "No pull request" on the branch tabs,
"No commits yet" on the form — "No X" or "No X yet", short. When the surface
already shows the situation, a tooltip is enough (see
[Tooltips](../components/tooltips.md)): the merge button blocked by checks
listed right above it. That needs a control that stays hoverable while
disabled — `Button`'s `focusableWhenDisabled`, which `DropdownButton` relies
on; a plain disabled button says it in the label.

**Whether the reason will pass decides the entry point.** Keep a surface
reachable when the user's next ordinary action clears the block: an empty
branch is one commit from a PR, so its tab stays live and the form explains
itself. Disable the entry point only when the reason is permanent for that
view: an unapplied branch cannot open a PR, so its tab segment says so.
