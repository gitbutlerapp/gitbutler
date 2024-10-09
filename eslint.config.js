import js from '@eslint/js';
import eslintConfigPrettier from 'eslint-config-prettier';
import eslintPluginSvelte from 'eslint-plugin-svelte';
import globals from 'globals';
import tsEslint from 'typescript-eslint';
import pluginImportX from 'eslint-plugin-import-x';
// Flat config support: https://github.com/storybookjs/eslint-plugin-storybook/pull/156
import storybook from 'eslint-plugin-storybook';
import svelteParser from 'svelte-eslint-parser';
import svelteConfig from './apps/desktop/svelte.config.js';

export default tsEslint.config(
	js.configs.recommended,
	...storybook.configs['flat/recommended'],
	...tsEslint.configs.recommended,
	...eslintPluginSvelte.configs['flat/recommended'],
	eslintConfigPrettier,
	...eslintPluginSvelte.configs['flat/prettier'],
	{
		files: ['apps/desktop/e2e/**'],
		languageOptions: {
			ecmaVersion: 2021,
			sourceType: 'module',
			globals: {
				...globals.node,
				...globals.browser,
				...globals.mocha,
				...globals.chai,
				$: false
			}
		}
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts'],
		languageOptions: {
			ecmaVersion: 2021,
			sourceType: 'module',
			globals: {
				...globals.node,
				...globals.browser
			},
			parser: svelteParser,
			parserOptions: {
				parser: tsEslint.parser,
				svelteConfig,
				extraFileExtensions: ['.svelte'],
				svelteFeatures: {
					runes: true,
					experimentalGenerics: true
				}
			}
		}
	},
	{
		languageOptions: {
			parserOptions: {
				parser: tsEslint.parser,
				projectService: true,
				extraFileExtensions: ['.svelte', '.svelte.ts']
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

			'import-x/no-relative-packages': 'error', // Don't allow packages to have relative imports between each other
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
						'./apps/desktop/tsconfig.json',
						'./apps/desktop/.svelte-kit/tsconfig.json',
						'./apps/web/tsconfig.json',
						'./apps/web/.svelte-kit/tsconfig.json',
						'./packages/**/tsconfig.json',
						'./packages/ui/.svelte-kit/tsconfig.json'
					]
				}
			}
		},
		plugins: {
			'import-x': pluginImportX
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
			'**/svelte.config.js',
			'**/vite.config.ts',
			'**/.pnpm-store',
			'**/vite.config.ts.timestamp-*',
			'!.storybook',
			'target/',
			'crates/',
			'packages/ui/storybook-static'
		]
	}
);
