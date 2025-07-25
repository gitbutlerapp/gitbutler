/**
 * @see https://prettier.io/docs/en/configuration.html
 * @type {import("prettier").Config}
 */
const config = {
	useTabs: true,
	singleQuote: true,
	trailingComma: 'none',
	printWidth: 100,
	cssDeclarationSorterOrder: 'smacss',
	plugins: ['prettier-plugin-svelte', 'prettier-plugin-css-order'],
	overrides: [{ files: '*.svelte', options: { parser: 'svelte' } }],
	endOfLine: 'auto'
};

export default config;
