import js from '@eslint/js';
import tsParser from '@typescript-eslint/parser';
import eslintConfigPrettier from 'eslint-config-prettier';
import eslintPluginSvelte from 'eslint-plugin-svelte';
import globals from 'globals';
import tsEslint from 'typescript-eslint';
import pluginImportX from 'eslint-plugin-import-x';
// Flat config support: https://github.com/storybookjs/eslint-plugin-storybook/pull/156
import storybook from 'eslint-plugin-storybook';
import svelteParser from 'svelte-eslint-parser';

export default tsEslint.config(
	js.configs.recommended,
	...storybook.configs['flat/recommended'],
	...tsEslint.configs.recommended,
	...eslintPluginSvelte.configs['flat/recommended'],
	eslintConfigPrettier,
	...eslintPluginSvelte.configs['flat/prettier'],
	{
		files: ['**/*.svelte'],
		languageOptions: {
			ecmaVersion: 2021,
			sourceType: 'module',
			globals: {
				...globals.node,
				...globals.browser,
				$state: 'readonly',
				$derived: 'readonly',
				$props: 'readonly',
				$bindable: 'readonly',
				$inspect: 'readonly',
				$host: 'readonly'
			},
			parser: svelteParser,
			parserOptions: {
				parser: tsParser,
				extraFileExtensions: ['.svelte']
			}
		}
	},
	{
		ignores: [
			'**/.*', // dotfiles aren't ignored by default in FlatConfig
			'.*', // dotfiles aren't ignored by default in FlatConfig
			'**/.DS_Store',
			'**/node_modules',
			'**/butler/target',
			'**/build',
			'**/dist',
			'.svelte-kit',
			'**/package',
			'**/e2e',
			'**/.env',
			'**/.env.*',
			'!**/.env.example',
			'**/pnpm-lock.yaml',
			'**/package-lock.json',
			'**/yarn.lock',
			'.github',
			'.vscode',
			'src-tauri',
			'**/eslint.config.js',
			'**/eslint.config.mjs',
			'**/svelte.config.js',
			'**/postcss.config.cjs',
			'**/playwright.config.ts',
			'**/.pnpm-store',
			'!.storybook',
			'target/',
			'crates/'
		]
	},
	{
		languageOptions: {
			parserOptions: {
				parser: tsEslint.parser,
				project: ['./packages/**/tsconfig.json', './app/tsconfig.json'],
				extraFileExtensions: ['.svelte']
			}
		},
		rules: {
			eqeqeq: ['error', 'always'],
			'import-x/no-cycle': 'error',

			'import-x/order': [
				'error',
				{
					alphabetize: {
						order: 'asc',
						orderImportKind: 'asc',
						caseInsensitive: false
					},

					groups: [
						'index',
						'sibling',
						'parent',
						'internal',
						'external',
						'builtin',
						'object',
						'type'
					],

					'newlines-between': 'never'
				}
			],

			'import-x/no-unresolved': [
				'error',
				{
					ignore: ['^\\$app', '^\\$env']
				}
			],

			'func-style': [2, 'declaration'],
			'@typescript-eslint/no-namespace': 'off',
			'@typescript-eslint/no-empty-function': 'off',
			'@typescript-eslint/no-explicit-any': 'off',

			'@typescript-eslint/no-unused-vars': [
				'error',
				{
					argsIgnorePattern: '^_',
					varsIgnorePattern: '^_',
					caughtErrorsIgnorePattern: '^_'
				}
			],

			'no-return-await': 'off',
			'@typescript-eslint/return-await': ['error', 'always'],
			'@typescript-eslint/promise-function-async': 'error',
			'@typescript-eslint/await-thenable': 'error',
			'svelte/no-at-html-tags': 'off'
		},
		settings: {
			'import-x/extensions': ['.ts'],
			'import-x/parsers': {
				'@typescript-eslint/parser': ['.ts']
			},
			'import-x/resolver': {
				typescript: {
					project: [
						'./app/tsconfig.json',
						'./app/.svelte-kit/tsconfig.json',
						'./packages/**/tsconfig.json',
						'./packages/ui/.svelte-kit/tsconfig.json'
					]
				}
			}
		},
		plugins: {
			'import-x': pluginImportX
		}
	}
);
