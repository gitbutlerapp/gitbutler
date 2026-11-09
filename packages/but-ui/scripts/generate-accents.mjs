// The accent a person can pick in place of teal. Figma keeps the default theme; each other
// accent is the --but-primary family pointed at the same steps of another ramp, so it reads
// wherever teal does. The family's tints that mix --but-primary follow on their own, as the
// override sits on the root element they're declared on.

/** The ramps a person can pick as the accent, besides teal. */
export const ACCENTS = ["blue", "purple"];

const DEFAULT = "teal";

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
	return `${css}\n/* The accent, set with data-but-accent on the root element; teal without it. */\n${blocks.join("\n")}`;
}
