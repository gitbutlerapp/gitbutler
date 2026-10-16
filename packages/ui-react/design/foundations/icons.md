# Icons

**Source.** Icons come from the ⚛️ Core Figma library. Don't draw new ones, and
don't borrow from 💎 Core or the shared Svelte UI package, a different set for
the Svelte desktop app.

**Grid and weight.** Icons are drawn 16×16 on a 16px grid with 1.5px strokes,
constant in screen pixels, so a 16px and a 24px icon read at the same weight.
Keep coordinates on the pixel grid and geometry inside the `0 0 16 16` frame.

**Color.** Icons are monochrome and inherit the text color around them, so one
asset works in light, dark, hover, disabled and accent buttons. Never give an
icon its own color; to change it in a state, change its container's.

**Sizing.** Size is owned by CSS (`--icon-size`, default 16px), not the asset.
Don't size an icon by editing the SVG.

**The exception: file icons.** `packages/ui-react/src/file-icons/` holds
language/filetype glyphs in their brand colors (the Rust gear, the TypeScript
square), the only icons that don't inherit `currentColor`. Use them for files
and file-shaped things only.

**A person without a picture gets their glitch, never a blank.** `Avatar` and
`ProfileImage` are one design at two sizes: the person's picture, else the
real Gravatar photo for their email, else their glitch — a quiet pattern of big
blocks in their colour's next step on their colour, both picked from their
email or login so a person looks the same everywhere. The glitch also shows
while a picture loads. No generated faces: Gravatar URLs from the server and
the backend ask for the real photo only.
