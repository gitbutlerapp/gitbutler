/**
 * At GitButler, we want stick to our language or ecosystem's default
 * configurations unless there is some very good reason why we should deviate.
 *
 * @see https://prettier.io/docs/en/configuration.html
 * @type {import("prettier").Config}
 */
const config = {
	// Tabs are considered more accessable for people using screen readers and
	// for people with large fonts who might want a tab size either less or
	// greater than 4.
	useTabs: true,
	// Print width being 100 is carried over before we started to standardize
	// styling. For now we are keeping as-is, because of the parity between TS &
	// Rust.
	printWidth: 100,
	// EOL 'auto' prevents prettier from trying to enforce some kind of line
	// ending. It's much better to let git filters handle this for us.
	endOfLine: "auto",
	cssDeclarationSorterOrder: "smacss",
	plugins: ["prettier-plugin-svelte", "prettier-plugin-css-order"],
	overrides: [{ files: "*.svelte", options: { parser: "svelte" } }],
};

export default config;
