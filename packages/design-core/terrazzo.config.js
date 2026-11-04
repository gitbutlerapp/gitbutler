import { defineConfig } from "@terrazzo/cli";
import css from "@terrazzo/plugin-css";
import {
	stripCollectionPrefix,
	transformToken,
} from "./scripts/terrazzo-css-helpers.mjs";

export default defineConfig({
	// Copies of tokens/json without the composed colors, which Terrazzo can't
	// parse; scripts/postprocess-light-dark.mjs writes them before each build.
	tokens: [
		"./.terrazzo/core.tokens.json",
		"./.terrazzo/semantic.tokens.json",
	],
	outDir: "./tokens",
	plugins: [
		css({
			filename: "tokens.css",
			modeSelectors: [
				{
					mode: "dark",
					selectors: [":root.dark"],
				},
			],
			transform: transformToken,
			variableName(token) {
				return stripCollectionPrefix(token.id);
			},
		}),
	],
});
