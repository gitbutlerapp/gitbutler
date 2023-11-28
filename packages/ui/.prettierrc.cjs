module.exports = {
	useTabs: true,
	singleQuote: true,
	trailingComma: 'none',
	printWidth: 100,
	plugins: [import('prettier-plugin-svelte'), import('prettier-plugin-tailwindcss')],
	overrides: [{ files: '*.svelte', options: { parser: 'svelte' } }]
};
