// Generates the colour ramps 🍑 But UI's primitives hold: twelve steps per hue, in light and
// dark, on Radix Colors' curves through dittoTones, then nudged until the pairs the semantic
// names make from them read. Steps keep Radix's jobs: 1–2 grounds, 3–5 tints (3 soft, 4 its
// hover), 6–8 lines, 9 the solid fill and 10 its hover, 11 text, 12 strong text.
//
//   node scripts/generate-ramps.mjs          writes tokens/ramps.json and prints the report
//   node scripts/generate-ramps.mjs --check  fails if tokens/ramps.json is out of date
//
// The ramps go into Figma's primitives from tokens/ramps.json; Figma stays the source the
// tokens are exported from.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import * as radix from "@radix-ui/colors";
import { clampChroma, formatHex, oklch, parse } from "culori";
import { DittoTones } from "dittotones";
import { radixRamps } from "dittotones/ramps/radix";

const out = join(dirname(fileURLToPath(import.meta.url)), "../tokens/ramps.json");

// Each hue from the colour it was picked as. `label` is what sits on its step 9 fill: white,
// except amber, whose fill stays bright under dark text, as Radix draws its bright hues.
const HUES = {
	gray: { seed: "#888888" },
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

const rgb = (hex) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
const luminance = (hex) => {
	const [r, g, b] = rgb(hex).map((v) => {
		v /= 255;
		return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
	});
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
const wcag = (a, b) => {
	const [x, y] = [luminance(a), luminance(b)];
	return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
};
// APCA 0.0.98G: how readable text of one colour is on another, |Lc| 0–106.
const apca = (text, ground) => {
	const y = (hex) => {
		const [r, g, b] = rgb(hex).map((v) => (v / 255) ** 2.4);
		const l = 0.2126729 * r + 0.7151522 * g + 0.072175 * b;
		return l < 0.022 ? l + (0.022 - l) ** 1.414 : l;
	};
	const [t, g] = [y(text), y(ground)];
	const s = g > t ? (g ** 0.56 - t ** 0.57) * 1.14 : (g ** 0.65 - t ** 0.62) * 1.14;
	return Math.abs(s) < 0.1 ? 0 : Math.abs(s > 0 ? s - 0.027 : s + 0.027) * 100;
};

// Moves a colour's OKLCH lightness by `dl`, keeping its hue, and back into sRGB.
const shift = (hex, dl) => {
	const c = oklch(parse(hex));
	return formatHex(clampChroma({ ...c, l: Math.min(1, Math.max(0, c.l + dl)) }, "oklch"));
};
// The first colour from `hex`, stepping its lightness by `dl`, for which `ok` holds.
const until = (hex, dl, ok) => {
	for (let i = 0; i <= 100; i++) {
		const c = shift(hex, dl * i);
		if (ok(c)) return c;
	}
	throw new Error(`no step from ${hex} reaches its floor`);
};

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
for (const [hue, { seed, label }] of Object.entries(HUES)) {
	ramps[hue] = {};
	for (const mode of ["light", "dark"]) {
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
		s[10] = until(s[10], away, (c) => grounds.every((g) => wcag(c, g) >= TEXT));
		if (label) {
			const reads = (on, fill) => wcag(on, fill) >= TEXT && apca(on, fill) >= LABEL_LC;
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
		ramps[hue][mode] = Object.fromEntries(s.map((c, i) => [String(i + 1), c]));
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
