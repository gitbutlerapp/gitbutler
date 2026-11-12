// The tokens as data, for a platform that can't read CSS: what each colour comes to in light
// and in dark, each length in px and each duration in ms. tokens/tokens.json holds it. These
// are Figma's values; what a setting changes is in tokens/settings.json. Fonts, shadows, easings
// and opacities stay in tokens.css alone, as they're written for CSS.

import { resolveColor, rootDeclarations } from "./resolve-color.mjs";
import { px } from "./setting-blocks.mjs";
import { formatHex, formatHex8 } from "culori";

/** How tokens.css writes a colour, once its aliases are followed. */
const COLOR = /^(light-dark|color-mix|hsl|oklch)\(|^#/;

/** { colors, lengths, durations, aliases, grounds } for the tokens `css` declares: a colour as
 * sRGB hex for light and for dark, with its alpha last when it has one, a length in px, a duration
 * in ms, and for a length that aliases another, the one it names, as a setting moves only the
 * steps and the roles follow them. `grounds` has, for each ground of `ground` (the setting) but
 * its default, every colour that comes out differently on it. Names are tokens.css's without `--but-`. */
export function tokensData(css, ground) {
	const declarations = rootDeclarations(css);
	/** The token `value` aliases, if it's only an alias. */
	function aliased(value) {
		return value.match(/^var\(--but-([\w-]+)\)$/)?.[1];
	}
	/** What `value` is once its aliases are followed. */
	function literal(value) {
		const alias = aliased(value);
		return alias ? literal(declarations.get(alias) ?? "") : value;
	}
	function hex(value, mode, scope = declarations) {
		const { r, g, b, alpha } = resolveColor(value, mode, scope);
		const colour = { mode: "rgb", r, g, b, alpha };
		return alpha < 1 ? formatHex8(colour) : formatHex(colour);
	}
	const colors = {};
	const lengths = {};
	const durations = {};
	const aliases = {};
	for (const [token, value] of declarations) {
		const end = literal(value);
		const rem = end.match(/^(-?[\d.]+)rem$/)?.[1];
		const ms = end.match(/^([\d.]+)ms$/)?.[1];
		if (ms !== undefined) durations[token] = Number(ms);
		else if (rem !== undefined) {
			lengths[token] = px(rem);
			const alias = aliased(value);
			if (alias) aliases[token] = alias;
		} else if (COLOR.test(end))
			colors[token] = { light: hex(value, "light"), dark: hex(value, "dark") };
	}
	const grounds = {};
	for (const [name, steps] of Object.entries(ground?.values ?? {})) {
		if (name === ground.default) continue;
		const scope = new Map([
			...declarations,
			...Object.entries(steps).map(([step, to]) => [step, `var(--but-${to})`]),
		]);
		grounds[name] = {};
		for (const [token, value] of declarations) {
			if (!(token in colors)) continue;
			const drawn = { light: hex(value, "light", scope), dark: hex(value, "dark", scope) };
			if (drawn.light !== colors[token].light || drawn.dark !== colors[token].dark)
				grounds[name][token] = drawn;
		}
	}
	return { colors, lengths, durations, aliases, grounds };
}
