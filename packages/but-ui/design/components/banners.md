# Banners

**A banner is a state, not news.** A sentence about a condition that holds
right now, set into the layout above what it affects: live updates paused, a
machine offline, a server that won't answer. It stays while the condition is
true and leaves on its own when it clears, so it has no close button. The
outcome of something the user just did is a snackbar or a toast (see
[Toasts and snackbars](../patterns/toasts-and-snackbars.md)).

**Pick the variant by meaning,** as with [Badges](badges.md): what each is for
is on `Banner`. The text carries the colour, over a pale ground of the same
hue, with no border, and pop, the accent, stands on at most one per surface.

**A title, then at most one more sentence.** The title says the condition. A
second sentence sits under it, in the same colour at regular weight.

**At most one action, and it's the fix:** Retry, Reconnect, Pull. Anything else
the user might do about it belongs on the surface itself.
