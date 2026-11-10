// The densities a person can pick in place of Figma's, balanced. Each setting moves the heights
// that set how tall a control, a row or a bar is, and the padding beside a control's text by a
// smaller step, so a compact control is a little narrower as well as shorter and a spacious one
// doesn't read as tall and thin. Type, icons and corners stay as they are, so a denser screen
// holds more lines rather than smaller ones. A height of 28px or more moves 4px; a smaller one
// moves 2px, so a small button keeps room for its line. size/tag aliases size/control-sm, so it
// follows on its own, as the override sits on the root element the tokens are declared on.

/** The density settings a person can pick, besides Figma's, and which way each moves. */
export const DENSITIES = { compact: -1, spacious: 1 };

/** The heights a density moves. */
const HEIGHTS = ["control-sm", "control", "bar", "row", "header"];

/** How far each setting moves the side padding of controls and rows: buttons, segments, fields,
 * tabs and popup rows add it to their own padding, so rows and the headings over them stay
 * aligned. */
const PADDING = 2;

const REM = 16;

/** `px`, one of the heights, for a setting that moves it the way `direction` says. */
function moved(px, direction) {
	return px + direction * (px >= 28 ? 4 : 2);
}

/** `css` with an `:root[data-but-density=…]` block for each setting after its token blocks. */
export function appendDensities(css) {
	const heights = HEIGHTS.map((step) => {
		const name = `--but-size-${step}`;
		const rem = css.match(new RegExp(`^\\s*${name}:\\s*([\\d.]+)rem;\\s*$`, "m"))?.[1];
		if (rem === undefined) throw new Error(`${name} isn't declared in tokens.css.`);
		return { name, px: Number(rem) * REM };
	});
	const blocks = Object.entries(DENSITIES).map(([setting, direction]) => {
		const lines = heights.map(({ name, px }) => `  ${name}: ${moved(px, direction) / REM}rem;`);
		lines.push(`  --but-density-padding: ${(direction * PADDING) / REM}rem;`);
		return `:root[data-but-density="${setting}"] {\n${lines.join("\n")}\n}\n`;
	});
	return `${css}\n/* The density, set with data-but-density on the root element; Figma's without it. density-padding is how far it moves a control's side padding, nothing at Figma's. */\n:root {\n  --but-density-padding: 0rem;\n}\n\n${blocks.join("\n")}`;
}
