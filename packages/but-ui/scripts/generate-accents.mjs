// The accent a person can pick in place of teal. Figma keeps the default theme; each other
// accent is the --but-primary family pointed at the same steps of another ramp, so it reads
// wherever teal does. The family's tints that mix --but-primary follow on their own, as the
// override sits on the root element they're declared on.

import { converter, parse } from "culori";

/** The ramps a person can pick as the accent, besides teal. */
export const ACCENTS = ["blue", "purple"];

const DEFAULT = "teal";

// Any colour, as --but-accent-color: its hue and chroma at teal's lightness, so it reads
// wherever teal does. The chroma is capped and the steps text sits on or is drawn in are a
// little further from the ground than teal's, which is what keeps the most vivid picks, the
// cyans and reds that leave sRGB at teal's lightness, at 4.5:1 once the browser clips them.
const CHROMA_CAP = 0.15;
const LIGHTNESS_MARGIN = 0.04;
const MARGIN_STEPS = new Set(["9", "10", "11"]);

const oklch = converter("oklch");
function round(n) {
	return Number(n.toFixed(3));
}

/** `css` with an `:root[data-but-accent=…]` block for each accent after its token blocks. */
export function appendAccents(css) {
	const family = [...css.matchAll(/^\s*(--but-primary[\w-]*):\s*(.+?);\s*$/gm)]
		.map(([, name, value]) => ({ name, value }))
		.filter(({ value }) => value.includes(`--but-${DEFAULT}-`));
	const blocks = ACCENTS.map((accent) => {
		const lines = family.map(
			({ name, value }) =>
				`  ${name}: ${value.replaceAll(`--but-${DEFAULT}-`, `--but-${accent}-`)};`,
		);
		return `:root[data-but-accent="${accent}"] {\n${lines.join("\n")}\n}\n`;
	});
	return `${css}\n/* The accent, set with data-but-accent on the root element; teal without it. */\n${blocks.join("\n")}\n${customBlock(css, family)}`;
}

function customBlock(css, family) {
	function ramp(step) {
		const value = css.match(new RegExp(`^\\s*--but-${DEFAULT}-${step}:\\s*(.+?);\\s*$`, "m"))?.[1];
		if (!value) throw new Error(`--but-${DEFAULT}-${step} isn't declared`);
		const lightDark = value.match(/^light-dark\((hsl\([^)]*\)),\s*(hsl\([^)]*\))\)$/);
		return (lightDark ? [lightDark[1], lightDark[2]] : [value, value]).map((v) => oklch(parse(v)));
	}
	const [primary] = ramp("9");
	const lines = family.map(({ name, value }) => {
		const step = value.match(new RegExp(`--but-${DEFAULT}-(\\d+)`))[1];
		const [light, dark] = ramp(step).map((colour, mode) => {
			// Further from the ground: darker in light, and for text on dark, lighter.
			const away = MARGIN_STEPS.has(step) ? (mode === 1 && step === "11" ? 1 : -1) : 0;
			const l = round(colour.l + away * LIGHTNESS_MARGIN);
			const share = round(colour.c / primary.c);
			const c = share === 1 ? `min(c, ${CHROMA_CAP})` : `calc(min(c, ${CHROMA_CAP}) * ${share})`;
			return `oklch(from var(--but-accent-color, var(--but-${DEFAULT}-9)) ${l} ${c} h)`;
		});
		return `  ${name}: ${light === dark ? light : `light-dark(${light}, ${dark})`};`;
	});
	return `/* Any colour: data-but-accent="custom" and --but-accent-color on the root element. */\n:root[data-but-accent="custom"] {\n${lines.join("\n")}\n}\n`;
}
