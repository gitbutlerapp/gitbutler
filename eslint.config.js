import noRelativeImportPaths from '@gitbutler/no-relative-imports';
import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import { createNextImportResolver } from 'eslint-import-resolver-next';
import pluginImportX from 'eslint-plugin-import-x';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';

export default ts.config(
	js.configs.recommended,
	...ts.configs.recommended,
	...svelte.configs.recommended,
	prettier,
	...svelte.configs.prettier,
	{
		languageOptions: {
			globals: {
				...globals.browser,
				...globals.node
			},
			parserOptions: {
				projectService: {
					// This prevents lint error when running eslint from
					// subdirectories, ignoring the root tsconfig.json
					allowDefaultProject: [
						'svelte.config.js',
						'packages/ui/.storybook/*.ts',
						'packages/ui/playwright-ct.config.ts'
					]
				}
			}
		},
		rules: {
			'no-console': ['error', { allow: ['warn', 'error'] }],
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
			'@typescript-eslint/return-await': ['error', 'always'],
			'@typescript-eslint/promise-function-async': 'error',
			'@typescript-eslint/await-thenable': 'error',

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
					// Add explicit pathGroups to define what imports go in which groups
					pathGroups: [
						// Define monorepo paths as internal
						{ pattern: 'apps/**', group: 'internal' },
						{ pattern: 'packages/**', group: 'internal' },
						{ pattern: 'e2e/**', group: 'internal' },
						// Add SvelteKit paths
						{ pattern: '$lib/**', group: 'internal' },
						{ pattern: '$components/**', group: 'internal' },
						{ pattern: '$app/**', group: 'internal' }
					],
					// Ensure certain import types are only categorized by their type
					pathGroupsExcludedImportTypes: ['builtin', 'external', 'object', 'type']
				}
			],
			'func-style': [2, 'declaration'],
			'no-return-await': 'off',
			'svelte/no-at-html-tags': 'off',
			'svelte/button-has-type': [
				'error',
				{
					button: true,
					submit: true,
					reset: true
				}
			],
			'no-relative-import-paths/no-relative-import-paths': 'error',
			'no-undef': 'off', // eslint faq advises `no-undef` turned off for typescript projects.
			'svelte/require-each-key': 'off',
			'svelte/no-inspect': 'error',
			'svelte/no-at-debug-tags': 'error',
			'svelte/no-unused-props': 'error',
			'svelte/prefer-svelte-reactivity': 'off'
		},

		settings: {
			'import-x/extensions': ['.ts', '.js', '.mjs'],
			'import-x/parsers': {
				'@typescript-eslint/parser': ['.ts', '.js', '.mjs']
			},
			'import-x/resolver-next': [createNextImportResolver()]
		},
		plugins: {
			'import-x': pluginImportX,
			'no-relative-import-paths': noRelativeImportPaths
		}
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts'],
		...ts.configs.disableTypeChecked
	},
	{
		files: ['**/*.svelte', '**/*.svelte.ts'],
		languageOptions: {
			parserOptions: {
				parser: ts.parser
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
			'**/static',
			'**/dist',
			'**/.svelte-kit',
			'**/package',
			'**/.env',
			'**/.env.*',
			'!**/.env.example',
			'**/pnpm-lock.yaml',
			'**/package-lock.json',
			'**/yarn.lock',
			'.github',
			'.vscode',
			'**/.pnpm-store',
			'**/vite.config.ts.timestamp-*',
			'!**/.storybook',
			'target/',
			'crates/',
			'packages/ui/storybook-static',
			// Storybook Meta type wrapper
			'packages/ui/src/stories/**/*.stories.ts',
			'e2e/playwright/fixtures'
		]
	}
);
