// Test suite for detecting false positives in dependency analysis
import { describe, it, expect, beforeAll } from 'vitest';
import { DependencyAnalyzer } from '../dist/index.js';
import path from 'path';

describe('False Positive Detection', () => {
	let analyzer;
	const config = {
		rootDir: path.resolve('/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop'),
		tsConfigPath: path.resolve(
			'/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop/tsconfig.json'
		),
		includePatterns: ['src/**/*.{ts,tsx,js,jsx,svelte}'],
		excludePatterns: ['node_modules/**', '.svelte-kit/**', 'dist/**'],
		entryPoints: []
	};

	beforeAll(() => {
		analyzer = new DependencyAnalyzer(config);
	});

	describe('Known good symbols that should not be flagged', () => {
		const knownGoodSymbols = [
			{
				name: 'TabContext',
				file: 'src/lib/tabs.ts',
				reason: 'Used in TabContent.svelte and Tabs.svelte'
			},
			{
				name: 'emptyConflictEntryPresence',
				file: 'src/lib/conflictEntryPresence.ts',
				reason: 'Used in conflicts.ts'
			},
			{
				name: 'getColorFromCommitState',
				file: 'src/components/lib.ts',
				reason: 'Used in BranchList.svelte and other components'
			},
			{
				name: 'getIconFromCommitState',
				file: 'src/components/lib.ts',
				reason: 'Used in BranchList.svelte'
			},
			{
				name: 'getConflictState',
				file: 'src/lib/conflictEntryPresence.ts',
				reason: 'Used in EditMode.svelte and internally'
			},
			{
				name: 'ConflictState',
				file: 'src/lib/conflictEntryPresence.ts',
				reason: 'Type used in EditMode.svelte and as return type'
			},
			{
				name: 'isQueryDefinition',
				file: 'src/lib/state/helpers.ts',
				reason: 'Used in butlerModule.ts'
			}
		];

		knownGoodSymbols.forEach((symbol) => {
			it(`should not flag ${symbol.name} as unused (${symbol.reason})`, async () => {
				const result = await analyzer.analyze();

				const unusedExport = result.unusedExports.find(
					(exp) => exp.name === symbol.name && exp.filePath.includes(symbol.file)
				);

				expect(unusedExport).toBeUndefined();
			}, 30000); // 30s timeout for full analysis
		});
	});

	describe('SvelteKit framework exports', () => {
		const frameworkExports = [
			{ name: 'ssr', file: 'src/routes/+layout.ts' },
			{ name: 'prerender', file: 'src/routes/+layout.ts' },
			{ name: 'csr', file: 'src/routes/+layout.ts' },
			{ name: 'load', file: 'src/routes/+layout.ts' }
		];

		frameworkExports.forEach((exp) => {
			it(`should not flag ${exp.name} as unused (SvelteKit framework export)`, async () => {
				const result = await analyzer.analyze();

				const unusedExport = result.unusedExports.find(
					(unused) => unused.name === exp.name && unused.filePath.includes(exp.file)
				);

				expect(unusedExport).toBeUndefined();
			}, 30000);
		});
	});

	describe('Files that should not be flagged as unused', () => {
		const usedFiles = [
			{
				file: 'src/lib/tabs.ts',
				reason: 'Exports TabContext interface used by tab components'
			},
			{
				file: 'src/lib/conflictEntryPresence.ts',
				reason: 'Exports functions used by conflicts.ts'
			}
		];

		usedFiles.forEach((fileInfo) => {
			it(`should not flag ${fileInfo.file} as unused (${fileInfo.reason})`, async () => {
				const result = await analyzer.analyze();

				const unusedFile = result.unusedFiles.find((file) => file.filePath.includes(fileInfo.file));

				expect(unusedFile).toBeUndefined();
			}, 30000);
		});
	});
});
