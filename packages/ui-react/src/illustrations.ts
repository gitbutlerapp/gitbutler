import cactus from "./illustrations/cactus.svg?raw";
import idCard from "./illustrations/id-card.svg?raw";
import papers from "./illustrations/papers.svg?raw";
import sleep from "./illustrations/sleep.svg?raw";
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
 *   `--but-border-hover`, matching the 🍑 But UI library.
 * - enclosed areas are `--but-background`, the app's paper, so a shape occludes whatever
 *   it overlaps instead of staying white in the dark.
 * - shaded faces are `--but-muted`, the ground these illustrations sit on.
 *
 * An illustration works on either ground. On `--but-muted` (the sidebar's panels,
 * the details pane's empty state) a `--but-muted` face paints in the ground's own
 * colour and shows only its outline, an open face; on `--but-background` (a popup's
 * paper, a settings card) the same face reads as a light tint. Both are the
 * drawing as intended, so nothing is checked against its ground. `id-card`'s
 * asterisks are `--but-muted-foreground`, a step darker than the outline, since they stand
 * for text. `terminal` has no shaded face at all: its screen is solid
 * `currentColor` with the prompt cut out of it in `--but-background`.
 *
 * 🍑 But UI's Illustrations section is the source of truth: each colour here is
 * the token Figma binds there, and a change is made in Figma first. Each asset
 * keeps the width and height Figma gave it and renders at that size, never
 * scaled.
 *
 * Separate from `Illustration.tsx` for the same reason `icons.ts` is separate
 * from `Icon.tsx`: a module that exports anything but components loses fast
 * refresh for the component beside it.
 */
export const illustrations = {
	cactus,
	"id-card": idCard,
	papers,
	sleep,
	terminal,
	waving,
} as const;

/** @public */
export type IllustrationName = keyof typeof illustrations;
