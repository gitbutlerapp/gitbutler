# Lite design notes

The shared design language is `packages/ui-react/DESIGN.md`; read it first.
These are the choices Lite makes for itself, as a desktop app, and where it
keeps what the shared rules ask for.

## Cursors

**The arrow, with the hand as a setting.** Lite ships with the arrow, and the
_Hand cursor_ switch in Appearance turns the hand on for whoever wants it. The
app flips `data-hand-cursor` on the document from the setting, and
`global.css` maps it to `--control-cursor: pointer`.

## Focus

**`global.css` carries the ring.** It puts `--focus-ring` on every `button`
and `a` under `:focus-visible`, so a plain control gets it for free.

## Selected rows

**Inverted buttons come through CSS.** A selected row reaches
`ghost-inverted` and `outline-inverted` through `Row.module.css` rather than
by passing the variant, so selection restyles without a re-render.

## Links

**Every link opens in the browser**, never in Lite itself.

## Snackbars

**The workspace seats a refused operation's snackbar in the toolbox lane**,
where the operation's own controls stood, and clears it after five seconds.

## Markdown

**One kit, on the Client file's ⚙️ Meta page: the `Markdown/`
components.** A component per block — Heading with its three levels,
Paragraph, List and List item, Link, Inline code, and Block, which wraps a
code block, blockquote, table, image or rule chosen by its swap — and
"Markdown / slot", whose default content is a sample description built from
them. `ui/src/components/Markdown.tsx` renders it, and `Markdown.stories.tsx`
renders the same sample document, so compare the two when either side
changes. Each component's description in Figma names the CSS selector it
stands for; change one and change the other.

**Figma spacing.** The shared rhythm (12, 16, 4) is the slot's 12px gap plus
each block's own padding: 4 on Block, 12 on H2, 8 on H3. Figma writes the
link arrow as ↗.

**Inline pieces Figma can't run.** Inline code, keycaps and folds sit inside a
text line in the app. Figma has no way to flow a chip through text, so the
kit's sample puts the chip between two text nodes in a row. That is a limit of
the mockup, not a layout: don't design around where the chip breaks.

**The measure is 480.** A description is set at the details pane's width, the
story sets the same, and every block component in the kit is 480 wide.
