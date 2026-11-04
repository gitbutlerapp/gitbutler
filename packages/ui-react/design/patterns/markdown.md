# Markdown

How rendered markdown — a pull request description, a comment — looks in any
app. `Markdown` draws it; these are its rules, and ⚛️ Core's Markdown section
draws the same.

**The rhythm is 12, 16, 4.** 12px between text blocks; 16px around anything
with an edge — a table, a code block, a quote bar, an image, a rule — because a
box lacks text's leading; 4px between list items, nested lists included. A
heading takes 24px above (twice the text gap), 20px for h3 and below, and 8px
below, collapsing into the next block's margin. Margins collapse, so a box next
to a paragraph gets 16, not 28.

**Text reads narrow; media spreads wide.** Headings, paragraphs, lists, quotes
and code keep to `--size-prose` (540px), a comfortable line. Tables, images,
galleries and video take `--size-prose-wide` (740px), centred on the same axis,
so they break out of the text on both sides.

**Type.** Body/13 on a 160% line. H1 is 20, H2 18, H3 16, all semibold on a
130% line so a wrapped heading reads as one: design-core's `text-h1`,
`text-h2` and `text-h3`, Figma's Markdown/H1–H3. Levels four to six stay at body
size, semibold only; Figma has no component for them, and a description that
needs a fourth level needs fewer levels.

**Every link leaves the app, and the arrow says so.** Each is a `TextLink` (see
[Links](../components/links.md)), ending in the arrow hung off the text without
a space. Prose links keep the kit's blue, so their hover is the underline going
solid, not the text lifting.

**A box gets an edge.** Code blocks and inline code sit on `--bg-2` in a 1px
`--border-section` ring inset, at `--radius-card` and `--radius-control`
respectively, in mono 12; the ring gives code the same edge as a card beside it,
and keeps it from vanishing on a `--bg-2` panel. A blockquote is
a 3px `--border-2` bar with `--text-2` text. An image takes a 1px
`--border-section` ring inset inside `--radius-card` corners, so a white
screenshot has an edge on a white panel, and opens externally on click. A
table is a Table, as anywhere else in the app: the card `DescriptionList`
draws, column names in `--text-2`, a hairline under each row but the last.
