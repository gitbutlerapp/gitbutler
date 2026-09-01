# Lite design notes

The visual language of Lite (`apps/lite/ui`): the conventions you need to make
an on-brand choice without opening Figma. Rules here are about how the UI
should look and read. The tooling that enforces them — scripts, generated
files, commands — lives in `apps/lite/AGENTS.md`.

## Icons

**Source.** Icons come from the ⚛️ Lite Core Figma library. Don't draw new
ones, and don't borrow from 💎 Core or the shared Svelte UI package — those are
a different set for a different app.

**Grid and weight.** Icons are drawn 16×16 on a 16px grid with 1.5px strokes.
Stroke width is constant in screen pixels, so a 16px icon and a 24px icon read
at the same visual weight and sit correctly next to text of any size. Keep
coordinates on the pixel grid; keep all geometry inside the `0 0 16 16` frame.

**Color.** Icons are monochrome and inherit the text color of whatever they sit
in — one asset works in light and dark, in hover and disabled states, and in
accent-colored buttons. Never give an icon a color of its own. If an icon needs
to look different in a state, change the color of its container.

**Sizing.** Size is owned by CSS (`--icon-size`, default 16px), not by the
asset, so an icon scales with the row, button, or type it belongs to. Don't
size an icon by editing the SVG.

**The exception: file icons.** `ui/src/components/file-icons/` holds
language/filetype glyphs that carry their own brand colors (the Rust gear, the
TypeScript square). They are deliberately full-color and are the only icons
that don't inherit `currentColor`. Use them for files and file-shaped things
only — never as general-purpose UI icons.
