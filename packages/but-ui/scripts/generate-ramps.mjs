// Generates the colour ramps 🍑 But UI's primitives hold: twelve steps per hue, in light and
// dark, on Radix Colors' curves through dittoTones, then nudged until the pairs the semantic
// names make from them read. Steps keep Radix's jobs: 1–2 grounds, 3–5 tints (3 soft, 4 its
// hover), 6–8 lines, 9 the solid fill and 10 its hover, 11 text, 12 strong text. Each colour
// (not gray or a ground) also has a3 and a4: steps 3 and 4 see-through, for the soft fills.
//
//   node scripts/generate-ramps.mjs          writes tokens/ramps.json and prints the report
//   node scripts/generate-ramps.mjs --check  fails if tokens/ramps.json is out of date
//
// The ramps go into Figma's primitives from tokens/ramps.json; Figma stays the source the
// tokens are exported from.

import * as radix from "@radix-ui/colors";
import { clampChroma, differenceEuclidean, formatHex, oklch, parse } from "culori";
import { DittoTones } from "dittotones";
import { radixRamps } from "dittotones/ramps/radix";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const out = join(dirname(fileURLToPath(import.meta.url)), "../tokens/ramps.json");

// Each hue from the colour it was picked as. `label` is what sits on its step 9 fill: white,
// except amber, whose fill stays bright under dark text, as Radix draws its bright hues.
const HUES = {
	gray: { seed: "#888888" },
	// The grounds a person can pick in place of gray: Radix's tinted grays, cool, green and warm,
	// whose tint is a whisper on the grounds and text and fullest around the lines and marks.
	slate: { ground: "slate" },
	sage: { ground: "sage" },
	sand: { ground: "sand" },
	teal: { seed: "#25b1b1", label: "white" },
	green: { seed: "#2eb87e", label: "white" },
	amber: { seed: "#f99406", label: "dark" },
	red: { seed: "#d64029", label: "white" },
	blue: { seed: "#3b82f6", label: "white" },
	purple: { seed: "#8a43d0", label: "white" },
};

// Every floor is aimed a little above its minimum, so rounding to hex never lands on 4.49.
const TEXT = 4.5 + 0.05;
const LABEL_LC = 60 + 1;

function rgb(hex) {
	return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
}
function luminance(hex) {
	const [r, g, b] = rgb(hex).map((v) => {
		v /= 255;
		return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
	});
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function wcag(a, b) {
	const [x, y] = [luminance(a), luminance(b)];
	return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
// APCA 0.0.98G: how readable text of one colour is on another, |Lc| 0–106.
function apca(text, ground) {
	function y(hex) {
		const [r, g, b] = rgb(hex).map((v) => (v / 255) ** 2.4);
		const l = 0.2126729 * r + 0.7151522 * g + 0.072175 * b;
		return l < 0.022 ? l + (0.022 - l) ** 1.414 : l;
	}
	const [t, g] = [y(text), y(ground)];
	const s = g > t ? (g ** 0.56 - t ** 0.57) * 1.14 : (g ** 0.65 - t ** 0.62) * 1.14;
	return Math.abs(s) < 0.1 ? 0 : Math.abs(s > 0 ? s - 0.027 : s + 0.027) * 100;
}
// Whether a label of colour `on` reads on `fill`.
function reads(on, fill) {
	return wcag(on, fill) >= TEXT && apca(on, fill) >= LABEL_LC;
}

// `hex`'s tint at the luminance of `like`: its OKLCH lightness found by halving, its hue and
// chroma kept. A ground's steps take gray's, so every pair that reads on gray reads on it.
function asLight(hex, like) {
	const c = oklch(parse(hex));
	const want = luminance(like);
	let [lo, hi] = [0, 1];
	for (let i = 0; i < 40; i++) {
		const mid = (lo + hi) / 2;
		const at = formatHex(clampChroma({ ...c, l: mid }, "oklch"));
		if (luminance(at) < want) lo = mid;
		else hi = mid;
	}
	return formatHex(clampChroma({ ...c, l: (lo + hi) / 2 }, "oklch"));
}

// Moves a colour's OKLCH lightness by `dl`, keeping its hue, and back into sRGB.
function shift(hex, dl) {
	const c = oklch(parse(hex));
	return formatHex(clampChroma({ ...c, l: Math.min(1, Math.max(0, c.l + dl)) }, "oklch"));
}
// `hex` at `share` opacity over `ground`, as color-mix(in srgb, … share, transparent) paints it.
function tint(hex, ground, share) {
	const [a, b] = [rgb(hex), rgb(ground)];
	return `#${a
		.map((v, i) =>
			Math.round(v * share + b[i] * (1 - share))
				.toString(16)
				.padStart(2, "0"),
		)
		.join("")}`;
}
// `hex` as the most see-through colour that comes to it laid over `ground`, as Radix makes its
// alpha steps: a soft fill so made reads as its step on the page and blends on a card or a bar.
// The page isn't pure white, so a step can be lighter than it in a channel, which no tint can
// make: the colour lands within NEAR of the step in OKLab, half the least difference anyone sees.
const NEAR = 0.01;
const distance = differenceEuclidean("oklab");
function seeThrough(hex, ground) {
	const [want, under] = [rgb(hex), rgb(ground)];
	// In whole percents, as the Figma export writes an opacity.
	for (let percent = 1; percent <= 100; percent++) {
		const alpha = Math.round(percent * 2.55);
		const a = alpha / 255;
		const colour = want.map((v, i) =>
			Math.min(255, Math.max(0, Math.round((v - (1 - a) * under[i]) / a))),
		);
		const hex8 = `#${[...colour, alpha].map((v) => v.toString(16).padStart(2, "0")).join("")}`;
		if (distance(parse(over(hex8, ground)), parse(hex)) <= NEAR) return hex8;
	}
	throw new Error(`no see-through colour comes to ${hex} over ${ground}`);
}
// `hex8`, a colour with its alpha last, laid over `ground`.
function over(hex8, ground) {
	return tint(hex8.slice(0, 7), ground, parseInt(hex8.slice(7, 9), 16) / 255);
}
// The first colour from `hex`, stepping its lightness by `dl`, for which `ok` holds.
function until(hex, dl, ok) {
	for (let i = 0; i <= 100; i++) {
		const c = shift(hex, dl * i);
		if (ok(c)) return c;
	}
	throw new Error(`no step from ${hex} reaches its floor`);
}

const darkRadix = new Map(
	[...radixRamps.keys()].map((name) => {
		const scale = radix[`${name}Dark`];
		return [
			name,
			Object.fromEntries(
				Array.from({ length: 12 }, (_, i) => [
					String(i + 1),
					oklch(parse(scale[`${name}${i + 1}`])),
				]),
			),
		];
	}),
);
const generators = {
	light: new DittoTones({ ramps: radixRamps }),
	dark: new DittoTones({ ramps: darkRadix }),
};

const ramps = {};
const report = [];
for (const [hue, { seed, label, ground }] of Object.entries(HUES)) {
	ramps[hue] = {};
	for (const mode of ["light", "dark"]) {
		if (ground) {
			// A ground is gray with a tint: Radix's tint at each step, at the luminance of gray's,
			// so every pair that reads on gray reads on it.
			const radixScale = radix[mode === "light" ? ground : `${ground}Dark`];
			const s = Array.from({ length: 12 }, (_, i) => radixScale[`${ground}${i + 1}`]);
			const gray = ramps.gray[mode];
			ramps[hue][mode] = Object.fromEntries(
				s.map((c, i) => [String(i + 1), asLight(c, gray[String(i + 1)])]),
			);
			report.push({ hue, mode, moved9: false, moved11: false, s: Object.values(ramps[hue][mode]) });
			continue;
		}
		const scale = generators[mode].generate(seed).scale;
		const s = Array.from({ length: 12 }, (_, i) => formatHex(scale[String(i + 1)]));
		// Text darkens in light and lightens in dark: away from the grounds it sits on.
		const away = mode === "light" ? -0.005 : 0.005;
		const gray = ramps.gray?.[mode];
		const grounds = [
			s[0],
			s[1],
			s[2],
			s[3],
			...(gray ? [gray["1"], gray["2"], gray["3"]] : []),
			...(mode === "light" ? ["#ffffff"] : []),
		];
		const before = { 9: s[8], 11: s[10] };
		if (label) {
			if (label === "white") {
				// Under white the fill darkens until the label reads, and its hover sits a step darker.
				s[8] = until(s[8], -0.005, (c) => reads("#ffffff", c));
				s[9] = until(shift(s[8], -0.03), -0.005, (c) => reads("#ffffff", c));
			} else {
				// The fill keeps its colour and the dark text darkens to read on it: light mode's step
				// 12, which dark mode's fill uses too, as dark mode's 12 is pale. The hover sits a step
				// lighter, where the text reads better still.
				if (mode === "light") s[11] = until(s[11], -0.005, (c) => reads(c, s[8]));
				const text = mode === "light" ? s[11] : ramps[hue].light["12"];
				s[8] = until(s[8], 0.005, (c) => reads(text, c));
				s[9] = until(shift(s[8], 0.03), 0.005, (c) => reads(text, c));
			}
		}
		// The status's -muted tint, its fill at 14% on the background, is a ground its text sits
		// on too: a banner, a highlighted row. Settled after the fill, which the tint is made of.
		grounds.push(tint(s[8], gray ? gray["1"] : s[0], 0.14));
		// The soft fill and its hover, a3 and a4: steps 3 and 4 made see-through over the page,
		// gray 1, where they come to steps 3 and 4. Its text reads on the fill over canvas, gray 3,
		// the darkest ground it sits on; a hover there, for a moment, is let off.
		const alphas = gray ? { a3: seeThrough(s[2], gray["1"]), a4: seeThrough(s[3], gray["1"]) } : {};
		if (gray) grounds.push(over(alphas.a3, gray["3"]));
		s[10] = until(s[10], away, (c) => grounds.every((g) => wcag(c, g) >= TEXT));
		ramps[hue][mode] = { ...Object.fromEntries(s.map((c, i) => [String(i + 1), c])), ...alphas };
		report.push({ hue, mode, moved9: before[9] !== s[8], moved11: before[11] !== s[10], s });
	}
}

const json = `${JSON.stringify({ $description: "Generated by scripts/generate-ramps.mjs; edit the script, not this file.", ...ramps }, null, "\t")}\n`;
if (process.argv.includes("--check")) {
	if (readFileSync(out, "utf8") !== json) {
		process.stderr.write("tokens/ramps.json is out of date: run node scripts/generate-ramps.mjs\n");
		process.exit(1);
	}
} else {
	writeFileSync(out, json);
	for (const { hue, mode, moved9, moved11, s } of report) {
		const notes = [moved9 && "9 and 10 moved for their label", moved11 && "11 moved to read"]
			.filter(Boolean)
			.join(", ");
		process.stdout.write(`${hue.padEnd(6)} ${mode.padEnd(5)} ${s.join(" ")}  ${notes}\n`);
	}
}
