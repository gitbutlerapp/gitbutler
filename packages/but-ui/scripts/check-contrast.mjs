// Fails when a pair of semantic tokens that is meant to be read stops reading, in light or
// dark: text on the grounds it sits on at 4.5:1 (WCAG AA), marks and faint details at 3:1.
// generate-ramps.mjs holds the ramps to these floors; this holds the names built from them,
// so a token pointed at the wrong step in Figma fails here rather than on screen.
//
//   node scripts/check-contrast.mjs

import { converter, formatHex, parse } from "culori";
import { readFileSync } from "node:fs";

const toRgb = converter("rgb");
const toOklch = converter("oklch");

const TEXT = 4.5;
const MARK = 3;

const css = readFileSync("tokens/tokens.css", "utf8");
function read(block) {
	return [...block.matchAll(/^\s*--but-([\w-]+):\s*(.+?);\s*$/gm)].map(([, name, value]) => [
		name,
		value,
	]);
}
const defaults = read(css.match(/:root\s*\{([\s\S]*?)\n\}/)?.[1] ?? "");
// Any colour stands in as the full-strength hue every 15°, the purest a person can type, and
// a gray.
const SAMPLES = [
	...Array.from({ length: 24 }, (_, i) => formatHex({ mode: "hsl", h: i * 15, s: 1, l: 0.5 })),
	"#808080",
];
// Every accent a person can pick, as the declarations it changes; teal changes none.
const accents = [
	["teal", []],
	...[...css.matchAll(/:root\[data-but-accent="(\w+)"\]\s*\{([\s\S]*?)\n\}/g)].flatMap(
		([, accent, block]) =>
			accent === "custom"
				? SAMPLES.map((sample) => [sample, [...read(block), ["accent-color", sample]]])
				: [[accent, read(block)]],
	),
];
// The declarations in force for the accent being checked.
let declarations = new Map(defaults);

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

// A token's colour in `mode`, as { r, g, b, alpha } with channels 0–1.
function resolve(value, mode) {
	value = value.trim();
	const ref = value.match(/^var\(--but-([\w-]+)\)$/);
	if (ref) {
		const next = declarations.get(ref[1]);
		if (!next) throw new Error(`--but-${ref[1]} isn't declared`);
		return resolve(next, mode);
	}
	const lightDark = call(value, "light-dark");
	if (lightDark) return resolve(lightDark[mode === "light" ? 0 : 1], mode);
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
		const base = resolve(at[1], mode);
		return { ...base, alpha: base.alpha * (parseFloat(share) / 100) };
	}
	const colour = parse(value);
	if (!colour) throw new Error(`can't read ${value}`);
	const { r, g, b } = toRgb(colour);
	return { r, g, b, alpha: colour.alpha ?? 1 };
}

function over(top, under) {
	return {
		r: top.r * top.alpha + under.r * (1 - top.alpha),
		g: top.g * top.alpha + under.g * (1 - top.alpha),
		b: top.b * top.alpha + under.b * (1 - top.alpha),
		alpha: 1,
	};
}
function luminance({ r, g, b }) {
	function lin(v) {
		return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
	}
	return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}
function contrast(a, b) {
	const [x, y] = [luminance(a), luminance(b)];
	return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

// The pairs, as [what's read, what it's read on, floor]. A translucent ground sits on
// background, as a tint or a hover does in a card; translucent text sits on its ground.
const GROUNDS = ["background", "canvas", "muted"];
const STATUSES = ["primary", "danger", "warning", "success", "info", "done"];
const pairs = [
	...["foreground", "muted-foreground"].flatMap((text) =>
		GROUNDS.map((ground) => [text, ground, TEXT]),
	),
	...GROUNDS.map((ground) => ["subtle-foreground", ground, MARK]),
	["card-foreground", "card", TEXT],
	["popover-foreground", "popover", TEXT],
	["secondary-foreground", "secondary", TEXT],
	["selected-foreground", "selected", TEXT],
	["inverse-foreground", "inverse", TEXT],
	["inverse-foreground", "inverse-hover", TEXT],
	["inverse-muted-foreground", "inverse", TEXT],
	["inverse-subtle-foreground", "inverse", MARK],
	// Each colour's two weights: soft-foreground on soft, and foreground on the solid; -text is for
	// the neutral grounds, not for soft.
	...STATUSES.flatMap((status) => [
		...GROUNDS.map((ground) => [`${status}-text`, ground, TEXT]),
		[`${status}-soft-foreground`, `${status}-soft`, TEXT],
		[`${status}-soft-foreground`, `${status}-soft-hover`, TEXT],
		[`${status}-foreground`, status, TEXT],
		[`${status}-foreground`, `${status}-hover`, TEXT],
	]),
	...["local", "remote", "upstream", "integrated"].flatMap((commit) =>
		["background", "canvas"].map((ground) => [`commit-${commit}`, ground, MARK]),
	),
	...["addition", "modification", "deletion", "rename"].flatMap((change) =>
		["background", "canvas"].map((ground) => [`change-status-${change}`, ground, MARK]),
	),
	...["background", "canvas"].map((ground) => ["ring", ground, MARK]),
];

const failures = [];
for (const [accent, overrides] of accents) {
	declarations = new Map([...defaults, ...overrides]);
	for (const mode of ["light", "dark"]) {
		const background = resolve("var(--but-background)", mode);
		for (const [text, ground, floor] of pairs) {
			const on = over(resolve(`var(--but-${ground})`, mode), background);
			const ratio = contrast(over(resolve(`var(--but-${text})`, mode), on), on);
			if (ratio < floor)
				failures.push(
					`${accent.padEnd(7)} ${mode.padEnd(5)}  --but-${text} on --but-${ground}: ${ratio.toFixed(2)}:1, needs ${floor}:1`,
				);
		}
	}
}

if (failures.length) {
	process.stderr.write(
		`check-contrast: ${failures.length} pairs don't read:\n${failures.join("\n")}\n`,
	);
	process.exit(1);
}
process.stdout.write(
	`check-contrast: all ${pairs.length} pairs read in light and dark, with each accent (${accents.length - SAMPLES.length} named and ${SAMPLES.length} custom).\n`,
);
