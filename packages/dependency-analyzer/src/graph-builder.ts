import { ASTParser } from './ast-parser.js';
import { EnhancedPathResolver } from './enhanced-path-resolver.js';
import { PathResolver } from './path-resolver.js';
import { SvelteParser } from './svelte-parser.js';
import { SvelteKitAnalyzer } from './sveltekit-analyzer.js';
import { glob } from 'glob';
import * as fs from 'fs';
import * as path from 'path';
import type { IPathResolver } from './path-resolver-interface.js';
import type {
	DependencyGraph,
	DependencyNode,
	FileNode,
	AnalysisConfig,
	CrossFileSymbolUsage
} from './types.js';

export class GraphBuilder {
	public pathResolver: IPathResolver;
	private astParser: ASTParser;
	private svelteParser: SvelteParser;
	private svelteKitAnalyzer: SvelteKitAnalyzer;

	constructor(private config: AnalysisConfig) {
		this.pathResolver = new EnhancedPathResolver(config);
		this.astParser = new ASTParser(config.rootDir, config.tsConfigPath, this.pathResolver);
		this.svelteParser = new SvelteParser(this.pathResolver);
		this.svelteKitAnalyzer = new SvelteKitAnalyzer(config);
	}

	async build(): Promise<DependencyGraph> {
		const graph: DependencyGraph = {
			nodes: new Map(),
			files: new Map(),
			entryPoints: new Set(),
			rootDir: this.config.rootDir,
			tsConfigPath: this.config.tsConfigPath,
			pathMappings: new Map()
		};

		// Find all files to analyze
		const files = await this.findFiles();

		// Parse all files
		console.log(`Parsing ${files.length} files...`);
		for (const filePath of files) {
			try {
				const fileNode = this.parseFile(filePath);
				graph.files.set(filePath, fileNode);
			} catch (error) {
				console.warn(`Failed to parse ${filePath}:`, error);
			}
		}

		// Build dependency relationships
		console.log('Building dependency relationships...');
		this.buildDependencyRelationships(graph);

		// Enhance graph with SvelteKit-specific analysis
		console.log('Analyzing SvelteKit patterns...');
		this.svelteKitAnalyzer.enhanceGraphForSvelteKit(graph);

		// Mark entry points
		this.markEntryPoints(graph);

		// Create dependency nodes
		this.createDependencyNodes(graph);

		return graph;
	}

	private async findFiles(): Promise<string[]> {
		const files: string[] = [];

		for (const pattern of this.config.includePatterns) {
			const matches = await glob(pattern, {
				cwd: this.config.rootDir,
				absolute: true,
				ignore: this.config.excludePatterns
			});
			files.push(...matches);
		}

		// Filter by file extensions
		const supportedExtensions = ['.ts', '.tsx', '.js', '.jsx', '.svelte'];
		return files.filter((file) => supportedExtensions.some((ext) => file.endsWith(ext)));
	}

	private parseFile(filePath: string): FileNode {
		if (filePath.endsWith('.svelte')) {
			return this.svelteParser.parseFile(filePath);
		} else {
			return this.astParser.parseFile(filePath);
		}
	}

	private buildDependencyRelationships(graph: DependencyGraph) {
		// Build file-to-file dependencies
		for (const [filePath, fileNode] of graph.files) {
			for (const importInfo of fileNode.imports) {
				const resolvedPath = this.pathResolver.resolveFileExtension(importInfo.resolvedPath);

				if (graph.files.has(resolvedPath)) {
					fileNode.dependencies.add(resolvedPath);
					const dependentFile = graph.files.get(resolvedPath)!;
					dependentFile.dependents.add(filePath);
				}
			}
		}

		// Build symbol-to-symbol dependencies
		for (const [filePath, fileNode] of graph.files) {
			for (const importInfo of fileNode.imports) {
				const resolvedPath = this.pathResolver.resolveFileExtension(importInfo.resolvedPath);
				const importedFile = graph.files.get(resolvedPath);

				if (importedFile) {
					for (const importedSymbol of importInfo.imported) {
						// Find the symbol in the imported file
						const exportedSymbol = importedFile.exports.find(
							(exp) =>
								exp.name === importedSymbol.name || (importedSymbol.isDefault && exp.isDefault)
						);

						if (exportedSymbol) {
							// Create usage record
							const localSymbol = fileNode.symbols.find(
								(sym) => sym.name === (importedSymbol.alias || importedSymbol.name)
							);

							if (localSymbol) {
								localSymbol.usages.push({
									filePath,
									line: 0, // We'll improve this later
									column: 0,
									context: 'import'
								});
							}
						}
					}
				}
			}
		}
	}

	private markEntryPoints(graph: DependencyGraph) {
		// Mark configured entry points
		for (const entryPoint of this.config.entryPoints) {
			const resolvedPath = path.resolve(this.config.rootDir, entryPoint);
			const normalizedPath = this.pathResolver.resolveFileExtension(resolvedPath);

			if (graph.files.has(normalizedPath)) {
				graph.entryPoints.add(normalizedPath);
				graph.files.get(normalizedPath)!.isEntryPoint = true;
			}
		}

		// Auto-detect entry points if none configured
		if (graph.entryPoints.size === 0) {
			this.autoDetectEntryPoints(graph);
		}
	}

	private autoDetectEntryPoints(graph: DependencyGraph) {
		const commonEntryPoints = [
			'src/main.ts',
			'src/index.ts',
			'src/app.ts',
			'src/routes/+layout.ts',
			'src/routes/+page.ts',
			'src/hooks.client.ts',
			'src/hooks.server.ts'
		];

		for (const entryPoint of commonEntryPoints) {
			const fullPath = path.resolve(this.config.rootDir, entryPoint);
			const normalizedPath = this.pathResolver.resolveFileExtension(fullPath);

			if (graph.files.has(normalizedPath)) {
				graph.entryPoints.add(normalizedPath);
				graph.files.get(normalizedPath)!.isEntryPoint = true;
			}
		}

		// If still no entry points, find files with no imports (potential entry points)
		if (graph.entryPoints.size === 0) {
			for (const [filePath, fileNode] of graph.files) {
				if (fileNode.imports.length === 0 && fileNode.exports.length > 0) {
					graph.entryPoints.add(filePath);
					fileNode.isEntryPoint = true;
				}
			}
		}
	}

	private createDependencyNodes(graph: DependencyGraph) {
		let nodeId = 0;

		for (const [filePath, fileNode] of graph.files) {
			// Create nodes for each symbol
			for (const symbol of fileNode.symbols) {
				const node: DependencyNode = {
					id: `${nodeId++}`,
					filePath,
					name: symbol.name,
					type: symbol.type,
					dependencies: new Set(),
					dependents: new Set(),
					isEntryPoint: false, // Only set to true for symbols that are actually entry points
					isUsed: false, // Will be determined by dead code analysis
					metadata: symbol.metadata
				};

				// Add export information
				const exportInfo = fileNode.exports.find((exp) => exp.name === symbol.name);
				if (exportInfo) {
					node.exportedAs = exportInfo.name;
					// For export lists, use the export declaration's metadata instead of symbol's metadata
					if (!exportInfo.isReExport) {
						node.metadata = exportInfo.metadata;
					}
				}

				// Only mark as entry point if it's in an entry point file AND meets specific criteria
				if (fileNode.isEntryPoint) {
					// Mark as entry point if it's a main function, default export, or has usages
					if (
						symbol.name === 'main' ||
						symbol.name === 'default' ||
						exportInfo?.isDefault ||
						symbol.usages.length > 0
					) {
						node.isEntryPoint = true;
					}
				}

				graph.nodes.set(node.id, node);
			}

			// Create nodes for re-exports
			for (const exportInfo of fileNode.exports) {
				if (exportInfo.isReExport) {
					const node: DependencyNode = {
						id: `${nodeId++}`,
						filePath,
						name: exportInfo.name,
						type: exportInfo.type,
						exportedAs: exportInfo.name,
						importedFrom: exportInfo.reExportFrom,
						dependencies: new Set(),
						dependents: new Set(),
						isEntryPoint: fileNode.isEntryPoint,
						isUsed: false,
						metadata: exportInfo.metadata
					};

					graph.nodes.set(node.id, node);
				}
			}
		}

		// Build node-to-node dependencies
		this.buildNodeDependencies(graph);

		// Process cross-file symbol usages
		this.processCrossFileSymbolUsages(graph);

		// Link re-exports to their original symbols
		this.linkReExportsToOriginalSymbols(graph);
	}

	private buildNodeDependencies(graph: DependencyGraph) {
		for (const [filePath, fileNode] of graph.files) {
			// Read the entire file content to check for symbol usage
			let fileContent = '';
			try {
				fileContent = fs.readFileSync(filePath, 'utf8');
			} catch (error) {
				console.warn(`Could not read file ${filePath}:`, error);
				continue;
			}

			for (const importInfo of fileNode.imports) {
				const resolvedPath = this.pathResolver.resolveFileExtension(importInfo.resolvedPath);
				const importedFile = graph.files.get(resolvedPath);

				if (importedFile) {
					for (const importedSymbol of importInfo.imported) {
						// Find the imported node
						const importedNode = Array.from(graph.nodes.values()).find(
							(node) =>
								node.filePath === resolvedPath &&
								(node.name === importedSymbol.name ||
									(importedSymbol.isDefault && node.metadata.isDefaultExport))
						);

						if (importedNode) {
							// Check if the imported symbol is used anywhere in the file
							const symbolName = importedSymbol.alias || importedSymbol.name;
							const isUsedInFile = this.checkSymbolUsage(fileContent, symbolName);

							if (isUsedInFile) {
								// Find the local nodes that could be using this symbol
								const localNodes = Array.from(graph.nodes.values()).filter(
									(node) => node.filePath === filePath
								);

								// For each local node, check if it specifically uses the imported symbol
								for (const localNode of localNodes) {
									const usesImportedSymbol = this.checkSymbolUsage(
										localNode.metadata.sourceText,
										symbolName
									);

									if (usesImportedSymbol) {
										localNode.dependencies.add(importedNode.id);
										importedNode.dependents.add(localNode.id);
									}
								}

								// If no specific local node uses it, but it's used in the file,
								// create a dependency from the most likely candidate node
								if (importedNode.dependents.size === 0) {
									// Find the first exported function/class that might be using it
									const candidateNode =
										localNodes.find(
											(node) =>
												node.metadata.isNamedExport &&
												(node.type === 'function' || node.type === 'class')
										) || localNodes[0];

									if (candidateNode) {
										candidateNode.dependencies.add(importedNode.id);
										importedNode.dependents.add(candidateNode.id);
									}
								}
							}
						}
					}
				}
			}
		}
	}

	private processCrossFileSymbolUsages(graph: DependencyGraph) {
		// Process all cross-file symbol usages recorded during AST parsing
		for (const [filePath, fileNode] of graph.files) {
			if (!fileNode.symbolUsages) continue;

			for (const crossFileUsage of fileNode.symbolUsages) {
				// Find the target file and symbol
				const targetFile = graph.files.get(crossFileUsage.importPath);
				if (!targetFile) continue;

				// Find the symbol in the target file
				const targetSymbol = targetFile.symbols.find(
					(sym) => sym.name === crossFileUsage.symbolName
				);
				if (targetSymbol) {
					// Add the usage to the original symbol
					targetSymbol.usages.push(crossFileUsage.usage);
				}
			}
		}
	}

	private checkSymbolUsage(sourceText: string, symbolName: string): boolean {
		// Simple text-based check for now
		// Could be improved with more sophisticated AST analysis
		const regex = new RegExp(`\\b${symbolName}\\b`, 'g');
		return regex.test(sourceText);
	}

	private linkReExportsToOriginalSymbols(graph: DependencyGraph) {
		// Find all re-export nodes
		const reExportNodes = Array.from(graph.nodes.values()).filter((node) => node.importedFrom);

		for (const reExportNode of reExportNodes) {
			if (!reExportNode.importedFrom) continue;

			// Resolve the file extension for the imported path
			const resolvedImportedFrom = this.pathResolver.resolveFileExtension(
				reExportNode.importedFrom
			);

			// Find the original symbol in the source file (don't match on type since re-exports might have wrong type)
			const originalSymbolNode = Array.from(graph.nodes.values()).find(
				(node) =>
					node.filePath === resolvedImportedFrom &&
					node.name === reExportNode.name &&
					!node.importedFrom // Not a re-export itself
			);

			if (originalSymbolNode) {
				// Correct the re-export type to match the original
				reExportNode.type = originalSymbolNode.type;

				// Create a dependency from the re-export to the original symbol
				reExportNode.dependencies.add(originalSymbolNode.id);
				originalSymbolNode.dependents.add(reExportNode.id);
			}
		}
	}
}
