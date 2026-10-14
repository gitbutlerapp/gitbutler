/**
 * The spacing a diff's file cards are laid out with, in pixels: `top` above
 * the first card, `inset` either side, `gap` between cards, `bottom` after
 * the last. In one place so every app spaces its diffs alike, `DiffFileList`
 * and a virtualizing host (Lite's CodeView) both. A bar above the list lines
 * up with the cards by insetting its content by `inset` too.
 */
export const diffFileSpacing = { top: 8, inset: 9, gap: 10, bottom: 12 } as const;

/**
 * The height of a `DiffFileHeader` in its card, borders included. The card
 * holds a stuck header in place over this much of its end (see `DiffFile`),
 * and a virtualizing host (Lite's CodeView) reserves it for each header.
 */
export const diffFileHeaderHeight = 38;
