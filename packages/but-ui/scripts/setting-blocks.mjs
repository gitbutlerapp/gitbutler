// How a setting a person can pick is held as data, and how that data becomes the blocks
// tokens.css carries. A setting is { attribute, default, values }: `values` has every choice,
// Figma's included, as the tokens it sets, named as tokens.css names them without `--but-`.
// tokens/settings.json holds the same objects, so a platform that can't read CSS takes the
// values the web does, and the CSS is written from them so the two can't differ.

const REM = 16;

/** A length in px, as tokens.css writes it. */
export function rem(length) {
	return `${length / REM}rem`;
}

/** A length tokens.css wrote in rem, in px. */
export function px(length) {
	return Number(length) * REM;
}

/** An `:root[attribute=…]` block for each of the setting's choices but its default, which the
 * tokens already are. `declaration` writes a token's value as CSS. */
export function settingBlocks({ attribute, default: figma, values }, declaration) {
	return Object.entries(values)
		.filter(([choice]) => choice !== figma)
		.map(([choice, tokens]) => {
			const lines = Object.entries(tokens).map(
				([token, value]) => `  --but-${token}: ${declaration(value)};`,
			);
			return `:root[${attribute}="${choice}"] {\n${lines.join("\n")}\n}\n`;
		})
		.join("\n");
}
