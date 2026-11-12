// The accent a person can pick in place of teal. Figma keeps the default theme; each other
// accent is the --but-primary family pointed at the same steps of another ramp, so it reads
// wherever teal does. The family's tints that mix --but-primary follow on their own, as the
// override sits on the root element they're declared on.

import { settingBlocks } from "./setting-blocks.mjs";
import { converter, parse } from "culori";

/** The ramps a person can pick as the accent; teal is Figma's. */
const ACCENTS = ["teal", "blue", "purple"];

const FIGMA = "teal";

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

/**
 * The accent as data. `values` has, for each named accent, the ramp step every token of the
 * primary family points at. `custom` is any colour: each token is that colour's hue, with its
 * chroma capped at `chromaCap` and taken at the `chroma` share, at the `lightness` given, in
 * OKLCH, for light and for dark, and at `alpha` for a see-through step (the soft fills).
 */
export function accentSetting(css) {
	// The family as Figma points it: each token at a step of teal.
	const family = [...css.matchAll(/^\s*--but-(primary[\w-]*):\s*(.+?);\s*$/gm)]
		.filter(([, , value]) => value.includes(`--but-${FIGMA}-`))
		.map(([, token, value]) => {
			const step = value.match(new RegExp(`^var\\(--but-${FIGMA}-(a?\\d+)\\)$`))?.[1];
			if (step === undefined) throw new Error(`--but-${token} isn't one step of ${FIGMA}.`);
			return [token, step];
		});
	function ramp(step) {
		const value = css.match(new RegExp(`^\\s*--but-${FIGMA}-${step}:\\s*(.+?);\\s*$`, "m"))?.[1];
		if (!value) throw new Error(`--but-${FIGMA}-${step} isn't declared`);
		const lightDark = value.match(/^light-dark\((hsl\([^)]*\)),\s*(hsl\([^)]*\))\)$/);
		return (lightDark ? [lightDark[1], lightDark[2]] : [value, value]).map((v) => oklch(parse(v)));
	}
	const [primary] = ramp("9");
	return {
		attribute: "data-but-accent",
		default: FIGMA,
		values: Object.fromEntries(
			ACCENTS.map((accent) => [
				accent,
				Object.fromEntries(family.map(([token, step]) => [token, `${accent}-${step}`])),
			]),
		),
		custom: {
			color: "accent-color",
			fallback: `${FIGMA}-9`,
			chromaCap: CHROMA_CAP,
			tokens: Object.fromEntries(
				family.map(([token, step]) => {
					const [light, dark] = ramp(step).map((colour, mode) => {
						// Further from the ground: darker in light, and for text on dark, lighter.
						const away = MARGIN_STEPS.has(step) ? (mode === 1 && step === "11" ? 1 : -1) : 0;
						return {
							lightness: round(colour.l + away * LIGHTNESS_MARGIN),
							chroma: round(colour.c / primary.c),
							...(colour.alpha < 1 && { alpha: round(colour.alpha) }),
						};
					});
					return [token, { light, dark }];
				}),
			),
		},
	};
}

/** The accent as tokens.css carries it: a block for each named accent after the token blocks,
 * then the one that draws the family from any colour. */
export function accentCss(setting) {
	const { color, fallback, chromaCap, tokens } = setting.custom;
	function drawn({ lightness, chroma, alpha }) {
		const c = chroma === 1 ? `min(c, ${chromaCap})` : `calc(min(c, ${chromaCap}) * ${chroma})`;
		const a = alpha === undefined ? "" : ` / ${alpha}`;
		return `oklch(from var(--but-${color}, var(--but-${fallback})) ${lightness} ${c} h${a})`;
	}
	const lines = Object.entries(tokens).map(([token, modes]) => {
		const [light, dark] = [drawn(modes.light), drawn(modes.dark)];
		return `  --but-${token}: ${light === dark ? light : `light-dark(${light}, ${dark})`};`;
	});
	const named = settingBlocks(setting, (step) => `var(--but-${step})`);
	return `/* The accent, set with ${setting.attribute} on the root element; teal without it. */\n${named}\n/* Any colour: ${setting.attribute}="custom" and --but-${color} on the root element. */\n:root[${setting.attribute}="custom"] {\n${lines.join("\n")}\n}\n`;
}
