// The densities a person can pick in place of Figma's, balanced. Each setting moves the heights
// that set how tall a control, a row or a bar is, and the padding beside a control's text by a
// smaller step, so a compact control is a little narrower as well as shorter and a spacious one
// doesn't read as tall and thin. Type, icons and corners stay as they are, so a denser screen
// holds more lines rather than smaller ones. A height of 28px or more moves 4px; a smaller one
// moves 2px, so a small button keeps room for its line. size/tag aliases size/control-sm, so it
// follows on its own, as the override sits on the root element the tokens are declared on.

import { px, rem, settingBlocks } from "./setting-blocks.mjs";

/** The density settings a person can pick and which way each moves; balanced is Figma's. */
const DENSITIES = { compact: -1, balanced: 0, spacious: 1 };

/** The heights a density moves. */
const HEIGHTS = ["control-sm", "control", "bar", "row", "header"];

/** How far each setting moves the side padding of controls and rows: buttons, segments, fields,
 * tabs and popup rows add it to their own padding, so rows and the headings over them stay
 * aligned. */
const PADDING = 2;

/** `height`, one of the heights in px, for a setting that moves it the way `direction` says. */
function moved(height, direction) {
	return height + direction * (height >= 28 ? 4 : 2);
}

/** The density as data: the heights `css` declares and the padding step, in px, for every
 * setting. */
export function densitySetting(css) {
	const heights = HEIGHTS.map((step) => {
		const token = `size-${step}`;
		const length = css.match(new RegExp(`^\\s*--but-${token}:\\s*([\\d.]+)rem;\\s*$`, "m"))?.[1];
		if (length === undefined) throw new Error(`--but-${token} isn't declared in tokens.css.`);
		return [token, px(length)];
	});
	return {
		attribute: "data-but-density",
		default: "balanced",
		values: Object.fromEntries(
			Object.entries(DENSITIES).map(([setting, direction]) => [
				setting,
				{
					...Object.fromEntries(
						heights.map(([token, height]) => [token, moved(height, direction)]),
					),
					"density-padding": direction * PADDING,
				},
			]),
		),
	};
}

/** The density as tokens.css carries it: the padding step Figma's has, nothing, and a block for
 * each setting after the token blocks. */
export function densityCss(setting) {
	const figma = setting.values[setting.default];
	return `/* The density, set with data-but-density on the root element; Figma's without it. density-padding is how far it moves a control's side padding, nothing at Figma's. */\n:root {\n  --but-density-padding: ${rem(figma["density-padding"])};\n}\n\n${settingBlocks(setting, rem)}`;
}
