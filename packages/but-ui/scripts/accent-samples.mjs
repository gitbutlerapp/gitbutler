// What the any-colour accent comes to for a set of picks, as a browser draws it: the samples a
// platform that draws the accent itself, as GPUI does, checks its own formula against.
// tokens/accent-samples.json holds them.

import { declarationsIn, resolveColor, rootDeclarations } from "./resolve-color.mjs";
import { formatHex, formatHex8 } from "culori";

/** Any colour, as the full-strength hue every 15°, the purest a person can type, and a gray. */
export const SAMPLES = [
	...Array.from({ length: 24 }, (_, i) => formatHex({ mode: "hsl", h: i * 15, s: 1, l: 0.5 })),
	"#808080",
];

/** For each sample, every token of the custom accent's block in `css` as sRGB hex, light and
 * dark. */
export function accentSamples(css) {
	const block = css.match(/:root\[data-but-accent="custom"\]\s*\{([\s\S]*?)\n\}/)?.[1];
	if (!block) throw new Error("tokens.css has no custom accent block");
	const custom = declarationsIn(block);
	return SAMPLES.map((color) => {
		const declarations = new Map([...rootDeclarations(css), ...custom, ["accent-color", color]]);
		function hex(value, mode) {
			const { r, g, b, alpha } = resolveColor(value, mode, declarations);
			const colour = { mode: "rgb", r, g, b, alpha };
			return alpha < 1 ? formatHex8(colour) : formatHex(colour);
		}
		return {
			color,
			tokens: Object.fromEntries(
				custom.map(([token, value]) => [
					token,
					{ light: hex(value, "light"), dark: hex(value, "dark") },
				]),
			),
		};
	});
}
