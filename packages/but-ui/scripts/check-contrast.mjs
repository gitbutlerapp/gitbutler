// Fails when a pair of semantic tokens that is meant to be read stops reading, in light or
// dark: text on the grounds it sits on at 4.5:1 (WCAG AA), marks and faint details at 3:1, and
// a label on a colour's solid fill at 4.5:1, or at APCA Lc 70 in light.
// generate-ramps.mjs holds the ramps to these floors; this holds the names built from them,
// so a token pointed at the wrong step in Figma fails here rather than on screen.
//
//   node scripts/check-contrast.mjs

import { SAMPLES } from "./accent-samples.mjs";
import { declarationsIn, resolveColor, rootDeclarations } from "./resolve-color.mjs";
import { readFileSync } from "node:fs";

const TEXT = 4.5;
const MARK = 3;
// A label on a colour's solid fill reads at 4.5:1, or in light mode by APCA at Lc 70 instead:
// the white labels, whose fills generate-ramps.mjs makes that way.
const LABEL = "label";
const LABEL_LC = 70;

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
// Every ground a person can pick, as the declarations it changes; gray changes none.
const grounds = [
	["gray", []],
	...[...css.matchAll(/:root\[data-but-ground="(\w+)"\]\s*\{([\s\S]*?)\n\}/g)].map(
		([, ground, block]) => [ground, declarationsIn(block)],
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
// APCA 0.0.98G's |Lc| of `text` on `ground`, as generate-ramps.mjs has it.
function apca(text, ground) {
	function y({ r, g, b }) {
		const l = 0.2126729 * r ** 2.4 + 0.7151522 * g ** 2.4 + 0.072175 * b ** 2.4;
		return l < 0.022 ? l + (0.022 - l) ** 1.414 : l;
	}
	const [t, g] = [y(text), y(ground)];
	const s = g > t ? (g ** 0.56 - t ** 0.57) * 1.14 : (g ** 0.65 - t ** 0.62) * 1.14;
	return Math.abs(s) < 0.1 ? 0 : Math.abs(s > 0 ? s - 0.027 : s + 0.027) * 100;
}

// Whether tokens.css declares `--but-${name}`.
function declared(name) {
	return defaults.has(name);
}

// The pairs, as [what's read, what it's read on, floor]. A translucent ground sits on
// background, as a tint or a hover does in a card; translucent text sits on its ground. A soft
// fill sits on canvas too, the darkest ground it meets; its hover there, for a moment, is let off.
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
	// Each colour's weights: -text on the plain grounds and an outline button's hover, -mark (an
	// icon, a dot, a switch) on the plain grounds, soft-foreground on soft and on surface, and
	// foreground on the solid.
	...STATUSES.flatMap((status) => [
		...[
			...GROUNDS,
			...(declared(`${status}-outline-hover`) ? [`${status}-outline-hover`] : []),
		].map((ground) => [`${status}-text`, ground, TEXT]),
		...GROUNDS.map((ground) => [`${status}-mark`, ground, MARK]),
		...[`soft`, `soft-hover`, `surface`, `surface-hover`].map((ground) => [
			`${status}-soft-foreground`,
			`${status}-${ground}`,
			TEXT,
		]),
		[`${status}-foreground`, status, LABEL],
		[`${status}-foreground`, `${status}-hover`, LABEL],
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
for (const [groundName, groundOverrides] of grounds)
	for (const [accent, overrides] of accents) {
		declarations = new Map([...defaults, ...groundOverrides, ...overrides]);
		for (const mode of ["light", "dark"]) {
			for (const [text, ground, floor] of pairs) {
				const unders = ground.endsWith("-soft") ? ["background", "canvas"] : ["background"];
				for (const under of unders) {
					const on = over(
						resolve(`var(--but-${ground})`, mode),
						resolve(`var(--but-${under})`, mode),
					);
					const ink = over(resolve(`var(--but-${text})`, mode), on);
					const ratio = contrast(ink, on);
					const where = under === "background" ? "" : ` over --but-${under}`;
					const said = `${groundName.padEnd(5)} ${accent.padEnd(7)} ${mode.padEnd(5)}  --but-${text} on --but-${ground}${where}`;
					if (floor === LABEL) {
						const lc = apca(ink, on);
						if (ratio < TEXT && (mode === "dark" || lc < LABEL_LC))
							failures.push(
								mode === "light"
									? `${said}: ${ratio.toFixed(2)}:1 and Lc ${lc.toFixed(1)}, needs ${TEXT}:1 or Lc ${LABEL_LC}`
									: `${said}: ${ratio.toFixed(2)}:1, needs ${TEXT}:1`,
							);
					} else if (ratio < floor)
						failures.push(`${said}: ${ratio.toFixed(2)}:1, needs ${floor}:1`);
				}
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
	`check-contrast: all ${pairs.length} pairs read in light and dark, with each accent (${accents.length - SAMPLES.length} named and ${SAMPLES.length} custom) on each ground (${grounds.map(([name]) => name).join(", ")}).\n`,
);
