# Markdown

How rendered markdown — a pull request description, a comment — looks in any
app. Lite's kit and renderer are in `apps/lite/DESIGN.md`.

**The rhythm is 12, 16, 4.** 12px between text blocks; 16px around anything
with an edge — a table, a code block, a quote bar, an image, a rule — because a
box lacks text's leading; 4px between list items, nested lists included. A
heading takes 24px above (twice the text gap), 20px for h3 and below, and 8px
below, collapsing into the next block's margin. Margins collapse, so a box next
to a paragraph gets 16, not 28.

**Type.** Body/13 on a 160% line. H1 is 18, H2 16, H3 14, all semibold on a
130% line so a wrapped heading reads as one. Levels four to six stay at body
size, semibold only; Figma has no component for them, and a description that
needs a fourth level needs fewer levels.

**Every link leaves the app, and the arrow says so.** Each is a `TextLink` (see
[Links](../components/links.md)), ending in the arrow hung off the text without
a space. Prose links keep the kit's blue, so their hover is the underline going
solid, not the text lifting.

**A box gets an edge.** Code blocks and inline code sit on `--bg-2`, at
`--radius-card` and `--radius-control` respectively, in mono 12. A blockquote is
a 3px `--border-2` bar with `--text-2` text. An image takes a 1px
`--card-border` ring inset inside `--radius-card` corners, so a white
screenshot has an edge on a white panel, and opens externally on click. A
table's cells are bordered in `--border-3` under a `--bg-2` header row.
