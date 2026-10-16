/**
 * A person's colour and the pattern on it, both picked from a seed (a login, an email), so
 * the same person looks the same in every surface and on every machine. The colour is one
 * of the theme's hues, so it follows light and dark like everything else.
 */

/**
 * The colours a person can be: each the theme's 80 step as the ground and its 70 step for
 * the pattern. Written out whole, so the token check can see every one.
 */
const COLOURS = [
	{ ground: "var(--scale-pop-80)", shade: "var(--scale-pop-70)" },
	{ ground: "var(--scale-purple-80)", shade: "var(--scale-purple-70)" },
	{ ground: "var(--scale-safe-80)", shade: "var(--scale-safe-70)" },
	{ ground: "var(--scale-warn-80)", shade: "var(--scale-warn-70)" },
	{ ground: "var(--scale-danger-80)", shade: "var(--scale-danger-70)" },
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

/** mulberry32: small, fast, and the same sequence for the same seed everywhere. */
const random = (seed: number): (() => number) => {
	let state = seed;
	return () => {
		state = (state + 0x6d2b79f5) | 0;
		let t = Math.imul(state ^ (state >>> 15), 1 | state);
		t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
};

/**
 * The person's colour: `ground` at the theme's 80 step, a pale tint in light and a deep one
 * in dark, and `shade` one step further for the pattern, close enough to stay quiet under
 * whatever sits on top.
 */
export const personColour = (seed: string): { ground: string; shade: string } =>
	COLOURS[hash(seed) % COLOURS.length] ?? COLOURS[0];

/** The pattern's grid: coarse, so each block is 9px on a 72px picture and never busy. */
export const GLITCH_SIZE = 8;

/**
 * A glitch in the person's colour, as a path on a `GLITCH_SIZE` square, symmetric about its
 * vertical middle: bands of rows, each its own width, growing from the middle out, with a
 * stray block or two, all drawn on the left half and mirrored. It grows from the middle
 * because the circle crops the corners: a shape grown from the edges all but vanishes at
 * 14px. The blocks are large, so it reads as a texture rather than a picture of its own.
 */
export const glitchPattern = (seed: string): string => {
	const r = random(hash(`${seed}#glitch`));
	const int = (n: number) => Math.floor(r() * n);
	const size = GLITCH_SIZE;
	const half = size / 2;
	// The left half only, row by row; the right is its mirror.
	const cells = new Uint8Array(half * size);

	// Bands of one to three rows, each torn to a width of its own.
	let y = 0;
	while (y < size) {
		const band = 1 + int(3);
		// Never the full row, so both tones always show in every band.
		const width = [1, 1, 2, 2, 3, 3][int(6)] ?? 2;
		for (let row = y; row < Math.min(size, y + band); row++)
			for (let k = 0; k < width; k++) cells[row * half + half - 1 - k] = 1;

		y += band;
	}

	// A stray block or two, flipped wherever it lands.
	const strays = 1 + int(2);
	for (let k = 0; k < strays; k++) {
		const i = int(size) * half + int(half);
		cells[i] = cells[i] === 1 ? 0 : 1;
	}

	// Upside down or not, so the widest band doesn't always sit at the same end.
	const upsideDown = r() < 0.5;
	const at = (x: number, y: number): number => {
		const row = upsideDown ? size - 1 - y : y;
		const column = x < half ? x : size - 1 - x;
		return cells[row * half + column] ?? 0;
	};

	// Runs of filled cells along each row, merged into one path.
	let d = "";
	for (let row = 0; row < size; row++) {
		let x = 0;
		while (x < size) {
			if (at(x, row) === 1) {
				const start = x;
				while (x < size && at(x, row) === 1) x++;
				d += `M${start} ${row}h${x - start}v1h${start - x}z`;
			} else {
				x++;
			}
		}
	}
	return d;
};
