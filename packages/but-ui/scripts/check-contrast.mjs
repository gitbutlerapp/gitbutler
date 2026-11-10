// Fails when a pair of semantic tokens that is meant to be read stops reading, in light or
// dark: text on the grounds it sits on at 4.5:1 (WCAG AA), marks and faint details at 3:1.
// generate-ramps.mjs holds the ramps to these floors; this holds the names built from them,
// so a token pointed at the wrong step in Figma fails here rather than on screen.
//
//   node scripts/check-contrast.mjs

import { SAMPLES } from "./accent-samples.mjs";
import { declarationsIn, resolveColor, rootDeclarations } from "./resolve-color.mjs";
import { readFileSync } from "node:fs";

const TEXT = 4.5;
const MARK = 3;

const css = readFileSync("tokens/tokens.css", "utf8");
const defaults = rootDeclarations(css);
// Every accent a person can pick, as the declarations it changes; teal changes none.
const accents = [
	["teal", []],
	...[...css.matchAll(/:root\[data-but-accent="(\w+)"\]\s*\{([\s\S]*?)\n\}/g)].flatMap(
		([, accent, block]) =>
			accent === "custom"
				? SAMPLES.map((sample) => [sample, [...declarationsIn(block), ["accent-color", sample]]])
				: [[accent, declarationsIn(block)]],
	),
];
// The declarations in force for the accent being checked.
let declarations = defaults;

// A token's colour in `mode`, as { r, g, b, alpha } with channels 0–1.
function resolve(value, mode) {
	return resolveColor(value, mode, declarations);
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
	// Each colour's two weights: -text and soft-foreground on soft, and foreground on the solid.
	...STATUSES.flatMap((status) => [
		...[...GROUNDS, `${status}-soft`, `${status}-soft-hover`].map((ground) => [
			`${status}-text`,
			ground,
			TEXT,
		]),
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
	// A snackbar's sentence and glyphs, on its ground.
	["snackbar-foreground", "snackbar", TEXT],
	...["snackbar-muted-foreground", "warning", "snackbar-danger", "snackbar-success"].map(
		(glyph) => [glyph, "snackbar", MARK],
	),
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
