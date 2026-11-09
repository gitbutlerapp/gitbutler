# Markdown

How rendered markdown — a pull request description, a comment — looks in any
app. `Markdown` draws it; these are its rules, and 🍑 But UI's Markdown section
draws the same.

**The rhythm is 12, 16, 4.** 12px between text blocks; 16px around anything
with an edge — a table, a code block, a quote bar, an image, a rule — because a
box lacks text's leading; 4px between list items, nested lists included. A
heading takes 24px above (twice the text gap), 20px for h3 and below, and 8px
below, collapsing into the next block's margin. Margins collapse, so a box next
to a paragraph gets 16, not 28.

**Text reads narrow; media spreads wide.** Headings, paragraphs, lists, quotes
and code keep to `--but-size-prose` (620px), a comfortable line. Tables, images,
galleries and video take `--but-size-prose-wide` (820px), centred on the same axis,
so they break out of the text on both sides.

**Type.** Text is 14 on a 160% line: but-ui's `text-prose`, Figma's
Markdown/Prose. H1 is 20, H2 18, H3 16, all semibold on a 130% line so a
wrapped heading reads as one: `text-h1`, `text-h2` and `text-h3`, Figma's
Markdown/H1–H3. A table stays at 13, as every `Table` is: its cells are rows,
not sentences. Levels four to six stay at body size, semibold only; Figma has
no component for them, and a description that needs a fourth level needs fewer
levels.

**Any reading text takes the same style.** A description in a modal, an empty
state or a toast, a component's notes in these docs: if it is sentences, it is
`text-prose`, not a size paired with `text-body`. Labels, rows and other single
lines keep their size class on the default line.

**Every link leaves the app, and the arrow says so.** Each is a `TextLink` (see
[Links](../components/links.md)), ending in the arrow hung off the text without
a space. Prose links keep the kit's blue, so their hover is the underline going
solid, not the text lifting.

**A box gets an edge.** Code blocks and inline code sit on `--but-muted` in a 1px
`--but-border-subtle` ring inset, at `--but-radius-card` and `--but-radius-control`
respectively, in mono 12; the ring gives code the same edge as a card beside it,
and keeps it from vanishing on a `--but-muted` panel. A blockquote is
a 3px `--but-border` bar with `--but-muted-foreground` text. An image takes a 1px
`--but-border-subtle` ring inset inside `--but-radius-card` corners, so a white
screenshot has an edge on a white panel, and opens externally on click. A
table is a Table, as anywhere else in the app: the card `DescriptionList`
draws, column names in `--but-muted-foreground`, a hairline under each row but the last.
