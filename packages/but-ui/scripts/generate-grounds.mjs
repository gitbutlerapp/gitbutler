// The grounds a person can pick in place of Figma's, gray: one of the tinted grays the ramps
// hold, slate, sage or sand. A ground points every step of the gray ramp at the same step of
// its own, so the names built on gray (the backgrounds, borders, text and the inverse surface)
// follow on their own, as the override sits on the root element they're declared on. Statuses,
// diffs and the accent read their own ramps, so a red line means the same on every ground.

import { settingBlocks } from "./setting-blocks.mjs";

/** The grounds a person can pick; gray is Figma's. Each is a ramp generate-ramps.mjs makes. */
const GROUNDS = ["gray", "slate", "sage", "sand"];

const FIGMA = "gray";

/** The ground as data: for each ground, the ramp step each gray step points at. */
export function groundSetting(css) {
	const steps = [...css.matchAll(/^\s*--but-gray-(\d+):/gm)].map(([, step]) => step);
	if (steps.length !== 12)
		throw new Error(`tokens.css declares ${steps.length} gray steps, not 12.`);
	for (const ground of GROUNDS)
		for (const step of steps)
			if (!css.includes(`--but-${ground}-${step}:`))
				throw new Error(`--but-${ground}-${step} isn't declared: is ${ground} a ramp in Figma?`);
	return {
		attribute: "data-but-ground",
		default: FIGMA,
		values: Object.fromEntries(
			GROUNDS.map((ground) => [
				ground,
				Object.fromEntries(steps.map((step) => [`gray-${step}`, `${ground}-${step}`])),
			]),
		),
	};
}

/** The ground as tokens.css carries it: a block for each ground but gray. */
export function groundCss(setting) {
	return `/* The ground, set with ${setting.attribute} on the root element; gray without it. */\n${settingBlocks(setting, (step) => `var(--but-${step})`)}`;
}
