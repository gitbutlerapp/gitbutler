# Toasts and snackbars

Two ways of saying what just happened; choose by **where the news belongs**,
not how bad it is.

**A snackbar is a sentence next to the thing it is about.** One glyph, one line,
no title, floated over the surface that caused it — `Snackbar.tsx`, ⚛️ Core
node `1706-1682`. News that won't fit in a line read without stopping isn't a
snackbar.

**A toast is a card in the corner of the window.** A glyph, a title, a
description that can hold real content — a list of rejected paths, an error
message — at most one action under it, and a close button, in a 320px stack at
the bottom right — `Toasts.tsx`, ⚛️ Core node `2805-61892`. It is for news that
outlives its source: a background failure, a half-succeeded operation, an
uncaught error.

**Toasts sit bottom right, snackbars bottom centre.** Every app keeps the same
two places, so news is always where the reader last saw it: toasts stack in the
window's bottom-right corner, 16px in, and snackbars stack at the bottom centre,
16px up. Neither moves to follow what caused it.

**Raise snackbars through `Snackbars`.** `Snackbars.tsx` is the stack, drawn in
⚛️ Core only as the single `Snackbar` it repeats: render it once with a manager
of its own and raise one with `add({ title, type })`. They sit one below
another, 8px apart, the newest at the bottom; each rises in and sinks out while
the rest close the gap, and past three the oldest recedes until there is room. A
busy one is a `loading` snackbar, updated to its result. A host with its own
furniture at the bottom centre, as Lite's operation toolbox, lifts the stack
above it with `--snackbars-bottom` rather than letting the two overlap.

**Pick by whether the surface is still there.** If the user is in front of the
thing that failed, use a snackbar. If the screen may have moved on, the news
needs the corner and a title; errors from mutations and the React root always
take the corner.

**Pick by whether it needs reading twice.** A snackbar goes after about five
seconds, or early on a click anywhere on it. The pointer or keyboard focus on
the stack holds it and starts the five seconds over; switching to another
window doesn't, so news from before the switch is gone on the way back. A toast can hold a paragraph, a
bulleted breakdown and a retry, and waits. Anything to copy, act on or reread
is a toast.

**A second sentence makes it a toast.** A snackbar is one line. When the news
carries a sentence that explains it — where the thing went, how to get it back —
that sentence is a description, and the news is a toast with a title: "Archived
bench-tweaks", then "It shows under Archived until it has new work";
"Unapplied bench-tweaks", then how to apply it again.

**Work out of sight gets a busy snackbar; work in sight gets nothing.** An
action that runs where the user can't watch it — on another machine, in the
background — shows a snackbar with a spinner while it runs ("Rewording on
macbook…"), and that same snackbar turns into the result ("Commit reworded")
before it goes. It stays for as long as the work runs, then leaves like any
other. A change made here that the screen shows at once — the commit in the
list, the branch on screen — needs neither. Lite's absorb does the first
("Absorbing…", then "Changes absorbed into 3 commits"). A failure with a raw
error leaves the snackbar for a toast; see below.

**The verdict is carried by the glyph, not the surface.** The four snackbar
variants share ground and border; `info`, `warning`, `danger` and `safe` differ
only in the leading icon. `warning` is for an act that ran with a catch — it
went through, but not quite as asked. No colored fill: news that needs more
weight is a toast. Toasts follow the same rule through `add({ type })`: `info`,
`warning` (an act that came off only in part), `error` and `success`, the names
Base UI's toast manager already carries.

**A snackbar's way out is optional; a toast's is not.** Give a snackbar
`onDismiss` only when it stays until dealt with; it then grows a divider and a
close button. One on a timer has none. Toasts always carry a close button,
labelled Dismiss, plus at most one action. The action sits under the
description rather than beside the close button, so its label can say what it
does without squeezing the words.

**A raw error is copied, not read.** When the only detail is what the backend
returned, the toast's title says what failed and its one action is "Copy error
message"; the error itself never shows under the title, and never in a code
block. The same goes for a view that failed to render: its boundary names what
couldn't be shown, with Retry and "Copy error message".

**Say it the way the rest of the app says it.** See
[Voice](../content/voice.md). A snackbar is one sentence, no full stop. A toast
title names what happened in a short line — "Some changes were not committed"
— and the description carries the detail.

**Both announce themselves to screen readers, differently.** A snackbar on its
own is `role="status"` and waits its turn, except `danger`, which is
`role="alert"` and interrupts. In the `Snackbars` stack, as toasts in theirs,
the viewport announces it instead. A state the user
must act on belongs in the UI itself.

**A state that holds is a banner, not news.** Live updates paused, a machine
offline, a server that won't answer: none of these happened once, they are
true until they clear. Set a `Banner` into the layout above what it affects,
for as long as the condition lasts, with the fix as its one action. It has no
close button; it leaves when the condition does.
