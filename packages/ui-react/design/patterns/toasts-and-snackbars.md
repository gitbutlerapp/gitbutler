# Toasts and snackbars

Two ways of saying what just happened; choose by **where the news belongs**,
not how bad it is.

**A snackbar is a sentence next to the thing it is about.** One glyph, one line,
no title, floated over the surface that caused it — `Snackbar.tsx`, ⚛️ Core
node `1706-1682`. News that won't fit in a line read without stopping isn't a
snackbar. Seat it where the operation's own controls stood.

**A toast is a card in the corner of the window.** A title, a description that
can hold real content — a list of rejected paths, an error message — and
buttons, in a 250px stack at the bottom right. It is for news that outlives its
source: a background failure, a half-succeeded operation, an uncaught error.

**Pick by whether the surface is still there.** If the user is in front of the
thing that failed, use a snackbar. If the screen may have moved on, the news
needs the corner and a title; errors from mutations and the React root always
take the corner.

**Pick by whether it needs reading twice.** A snackbar goes after about five
seconds, or early on a click anywhere on it. A toast can hold a paragraph, a
bulleted breakdown and a retry, and waits. Anything to copy, act on or reread
is a toast.

**Nothing routine gets either one.** A success the UI already shows — the
commit in the list, the branch on screen — needs no announcement. Use one only
when the result is invisible, partial, or refused.

**The verdict is carried by the glyph, not the surface.** The three snackbar
variants share ground and border; `info`, `danger` and `safe` differ only in
the leading icon. No colored fill: news that needs more weight is a toast.

**A snackbar's way out is optional; a toast's is not.** Give a snackbar
`onDismiss` only when it stays until dealt with; it then grows a divider and a
close button. One on a timer has none. Toasts always carry Dismiss, plus at
most one action.

**Say it the way the rest of the app says it.** See
[Voice](../content/voice.md). A snackbar is one sentence, no full stop. A toast
title names what happened in a short line — "Some changes were not committed"
— and the description carries the detail.

**Both announce themselves to screen readers, differently.** A snackbar is
`role="status"` and waits its turn, except `danger`, which is `role="alert"`
and interrupts. Toasts get theirs from the toast viewport. A state the user
must act on belongs in the UI itself.
