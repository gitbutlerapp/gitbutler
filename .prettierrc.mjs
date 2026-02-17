/**
 * We want to keep our code style aligned with the language or ecosystem's
 * defaults unless there is a very good reason like accessability that we have
 * all agreed on.
 *
 * @see https://prettier.io/docs/en/configuration.html
 * @type {import("prettier").Config}
 */
const config = {
	// Default false - Tabs have a good acccessability argument
	// https://adamtuttle.codes/blog/2021/tabs-vs-spaces-its-an-accessibility-issue/
	// and have the potential to become the default in prettier 3.0.
	useTabs: true,
	// Default 80 - This is a contraversial topic, for now we're going for
	// parity with rust's default line length.
	printWidth: 100,
	// Default 'lf' - The 'lf' option in prettier can break some windows
	// scripts. It's better to use 'auto' and a well configured git filter.
	endOfLine: "auto",
	cssDeclarationSorterOrder: "smacss",
	plugins: ["prettier-plugin-svelte", "prettier-plugin-css-order"],
	overrides: [{ files: "*.svelte", options: { parser: "svelte" } }],
};

export default config;
