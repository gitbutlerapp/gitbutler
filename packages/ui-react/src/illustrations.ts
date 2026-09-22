import cactus from "./illustrations/cactus.svg?raw";
import idCard from "./illustrations/id-card.svg?raw";
import looking from "./illustrations/looking.svg?raw";
import papers from "./illustrations/papers.svg?raw";
import terminal from "./illustrations/terminal.svg?raw";
import waving from "./illustrations/waving.svg?raw";

/**
 * The app's illustrations, and the only place they live.
 *
 * Listed by hand rather than globbed and codegenned the way `icons.ts` is:
 * there are few enough that an explicit map is the shorter path to the same
 * checked `name`, and it stays greppable — an illustration nobody renders shows
 * up as an unused key rather than surviving in a generated union.
 *
 * Unlike an icon, an illustration is drawn from three roles rather than one,
 * and each maps to a token so one asset works in both themes:
 *
 * - strokes and solid shapes are `currentColor`, so the container sets them —
 *   `--border-1`, matching the ⚛️ Lite Core library.
 * - enclosed areas are `--bg-1`, the app's paper, so a shape occludes whatever
 *   it overlaps instead of staying white in the dark.
 * - shaded faces are `--bg-2`, the ground these illustrations sit on.
 * - a darker face, where a drawing needs one more step between the ground
 *   and its outline, is `--border-2` — the token ⚛️ Lite Core binds it to.
 *
 * An illustration works on either ground. On `--bg-2` (the sidebar's panels,
 * the details pane's empty state) a `--bg-2` face paints in the ground's own
 * colour and shows only its outline, an open face; on `--bg-1` (a popup's
 * paper, a settings card) the same face reads as a light tint. Both are the
 * drawing as intended, so nothing is checked against its ground. `id-card`'s
 * asterisks are `--text-2`, a step darker than the outline, since they stand
 * for text. `terminal` has no shaded face at all: its screen is solid
 * `currentColor` with the prompt cut out of it in `--bg-1`.
 *
 * Each asset keeps the width and height Figma gave it and renders at that size;
 * `<Illustration width={n} />` overrides it where a surface needs another.
 *
 * Separate from `Illustration.tsx` for the same reason `icons.ts` is separate
 * from `Icon.tsx`: a module that exports anything but components loses fast
 * refresh for the component beside it.
 */
export const illustrations = {
	cactus,
	"id-card": idCard,
	looking,
	papers,
	terminal,
	waving,
} as const;

/** @public */
export type IllustrationName = keyof typeof illustrations;
