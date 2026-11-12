// The corners a person can pick in place of Figma's, soft. Each setting moves the whole radius
// scale by the same 4px, so a control 4px inside a popup keeps its corners concentric. A
// step no bigger than the move, a checkbox's or a kbd's, moves half its size, so sharp never
// squares it. The role tokens alias the steps, so they follow on their own, as the override
// sits on the root element they're declared on. radius/full, the pills, never changes.

import { px, rem, settingBlocks } from "./setting-blocks.mjs";

/** The corner settings a person can pick and how far each moves the scale; soft is Figma's. */
const RADII = { sharp: -4, soft: 0, round: 4 };

/** `step`, a step of the scale in px, for a setting that moves the scale by `shift`. */
function moved(step, shift) {
	return step + Math.sign(shift) * (step > Math.abs(shift) ? Math.abs(shift) : step / 2);
}

/** The corners as data: each step of the radius scale `css` declares, in px, for every setting. */
export function radiusSetting(css) {
	const steps = [
		...css.matchAll(/^\s*--but-(radius-(?:xs|sm|md|lg|\d*xl)):\s*([\d.]+)rem;\s*$/gm),
	].map(([, token, length]) => [token, px(length)]);
	if (steps.length === 0) throw new Error("No radius steps declared in tokens.css.");
	return {
		attribute: "data-but-radius",
		default: "soft",
		values: Object.fromEntries(
			Object.entries(RADII).map(([setting, shift]) => [
				setting,
				Object.fromEntries(steps.map(([token, step]) => [token, moved(step, shift)])),
			]),
		),
	};
}

/** The corners as tokens.css carries them: a block for each setting after the token blocks. */
export function radiusCss(setting) {
	return `/* The corners, set with data-but-radius on the root element; Figma's without it. */\n${settingBlocks(setting, rem)}`;
}
