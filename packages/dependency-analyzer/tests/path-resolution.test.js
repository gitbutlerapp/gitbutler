// Test suite for path resolution functionality
import { describe, it, expect, beforeAll } from 'vitest';
import { EnhancedPathResolver } from '../dist/enhanced-path-resolver.js';
import path from 'path';

describe('PathResolver', () => {
	let resolver;
	const config = {
		rootDir: path.resolve('/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop'),
		tsConfigPath: path.resolve(
			'/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop/tsconfig.json'
		)
	};

	beforeAll(() => {
		resolver = new EnhancedPathResolver(config);
	});

	describe('SvelteKit path mappings', () => {
		const testCases = [
			{
				name: 'should resolve $lib/tabs import',
				import: '$lib/tabs',
				fromFile:
					'/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop/src/components/TabContent.svelte',
				expected: '/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop/src/lib/tabs.ts'
			},
			{
				name: 'should resolve $lib/conflictEntryPresence import',
				import: '$lib/conflictEntryPresence',
				fromFile:
					'/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop/src/lib/files/conflicts.ts',
				expected:
					'/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop/src/lib/conflictEntryPresence.ts'
			},
			{
				name: 'should resolve $components/lib import',
				import: '$components/lib',
				fromFile:
					'/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop/src/components/BranchList.svelte',
				expected:
					'/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop/src/components/lib.ts'
			}
		];

		testCases.forEach((testCase) => {
			it(testCase.name, () => {
				const resolved = resolver.resolveImportPath(testCase.import, testCase.fromFile);
				const withExtension = resolver.resolveFileExtension(resolved);
				expect(withExtension).toBe(testCase.expected);
			});
		});
	});

	describe('External dependency detection', () => {
		const testCases = [
			{ import: 'svelte', expected: true },
			{ import: '@gitbutler/ui/Button.svelte', expected: true },
			{ import: '$lib/tabs', expected: false },
			{ import: './relative', expected: false },
			{ import: '../relative', expected: false }
		];

		testCases.forEach((testCase) => {
			it(`should detect ${testCase.import} as ${testCase.expected ? 'external' : 'internal'}`, () => {
				const isExternal = resolver.isExternal(testCase.import);
				expect(isExternal).toBe(testCase.expected);
			});
		});
	});
});
