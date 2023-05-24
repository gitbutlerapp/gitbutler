module.exports = {
	root: true,
	parser: '@typescript-eslint/parser',
	extends: [
		'eslint:recommended',
		'plugin:@typescript-eslint/recommended',
		'plugin:svelte/recommended',
		'plugin:svelte/prettier',
		'prettier'
	],
	plugins: ['svelte', '@typescript-eslint', 'square-svelte-store'],
	parserOptions: {
		sourceType: 'module',
		ecmaVersion: 2020,
		project: 'tsconfig.json',
		extraFileExtensions: ['.svelte']
	},
	overrides: [
		{
			files: ['*.svelte'],
			parser: 'svelte-eslint-parser',
			parserOptions: {
				parser: '@typescript-eslint/parser'
			}
		}
	],
	env: {
		browser: true,
		es2017: true,
		node: true
	},
	rules: {
		'svelte/no-at-html-tags': 'off',
		'@typescript-eslint/no-namespace': 'off',
		'@typescript-eslint/no-empty-function': 'off',
		'@typescript-eslint/no-explicit-any': 'off',
		'square-svelte-store/use-square-svelte-stores': 'error'
	}
};
