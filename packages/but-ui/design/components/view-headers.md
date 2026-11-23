# View headers

The header of a details view: what is shown, a line about it, and the buttons
for it. What goes on the line is in [Meta lines](meta-lines.md).

**The buttons stand at the end,** level with the meta line, and above the tabs
when the view has them. The menu comes last: ghost and icon-only. Every button
in the row shares one size (see [Buttons](buttons.md)).

**Primary actions get buttons, three at most; the rest go in the `⋯` menu.**
A primary action is one someone comes to this view to take: Send to on a
branch, a worktree or a machine; Merge on a pull request; Apply to workspace on
a branch outside it. A view can have more than one, but no more than three:
three read at a glance and leave the title its room, while a fourth turns the
header into a toolbar where nothing stands out, and it's the first thing a
narrow pane has to cut. Every secondary act goes in the menu: the rarer ones
(Rename, Copy name, Open on GitHub) and every act that can't be taken back
(Delete, Discard). A view with no primary action has the menu alone.

**A switch that holds a setting stays beside them.** Auto-merge isn't an act
but a state the view is in, so it stands before the primary actions, where it
can be read without opening the menu. It takes a button's room, so it counts
toward the three.

**The menu is the thing's context menu.** The same one its row opens on a
right-click (see [Context menus](../patterns/context-menus.md)), so it lists
the primary actions too, and the keyboard reaches everything through it.

**A count's next step isn't a header button.** Pushing what's unpushed or
updating what's behind stands on the meta line, in the count's place (see
[Meta lines](meta-lines.md)).

**A button shows only while its act can happen.** Apply is gone once the
branch is applied, not held disabled. Holding is for an act the next ordinary
step brings back, and it says why: Merge waiting on checks (see
[Blocked states](../patterns/blocked-states.md)).

**A commit's message unfolds from its title,** through a small icon button
just after it, rather than taking room until it's asked for.

**Tabs only when there's more than one thing to show.** A branch with a pull
request has Diff and its pull request's tab; without one, it has no tabs.
