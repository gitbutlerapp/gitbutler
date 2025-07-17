import { DeadCodeAnalyzer } from './dead-code-analyzer.js';
import { GraphBuilder } from './graph-builder.js';
import { Reporter, type ReportOptions } from './reporter.js';
import * as fs from 'fs';
import * as path from 'path';
import type { AnalysisConfig, DependencyNode } from './types.js';

export * from './types.js';
export { GraphBuilder } from './graph-builder.js';
export { DeadCodeAnalyzer } from './dead-code-analyzer.js';
export { Reporter, type ReportOptions } from './reporter.js';
export { PathResolver } from './path-resolver.js';
export { ASTParser } from './ast-parser.js';
export { SvelteParser } from './svelte-parser.js';
export { SvelteKitAnalyzer } from './sveltekit-analyzer.js';

export class DependencyAnalyzer {
	private config: AnalysisConfig;
	private graphBuilder: GraphBuilder;
	private deadCodeAnalyzer: DeadCodeAnalyzer;

	constructor(config: Partial<AnalysisConfig> = {}) {
		this.config = this.normalizeConfig(config);
		this.graphBuilder = new GraphBuilder(this.config);
		this.deadCodeAnalyzer = new DeadCodeAnalyzer(this.graphBuilder.pathResolver);
	}

	async analyze() {
		console.log('🔍 Starting dependency analysis...');

		// Build dependency graph
		const graph = await this.graphBuilder.build();
		console.log(`📊 Analyzed ${graph.files.size} files with ${graph.nodes.size} symbols`);

		// Analyze dead code
		const result = this.deadCodeAnalyzer.analyze(graph);
		console.log(
			`💀 Found ${result.unusedExports.length} unused exports and ${result.unusedFiles.length} unused files`
		);

		// Remove exports if requested
		if (this.config.removeExports) {
			await this.removeUnusedExports(result.unusedExports);
		}

		return result;
	}

	async generateReport(reportOptions: Partial<ReportOptions> = {}) {
		const result = await this.analyze();

		const options: ReportOptions = {
			format: reportOptions.format || 'console',
			verbose: reportOptions.verbose || false,
			includeStats: reportOptions.includeStats !== false,
			includeCircularDeps: reportOptions.includeCircularDeps !== false,
			showTopUnused: reportOptions.showTopUnused || 10,
			outputFile: reportOptions.outputFile
		};

		const reporter = new Reporter(options);
		const report = reporter.generateReport(result);
		reporter.saveReport(report);

		return result;
	}

	private normalizeConfig(config: Partial<AnalysisConfig>): AnalysisConfig {
		const rootDir = config.rootDir || process.cwd();
		const tsConfigPath = config.tsConfigPath || path.join(rootDir, 'tsconfig.json');

		return {
			rootDir,
			tsConfigPath,
			includePatterns: config.includePatterns || [
				'src/**/*.ts',
				'src/**/*.tsx',
				'src/**/*.js',
				'src/**/*.jsx',
				'src/**/*.svelte'
			],
			excludePatterns: config.excludePatterns || [
				'node_modules/**',
				'dist/**',
				'build/**',
				'.svelte-kit/**',
				'**/*.test.*',
				'**/*.spec.*',
				'**/*.d.ts'
			],
			entryPoints:
				config.entryPoints ||
				[
					// Let SvelteKit analyzer auto-detect entry points
				],
			followDynamicImports: config.followDynamicImports ?? true,
			analyzeTypeUsage: config.analyzeTypeUsage ?? true,
			includeSvelteComponents: config.includeSvelteComponents ?? true,
			includeTestFiles: config.includeTestFiles ?? false,
			removeExports: config.removeExports ?? false
		};
	}

	private async removeUnusedExports(unusedExports: DependencyNode[]): Promise<void> {
		console.log('🧹 Removing unused export statements...');
		console.log(`⚠️  This will modify ${unusedExports.length} exports across multiple files!`);
		console.log('   Please ensure you have committed your changes before proceeding.');
		console.log();

		// Group unused exports by file for batch processing
		const exportsByFile = new Map<string, DependencyNode[]>();
		for (const exportNode of unusedExports) {
			if (!exportsByFile.has(exportNode.filePath)) {
				exportsByFile.set(exportNode.filePath, []);
			}
			exportsByFile.get(exportNode.filePath)!.push(exportNode);
		}

		let filesModified = 0;
		let exportsRemoved = 0;

		for (const [filePath, exports] of exportsByFile) {
			try {
				const originalContent = fs.readFileSync(filePath, 'utf-8');
				let modifiedContent = originalContent;
				let fileWasModified = false;
				let fileExportsRemoved = 0;

				// Group exports by their export statements (same line number)
				const exportsByLine = new Map<number, DependencyNode[]>();
				for (const exportNode of exports) {
					const lineNumber = exportNode.metadata.startLine || 0;
					if (!exportsByLine.has(lineNumber)) {
						exportsByLine.set(lineNumber, []);
					}
					exportsByLine.get(lineNumber)!.push(exportNode);
				}

				// Sort export statements by line number (descending) to avoid line number shifts
				const sortedLines = Array.from(exportsByLine.entries()).sort((a, b) => b[0] - a[0]);

				for (const [lineNumber, exportsOnLine] of sortedLines) {
					const result = this.removeExportsFromContent(modifiedContent, exportsOnLine);
					if (result.modified) {
						modifiedContent = result.content;
						fileWasModified = true;
						fileExportsRemoved += exportsOnLine.length;
					}
				}

				if (fileWasModified) {
					fs.writeFileSync(filePath, modifiedContent, 'utf-8');
					filesModified++;
					exportsRemoved += fileExportsRemoved;

					const relativePath = path.relative(this.config.rootDir, filePath);
					console.log(`   ✓ Modified ${relativePath} (removed ${fileExportsRemoved} exports)`);
				}
			} catch (error) {
				const relativePath = path.relative(this.config.rootDir, filePath);
				console.warn(`   ⚠ Failed to modify ${relativePath}: ${error}`);
			}
		}

		console.log(
			`🎯 Export removal complete: ${exportsRemoved} exports removed from ${filesModified} files`
		);
	}

	private removeExportsFromContent(
		content: string,
		exportNodes: DependencyNode[]
	): { content: string; modified: boolean } {
		if (exportNodes.length === 0) {
			return { content, modified: false };
		}

		// Use the first export node to determine the export statement location
		const firstExportNode = exportNodes[0];

		// If there's only one export node, use the original single-export logic
		if (exportNodes.length === 1) {
			return this.removeExportFromContent(content, firstExportNode);
		}

		// Handle multiple exports from the same export statement (export list)
		const lines = content.split('\n');
		const startLine = (firstExportNode.metadata.startLine || 1) - 1; // Convert to 0-based
		const endLine =
			(firstExportNode.metadata.endLine || firstExportNode.metadata.startLine || 1) - 1;

		if (startLine < 0 || startLine >= lines.length) {
			return { content, modified: false };
		}

		// Get the export line(s)
		const exportLines = lines.slice(startLine, endLine + 1);
		const exportText = exportLines.join('\n');

		// Handle export lists: export { foo, bar, baz }
		const exportListPattern = /export\s*\{\s*([^}]+)\s*\}/;
		const match = exportText.match(exportListPattern);
		if (match) {
			const exportList = match[1];
			const exports = exportList
				.split(',')
				.map((e) => e.trim())
				.filter((e) => e);

			// Remove all target exports from the list
			const exportNamesToRemove = new Set(exportNodes.map((node) => node.name));
			const filteredExports = exports.filter((exp) => {
				const cleanExp = exp.replace(/\s+as\s+\w+/, '').trim(); // Remove 'as alias' part
				return !exportNamesToRemove.has(cleanExp);
			});

			if (filteredExports.length === 0) {
				// Remove entire export statement
				lines.splice(startLine, endLine - startLine + 1);
			} else {
				// Update export list
				const newExportText = exportText.replace(
					exportListPattern,
					`export { ${filteredExports.join(', ')} }`
				);
				lines.splice(startLine, endLine - startLine + 1, ...newExportText.split('\n'));
			}
			return { content: lines.join('\n'), modified: true };
		}

		// If it's not an export list, fall back to single export removal for each node
		let modifiedContent = content;
		let wasModified = false;

		// Process exports in reverse order to avoid line number shifts
		const sortedExports = exportNodes.sort(
			(a, b) => (b.metadata.startLine || 0) - (a.metadata.startLine || 0)
		);

		for (const exportNode of sortedExports) {
			const result = this.removeExportFromContent(modifiedContent, exportNode);
			if (result.modified) {
				modifiedContent = result.content;
				wasModified = true;
			}
		}

		return { content: modifiedContent, modified: wasModified };
	}

	private removeExportFromContent(
		content: string,
		exportNode: DependencyNode
	): { content: string; modified: boolean } {
		const lines = content.split('\n');
		const startLine = (exportNode.metadata.startLine || 1) - 1; // Convert to 0-based
		const endLine = (exportNode.metadata.endLine || exportNode.metadata.startLine || 1) - 1;

		// Handle different export patterns
		if (startLine < 0 || startLine >= lines.length) {
			return { content, modified: false };
		}

		// Get the export line(s)
		const exportLines = lines.slice(startLine, endLine + 1);
		const exportText = exportLines.join('\n');

		// Pattern 1: Remove 'export' keyword from declarations
		// export function foo() -> function foo()
		// export class Bar -> class Bar
		// export interface Baz -> interface Baz
		// export type Qux -> type Qux
		const declarationPattern = /^(\s*)export\s+(function|class|interface|type|const|let|var)\s+/;
		if (declarationPattern.test(exportText)) {
			const newText = exportText.replace(declarationPattern, '$1$2 ');
			lines.splice(startLine, endLine - startLine + 1, ...newText.split('\n'));
			return { content: lines.join('\n'), modified: true };
		}

		// Pattern 2: Remove from export lists
		// export { foo, bar } -> export { bar } (if removing foo)
		// export { foo } -> remove entire line
		const exportListPattern = /export\s*\{\s*([^}]+)\s*\}/;
		const match = exportText.match(exportListPattern);
		if (match) {
			const exportList = match[1];
			const exports = exportList
				.split(',')
				.map((e) => e.trim())
				.filter((e) => e);

			// Remove the target export from the list
			const filteredExports = exports.filter((exp) => {
				const cleanExp = exp.replace(/\s+as\s+\w+/, '').trim(); // Remove 'as alias' part
				return cleanExp !== exportNode.name;
			});

			if (filteredExports.length === 0) {
				// Remove entire export statement
				lines.splice(startLine, endLine - startLine + 1);
			} else {
				// Update export list
				const newExportText = exportText.replace(
					exportListPattern,
					`export { ${filteredExports.join(', ')} }`
				);
				lines.splice(startLine, endLine - startLine + 1, ...newExportText.split('\n'));
			}
			return { content: lines.join('\n'), modified: true };
		}

		// Pattern 3: Default exports
		// export default foo -> remove entire line
		if (/export\s+default\s+/.test(exportText)) {
			lines.splice(startLine, endLine - startLine + 1);
			return { content: lines.join('\n'), modified: true };
		}

		// If we can't handle the pattern, don't modify
		return { content, modified: false };
	}
}

// Convenience function for quick analysis
export async function analyzeProject(
	rootDir: string,
	options: Partial<AnalysisConfig & ReportOptions> = {}
): Promise<void> {
	const {
		format,
		verbose,
		includeStats,
		includeCircularDeps,
		showTopUnused,
		outputFile,
		...analysisConfig
	} = options;

	const analyzer = new DependencyAnalyzer({
		rootDir,
		...analysisConfig
	});

	await analyzer.generateReport({
		format,
		verbose,
		includeStats,
		includeCircularDeps,
		showTopUnused,
		outputFile
	});
}
