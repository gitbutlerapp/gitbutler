# Tooltips

**Short.** A label, not a sentence: two to five words, sentence case, no full
stop. The popup caps at 240px and wraps; two lines of prose belong in the UI, a
popup, an empty state, or the docs.

**Never repeat the trigger's own label.** "Commit" over a _Commit_ button is
noise. Add something — a shortcut, why it is disabled, the full value behind a
truncation — or have no tooltip. The Commit button's tooltip is off while its
label is visible and on only when it collapses to an icon.

**What a tooltip is for.** Three jobs, and not much else:

- **Naming an icon-only control.** In the imperative — "Copy branch name",
  "Hide form", "Toggle line wrapping".
- **Revealing what didn't fit.** The truncated path, the branch name, the
  absolute time behind "3h ago", the counts behind a stats badge — the value
  itself, not a description of it.
- **Saying why something is disabled.** "No changes to commit", "Set up AI in
  Settings → Application → AI", in place of the normal tooltip while the reason
  applies; the control stays hoverable with `focusableWhenDisabled`. Only for a
  reason that is one detail of the surface; when it is the whole story, it goes
  in the label — see [Blocked states](../patterns/blocked-states.md).

**Shortcuts go in the `kbd` slot, not the text.** Not "Fetch (⌘R)": pass the
hotkey as `Tooltip`'s `kbd` for keycaps, with `kbdScope` when it is bound to a
pane — a shortcut that does nothing from where the user stands is worse than
none.

**A tooltip is never the only way to know.** Screen readers don't announce it
and touch can't reach it. An icon-only button also gets an `aria-label`, which
the tooltip repeats. Nothing a user must read to proceed lives only in a
tooltip, and nothing inside one is clickable.

**Say it the way the rest of the app says it.** See
[Voice](../content/voice.md): the friendly word, and the wording of the button
that does the same thing.
