import cactus from "./illustrations/cactus.svg?raw";

/**
 * The app's illustrations, and the only place they live.
 *
 * Listed by hand rather than globbed and codegenned the way `icons.ts` is:
 * there are few enough that an explicit map is the shorter path to the same
 * checked `name`, and it stays greppable — an illustration nobody renders shows
 * up as an unused key rather than surviving in a generated union.
 *
 * Unlike an icon, an illustration is drawn from two roles rather than one. Its
 * strokes are `currentColor`, so the container sets them; its enclosed areas
 * are `--bg-1`, the app's paper, so the shape occludes whatever it overlaps and
 * follows the theme rather than staying white in the dark.
 *
 * Separate from `Illustration.tsx` for the same reason `icons.ts` is separate
 * from `Icon.tsx`: a module that exports anything but components loses fast
 * refresh for the component beside it.
 */
export const illustrations = { cactus } as const;

/** @public */
export type IllustrationName = keyof typeof illustrations;

/** Every illustration, for the catalogue story. */
export const illustrationNames = Object.keys(illustrations) as Array<IllustrationName>;
