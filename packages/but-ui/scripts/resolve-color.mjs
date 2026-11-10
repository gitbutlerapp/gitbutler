// What a colour token in tokens.css comes to: the sRGB colour a browser draws it as, in light
// or in dark. It reads only what the build writes: var() aliases, light-dark(), the export's
// color-mix() tints over transparent, the any-colour accent's oklch(from …), and plain colours.

import { converter, parse } from "culori";

const toRgb = converter("rgb");
const toOklch = converter("oklch");

/** The tokens `block` declares, as [name without --but-, value]. */
export function declarationsIn(block) {
	return [...block.matchAll(/^\s*--but-([\w-]+):\s*(.+?);\s*$/gm)].map(([, name, value]) => [
		name,
		value,
	]);
}

/** The tokens `css` declares on plain :root, Figma's values, as a Map from name to value. */
export function rootDeclarations(css) {
	return new Map(
		[...css.matchAll(/^:root\s*\{([\s\S]*?)\n\}/gm)].flatMap(([, block]) => declarationsIn(block)),
	);
}

// Splits a function's arguments at its top-level commas.
function args(inner) {
	const out = [];
	let depth = 0;
	let start = 0;
	for (let i = 0; i < inner.length; i++) {
		if (inner[i] === "(") depth++;
		else if (inner[i] === ")") depth--;
		else if (inner[i] === "," && depth === 0) {
			out.push(inner.slice(start, i).trim());
			start = i + 1;
		}
	}
	return [...out, inner.slice(start).trim()];
}
function call(value, fn) {
	return value.startsWith(`${fn}(`) && value.endsWith(")")
		? args(value.slice(fn.length + 1, -1))
		: null;
}

/** `value`'s colour in `mode` with `declarations` in force, as { r, g, b, alpha } with channels
 * 0–1. */
export function resolveColor(value, mode, declarations) {
	value = value.trim();
	const ref = value.match(/^var\(--but-([\w-]+)\)$/);
	if (ref) {
		const next = declarations.get(ref[1]);
		if (!next) throw new Error(`--but-${ref[1]} isn't declared`);
		return resolveColor(next, mode, declarations);
	}
	const lightDark = call(value, "light-dark");
	if (lightDark) return resolveColor(lightDark[mode === "light" ? 0 : 1], mode, declarations);
	// What generate-accents.mjs writes for any colour: its hue and capped chroma at a lightness.
	const relative = value.match(
		/^oklch\(from var\(--but-([\w-]+)(?:, [^)]*\))?\) ([\d.]+) (?:calc\()?min\(c, ([\d.]+)\)(?: \* ([\d.]+)\))? h\)$/,
	);
	if (relative) {
		const [, ref, l, cap, share = "1"] = relative;
		const from = toOklch(parse(declarations.get(ref)));
		const c = Math.min(from.c, parseFloat(cap)) * parseFloat(share);
		// Out of sRGB the browser clips each channel.
		const { r, g, b } = toRgb({ mode: "oklch", l: parseFloat(l), c, h: from.h ?? 0 });
		function clip(v) {
			return Math.min(1, Math.max(0, v));
		}
		return { r: clip(r), g: clip(g), b: clip(b), alpha: 1 };
	}
	const mix = call(value, "color-mix");
	if (mix) {
		// Only what the export writes: a colour at a share, over transparent.
		const [, colour, rest] = mix;
		const at = colour.match(/^(.*\S)\s+(\S+)$/);
		if (rest !== "transparent" || !at) throw new Error(`can't read ${value}`);
		const share = at[2].startsWith("var(")
			? declarations.get(at[2].match(/--but-([\w-]+)/)[1])
			: at[2];
		const base = resolveColor(at[1], mode, declarations);
		return { ...base, alpha: base.alpha * (parseFloat(share) / 100) };
	}
	const colour = parse(value);
	if (!colour) throw new Error(`can't read ${value}`);
	const { r, g, b } = toRgb(colour);
	return { r, g, b, alpha: colour.alpha ?? 1 };
}
