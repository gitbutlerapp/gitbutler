// The densities a person can pick in place of Figma's, balanced. Each setting moves
// the heights that set how tall a control, a row or a bar is, and nothing else: type, icons,
// corners and the padding beside the text stay as they are, so a denser screen holds more lines
// rather than smaller ones. A height of 28px or more moves 4px; a smaller one moves 2px, so a small button keeps
// room for its line. size/tag aliases size/control-sm, so it follows on its own, as the override
// sits on the root element the tokens are declared on.

/** The density settings a person can pick, besides Figma's, and which way each moves. */
export const DENSITIES = { compact: -1, spacious: 1 };

/** The heights a density moves. */
const HEIGHTS = ["control-sm", "control", "bar", "row", "header"];

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
		return `:root[data-but-density="${setting}"] {\n${lines.join("\n")}\n}\n`;
	});
	return `${css}\n/* The density, set with data-but-density on the root element; Figma's without it. */\n${blocks.join("\n")}`;
}
