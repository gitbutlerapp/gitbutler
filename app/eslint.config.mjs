import typescriptEslint from '@typescript-eslint/eslint-plugin';
import squareSvelteStore from 'eslint-plugin-square-svelte-store';
import _import from 'eslint-plugin-import';
import { fixupPluginRules } from '@eslint/compat';
import globals from 'globals';
import tsParser from '@typescript-eslint/parser';
import parser from 'svelte-eslint-parser';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import js from '@eslint/js';
import { FlatCompat } from '@eslint/eslintrc';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const compat = new FlatCompat({
	baseDirectory: __dirname,
	recommendedConfig: js.configs.recommended,
	allConfig: js.configs.all
});

export default [
	{
		ignores: [
			'**/.DS_Store',
			'**/node_modules',
			'butler/target',
			'build',
			'.svelte-kit',
			'package',
			'e2e',
			'**/.env',
			'**/.env.*',
			'!**/.env.example',
			'**/pnpm-lock.yaml',
			'**/package-lock.json',
			'**/yarn.lock',
			'.github',
			'.vscode',
			'src-tauri',
			'.eslintrc.cjs',
			'svelte.config.js',
			'postcss.config.cjs',
			'playwright.config.ts',
			'**/.pnpm-store'
		]
	},
	...compat.extends(
		'eslint:recommended',
		'plugin:@typescript-eslint/recommended',
		'plugin:svelte/recommended',
		'plugin:svelte/prettier',
		'prettier'
	),
	{
		plugins: {
			'@typescript-eslint': typescriptEslint,
			'square-svelte-store': squareSvelteStore,
			import: fixupPluginRules(_import)
		},

		languageOptions: {
			globals: {
				...globals.browser,
				...globals.node
			},

			parser: tsParser,
			ecmaVersion: 2020,
			sourceType: 'module',

			parserOptions: {
				project: 'tsconfig.json',
				extraFileExtensions: ['.svelte']
			}
		},

		settings: {
			'import/extensions': ['.ts'],

			'import/parsers': {
				'@typescript-eslint/parser': ['.ts']
			},

			'import/resolver': {
				typescript: {
					project: ['./tsconfig.json', './.svelte-kit/tsconfig.json']
				}
			}
		},

		rules: {
			eqeqeq: ['error', 'always'],
			'import/no-cycle': 'error',

			'import/order': [
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

			'import/no-unresolved': [
				'error',
				{
					ignore: ['^\\$app', '^\\$env']
				}
			],

			'func-style': [2, 'declaration'],
			'svelte/no-at-html-tags': 'off',
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
			'@typescript-eslint/await-thenable': 'error'
		}
	},
	{
		files: ['**/*.svelte'],

		languageOptions: {
			parser: parser,
			ecmaVersion: 5,
			sourceType: 'script',

			parserOptions: {
				parser: '@typescript-eslint/parser'
			}
		}
	}
];

