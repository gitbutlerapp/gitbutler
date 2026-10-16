# Minimums

**A hit area is never under 16px.** A control can draw smaller — a chevron, a
close cross, a diff line number — but responds to at least 16px on each side.
Extend the target with padding or a pseudo-element, not by growing the glyph,
and don't let two extended targets overlap.

**11px is for small UI, and nothing goes smaller.** Badges, counts, tags,
keycaps and a file's status letter can set 11px; labels, body text, captions
and hints are 12px or more. Nothing goes below 11px: if a ⚛️ Core token is
smaller, the token is wrong. Something that only works under 11px should be a
tooltip, an icon, or left out.
