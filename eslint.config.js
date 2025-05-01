import prettier from 'eslint-config-prettier';
import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import globals from 'globals';
import ts from 'typescript-eslint';
import pluginImportX from 'eslint-plugin-import-x';
import noRelativeImportPaths from '@gitbutler/no-relative-imports';

import path from 'node:path';

const rootDir = path.resolve();

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
				projectService: true
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
						// Add SvelteKit paths
						{ pattern: '$lib/**', group: 'internal' },
						{ pattern: '$components/**', group: 'internal' },
						{ pattern: '$app/**', group: 'internal' }
					],
					// Ensure certain import types are only categorized by their type
					pathGroupsExcludedImportTypes: ['builtin', 'external', 'object', 'type']
				}
			],
			'import-x/no-relative-packages': 'error', // Don't allow packages to have relative imports between each other
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
			'svelte/no-unused-props': 'error'
		},

		settings: {
			'import-x/extensions': ['.ts'],
			'import-x/parsers': {
				'@typescript-eslint/parser': ['.ts']
			},
			'import-x/resolver': {
				typescript: {
					project: [
						'./apps/**/tsconfig.json',
						'./apps/desktop/.svelte-kit/tsconfig.json',
						'./apps/web/.svelte-kit/tsconfig.json',
						'./packages/**/tsconfig.json',
						'./packages/ui/.svelte-kit/tsconfig.json',
						'./packages/shared/.svelte-kit/tsconfig.json'
					]
				}
			}
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
			'**/eslint.config.js',
			'**/svelte.config.js',
			'**/.pnpm-store',
			'**/vite.config.ts.timestamp-*',
			'!.storybook',
			'target/',
			'crates/',
			'packages/ui/storybook-static',
			// Storybook Meta type wrapper
			'packages/ui/src/stories/**/*.stories.ts'
		]
	}
);
