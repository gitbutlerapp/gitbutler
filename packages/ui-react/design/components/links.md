# Links

**A link looks like a link.** Text that opens a page is underlined, in
`--text-2`, the underline at 40% of the text color, going solid on hover as the
text lifts to `--text-1`. Nothing else changes. `TextLink` is that link; the
pull request number in a branch row and in the pull request panel are the
reference.

**Never dress a link as a button.** No ghost or outline wrapper, no ground, no
shared control: a status badge next to a number is two things, the badge and
the link. If it seems to need a button look, it wants a button or a plain
link.

**The exception states its reason.** Where the underline may not work — a link
that is the whole of a card, one inside a line of inline chips — the CSS that
drops it says why in a comment, as for a removed focus ring. A silently
unmarked link is a bug.

**Every link leaves the app, and the arrow says so.** It opens in the browser
from a desktop app, in a new tab from a web app, and ends in an arrow the
height of the text's caps, hung off the text without a space so the underline
stops at the word. `TextLink` draws it inline at a 1px stroke and nothing else
should; ↗ isn't typed because the text fonts don't carry it at every weight.
