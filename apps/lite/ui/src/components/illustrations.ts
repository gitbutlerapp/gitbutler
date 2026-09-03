import cactus from "./illustrations/cactus.svg?raw";
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
 *
 * A `--bg-2` face therefore paints in the ground's own colour and shows only
 * its outline — an open face, not a filled one, which is what the drawing
 * intends. It is a real dependency on the surface: put one of these on `--bg-1`
 * and every open face closes up into a tint. Both hosts today are `--bg-2` (the
 * sidebar's panels and the details pane's empty state), so an illustration for
 * anywhere else wants checking against its ground first.
 *
 * Each asset keeps the width and height Figma gave it and renders at that size;
 * `<Illustration width={n} />` overrides it where a surface needs another.
 *
 * Separate from `Illustration.tsx` for the same reason `icons.ts` is separate
 * from `Icon.tsx`: a module that exports anything but components loses fast
 * refresh for the component beside it.
 */
export const illustrations = { cactus, waving } as const;

/** @public */
export type IllustrationName = keyof typeof illustrations;
