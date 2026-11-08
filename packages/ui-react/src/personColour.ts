/**
 * A person's colour and initial, both picked from a seed (a login, an email), so the same
 * person looks the same in every surface and on every machine. The colour is one of the
 * theme's hues, so it follows light and dark like everything else.
 */

/**
 * The colours a person can be: each the theme's 70 step, a mid tint in light and a deep one
 * in dark, so `--but-foreground` reads on all of them. Written out whole, so the token check can see
 * every one.
 */
const COLOURS = [
	"var(--scale-pop-70)",
	"var(--scale-purple-70)",
	"var(--scale-safe-70)",
	"var(--scale-warn-70)",
	"var(--scale-danger-70)",
] as const;

/** FNV-1a: spreads a short login across all 32 bits. */
const hash = (seed: string): number => {
	let h = 2166136261;
	for (let i = 0; i < seed.length; i++) {
		h ^= seed.charCodeAt(i);
		h = Math.imul(h, 16777619);
	}
	return h >>> 0;
};

/** The person's colour. */
export const personColour = (seed: string): string =>
	COLOURS[hash(seed) % COLOURS.length] ?? COLOURS[0];

/**
 * The person's initial: the seed's first letter or digit, upper-cased, so `krlvi` is K and
 * `pavel@gitbutler.com` is P. Empty when the seed has neither.
 */
export const personInitial = (seed: string): string =>
	(seed.match(/[\p{L}\p{N}]/u)?.[0] ?? "").toUpperCase();
