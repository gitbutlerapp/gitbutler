// Test suite for export removal precision and safety
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { DependencyAnalyzer } from '../dist/index.js';
import path from 'path';
import fs from 'fs';
import os from 'os';

describe('Export Removal Precision', () => {
	let testDir;

	beforeEach(() => {
		// Create unique test directory for each test
		testDir = fs.mkdtempSync(path.join(os.tmpdir(), 'export-removal-test-'));
	});

	afterEach(() => {
		// Clean up test directory
		if (fs.existsSync(testDir)) {
			fs.rmSync(testDir, { recursive: true, force: true });
		}
	});

	it('should only remove unused exports, not all exports', async () => {
		// Create a test file with mixed used/unused exports
		const testFile = path.join(testDir, 'mixed-exports.ts');
		const originalContent = `// Used export
export function usedFunction() {
    return 'I am used';
}

// Unused export
export function unusedFunction() {
    return 'I am not used';
}

// Used export in export list
export const usedConstant = 'also used';

// Unused export in same list
export const unusedConstant = 'not used';

// Export list with mixed usage
export { usedFunction as renamedUsed, unusedFunction as renamedUnused };

// Used class
export class UsedClass {
    method() {}
}

// Unused interface
export interface UnusedInterface {
    prop: string;
}

// Internal usage to make some exports "used"
console.log(usedFunction());
console.log(usedConstant);
const instance = new UsedClass();
console.log(instance);
`;

		fs.writeFileSync(testFile, originalContent);

		// Create a consumer file that imports and uses some exports
		const consumerFile = path.join(testDir, 'consumer.ts');
		fs.writeFileSync(
			consumerFile,
			`
import { usedFunction, usedConstant, UsedClass } from './mixed-exports';

// Use the imported symbols
console.log(usedFunction());
console.log(usedConstant);
const obj = new UsedClass();
`
		);

		// Create tsconfig
		const tsconfig = path.join(testDir, 'tsconfig.json');
		fs.writeFileSync(
			tsconfig,
			JSON.stringify(
				{
					compilerOptions: {
						target: 'ES2020',
						module: 'ESNext',
						moduleResolution: 'node',
						strict: true,
						esModuleInterop: true,
						skipLibCheck: true,
						forceConsistentCasingInFileNames: true
					},
					include: ['*.ts']
				},
				null,
				2
			)
		);

		const config = {
			rootDir: testDir,
			tsConfigPath: tsconfig,
			includePatterns: ['*.ts'],
			excludePatterns: [],
			entryPoints: [consumerFile], // Make consumer an entry point
			followDynamicImports: true,
			analyzeTypeUsage: true,
			includeSvelteComponents: false,
			includeTestFiles: false,
			removeExports: true
		};

		const analyzer = new DependencyAnalyzer(config);
		await analyzer.analyze();

		// Read the modified file
		const modifiedContent = fs.readFileSync(testFile, 'utf-8');

		// Verify that used exports are still exported
		expect(modifiedContent).toContain('export function usedFunction()');
		expect(modifiedContent).toContain('export const usedConstant');
		expect(modifiedContent).toContain('export class UsedClass');

		// Verify that unused exports have been modified
		expect(modifiedContent).toContain('function unusedFunction()'); // export removed
		expect(modifiedContent).not.toContain('export function unusedFunction()');

		expect(modifiedContent).toContain('const unusedConstant'); // export removed
		expect(modifiedContent).not.toContain('export const unusedConstant');

		expect(modifiedContent).toContain('interface UnusedInterface'); // export removed
		expect(modifiedContent).not.toContain('export interface UnusedInterface');

		console.log('Original:', originalContent.substring(0, 200) + '...');
		console.log('Modified:', modifiedContent.substring(0, 200) + '...');
	});

	it('should handle export lists correctly', async () => {
		const testFile = path.join(testDir, 'export-lists.ts');
		const originalContent = `function usedFn() { return 'used'; }
function unusedFn1() { return 'unused1'; }
function unusedFn2() { return 'unused2'; }

// Mixed export list - should remove unused ones only
export { usedFn, unusedFn1, unusedFn2 };

// Use one function to make it "used"
console.log(usedFn());
`;

		fs.writeFileSync(testFile, originalContent);

		const tsconfig = path.join(testDir, 'tsconfig.json');
		fs.writeFileSync(
			tsconfig,
			JSON.stringify(
				{
					compilerOptions: {
						target: 'ES2020',
						module: 'ESNext',
						moduleResolution: 'node'
					},
					include: ['*.ts']
				},
				null,
				2
			)
		);

		const config = {
			rootDir: testDir,
			tsConfigPath: tsconfig,
			includePatterns: ['*.ts'],
			excludePatterns: [],
			entryPoints: [],
			removeExports: true
		};

		const analyzer = new DependencyAnalyzer(config);
		await analyzer.analyze();

		const modifiedContent = fs.readFileSync(testFile, 'utf-8');

		// Should only export the used function
		expect(modifiedContent).toContain('export { usedFn }');
		// Function declarations should remain, but not be exported
		expect(modifiedContent).toContain('function unusedFn1()');
		expect(modifiedContent).toContain('function unusedFn2()');
		// But they should not be in the export list
		expect(modifiedContent).not.toContain('export { usedFn, unusedFn1, unusedFn2 }');

		console.log('Export list test - Modified content:');
		console.log(modifiedContent);
	});

	it('should preserve exports that are re-exported from other files', async () => {
		// Create a file that exports something
		const sourceFile = path.join(testDir, 'source.ts');
		fs.writeFileSync(
			sourceFile,
			`
export function sourceFunction() {
    return 'from source';
}
`
		);

		// Create a file that re-exports from source
		const reexportFile = path.join(testDir, 'reexport.ts');
		fs.writeFileSync(
			reexportFile,
			`
export { sourceFunction } from './source';

export function localFunction() {
    return 'local';
}
`
		);

		// Create a consumer that uses the re-exported function
		const consumerFile = path.join(testDir, 'consumer.ts');
		fs.writeFileSync(
			consumerFile,
			`
import { sourceFunction } from './reexport';
console.log(sourceFunction());
`
		);

		const tsconfig = path.join(testDir, 'tsconfig.json');
		fs.writeFileSync(
			tsconfig,
			JSON.stringify(
				{
					compilerOptions: {
						target: 'ES2020',
						module: 'ESNext',
						moduleResolution: 'node'
					},
					include: ['*.ts']
				},
				null,
				2
			)
		);

		const config = {
			rootDir: testDir,
			tsConfigPath: tsconfig,
			includePatterns: ['*.ts'],
			excludePatterns: [],
			entryPoints: [consumerFile],
			removeExports: true
		};

		const analyzer = new DependencyAnalyzer(config);
		await analyzer.analyze();

		const sourceContent = fs.readFileSync(sourceFile, 'utf-8');
		const reexportContent = fs.readFileSync(reexportFile, 'utf-8');

		// Source function should still be exported (used via re-export)
		expect(sourceContent).toContain('export function sourceFunction()');

		// Re-export should still exist
		expect(reexportContent).toContain('export { sourceFunction }');

		// Local unused function should have export removed
		expect(reexportContent).toContain('function localFunction()');
		expect(reexportContent).not.toContain('export function localFunction()');
	});

	it('should not modify files when no unused exports are found', async () => {
		const testFile = path.join(testDir, 'all-used.ts');
		const originalContent = `export function usedFunction() {
    return 'used';
}

export const usedConstant = 'also used';

// Use everything
console.log(usedFunction());
console.log(usedConstant);
`;

		fs.writeFileSync(testFile, originalContent);

		const tsconfig = path.join(testDir, 'tsconfig.json');
		fs.writeFileSync(
			tsconfig,
			JSON.stringify(
				{
					compilerOptions: {
						target: 'ES2020',
						module: 'ESNext'
					},
					include: ['*.ts']
				},
				null,
				2
			)
		);

		const config = {
			rootDir: testDir,
			tsConfigPath: tsconfig,
			removeExports: true
		};

		const analyzer = new DependencyAnalyzer(config);
		await analyzer.analyze();

		const modifiedContent = fs.readFileSync(testFile, 'utf-8');

		// Content should be unchanged since all exports are used
		expect(modifiedContent).toBe(originalContent);
	});
});
