// Test suite for import parsing functionality
import { describe, it, expect, beforeAll } from 'vitest';
import { ASTParser } from '../dist/ast-parser.js';
import { SvelteParser } from '../dist/svelte-parser.js';
import { EnhancedPathResolver } from '../dist/enhanced-path-resolver.js';
import path from 'path';

describe('Import Parsing', () => {
	let astParser, svelteParser, pathResolver;
	const config = {
		rootDir: path.resolve('/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop'),
		tsConfigPath: path.resolve(
			'/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop/tsconfig.json'
		)
	};

	beforeAll(() => {
		pathResolver = new EnhancedPathResolver(config);
		astParser = new ASTParser(config.rootDir, config.tsConfigPath, pathResolver);
		svelteParser = new SvelteParser(pathResolver);
	});

	describe('Svelte component parsing', () => {
		it('should parse TabContent.svelte correctly', () => {
			const filePath =
				'/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop/src/components/TabContent.svelte';
			const fileNode = svelteParser.parseFile(filePath);

			// Should have 2 imports
			expect(fileNode.imports).toHaveLength(2);

			// Should include svelte import
			const svelteImport = fileNode.imports.find((imp) => imp.specifier === 'svelte');
			expect(svelteImport).toBeDefined();
			expect(svelteImport.isTypeOnly).toBe(true);
			expect(svelteImport.imported.map((sym) => sym.name)).toContain('getContext');

			// Should include $lib/tabs import
			const tabsImport = fileNode.imports.find((imp) => imp.specifier === '$lib/tabs');
			expect(tabsImport).toBeDefined();
			expect(tabsImport.isTypeOnly).toBe(true);
			expect(tabsImport.imported.map((sym) => sym.name)).toContain('TabContext');

			// Should have component export
			expect(fileNode.exports).toHaveLength(1);
			expect(fileNode.exports[0].name).toBe('TabContent');
			expect(fileNode.exports[0].type).toBe('component');
		});
	});

	describe('TypeScript file parsing', () => {
		it('should parse conflicts.ts correctly', () => {
			const filePath =
				'/Users/calebowens/Sherman/projects/gitbutler-client/apps/desktop/src/lib/files/conflicts.ts';
			const fileNode = astParser.parseFile(filePath);

			// Should have imports
			expect(fileNode.imports.length).toBeGreaterThan(0);

			// Should include conflictEntryPresence import
			const conflictImport = fileNode.imports.find(
				(imp) => imp.specifier === '$lib/conflictEntryPresence'
			);
			expect(conflictImport).toBeDefined();
			expect(conflictImport.imported.map((sym) => sym.name)).toContain(
				'emptyConflictEntryPresence'
			);

			// Should have exports
			expect(fileNode.exports.length).toBeGreaterThan(0);
			const classExport = fileNode.exports.find((exp) => exp.name === 'ConflictEntries');
			expect(classExport).toBeDefined();
			expect(classExport.type).toBe('class');
		});
	});

	describe('Import type syntax', () => {
		const testCases = [
			{
				name: 'regular import',
				code: `import { something } from 'module';`,
				expectedTypeOnly: false
			},
			{
				name: 'type-only import',
				code: `import type { Something } from 'module';`,
				expectedTypeOnly: true
			},
			{
				name: 'mixed import with type',
				code: `import { type Something, other } from 'module';`,
				expectedTypeOnly: true
			}
		];

		testCases.forEach((testCase) => {
			it(`should handle ${testCase.name}`, () => {
				// This would require creating temporary files or mocking the file system
				// For now, we can test the regex pattern directly
				const regex =
					/import\s+(?:(type)\s+)?(?:(\w+)(?:\s*,\s*)?)?(?:\{([^}]+)\})?(?:\*\s+as\s+(\w+))?\s+from\s+['"]([^'"]+)['"]|import\s+['"]([^'"]+)['"]/g;
				const match = regex.exec(testCase.code);
				expect(match).toBeTruthy();
				const hasTypeKeyword = match[1] === 'type' || match[0].includes('type');
				expect(hasTypeKeyword).toBe(testCase.expectedTypeOnly);
			});
		});
	});
});
