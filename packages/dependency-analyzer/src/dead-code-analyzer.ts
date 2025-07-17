import type { IPathResolver } from './path-resolver-interface.js';
import type {
	DependencyGraph,
	DependencyNode,
	FileNode,
	AnalysisResult,
	AnalysisStats
} from './types.js';

export class DeadCodeAnalyzer {
	constructor(private pathResolver: IPathResolver) {}

	analyze(graph: DependencyGraph): AnalysisResult {
		const startTime = Date.now();

		// Mark all nodes as unused initially
		this.markAllNodesAsUnused(graph);

		// Perform mark-and-sweep from entry points
		this.markUsedNodes(graph);

		// Find unused exports and files
		const unusedExports = this.findUnusedExports(graph);
		const unusedFiles = this.findUnusedFiles(graph);

		// Detect circular dependencies
		const circularDependencies = this.detectCircularDependencies(graph);

		// Generate statistics
		const stats = this.generateStats(
			graph,
			unusedExports,
			unusedFiles,
			circularDependencies,
			startTime
		);

		return {
			graph,
			unusedExports,
			unusedFiles,
			circularDependencies,
			stats
		};
	}

	private markAllNodesAsUnused(graph: DependencyGraph) {
		for (const node of graph.nodes.values()) {
			node.isUsed = false;
		}
	}

	private markUsedNodes(graph: DependencyGraph) {
		const visited = new Set<string>();
		const stack: string[] = [];

		// First, mark symbols with usages as used (including framework-consumed)
		this.markSymbolsWithUsagesAsUsed(graph, stack);

		// Start from entry points - only add exported nodes that are already marked as used
		// or are likely to be true entry points (main functions, default exports, etc.)
		for (const entryPointPath of graph.entryPoints) {
			const entryNodes = Array.from(graph.nodes.values()).filter(
				(node) =>
					node.filePath === entryPointPath &&
					node.exportedAs &&
					(node.isUsed || // Already marked as used (has actual usages)
						node.name === 'main' || // Main function
						node.name === 'default' || // Default export
						node.exportedAs === 'default') // Default export
			);

			for (const node of entryNodes) {
				if (!visited.has(node.id)) {
					stack.push(node.id);
				}
			}
		}

		// Mark nodes as used via depth-first traversal
		while (stack.length > 0) {
			const nodeId = stack.pop()!;

			if (visited.has(nodeId)) continue;
			visited.add(nodeId);

			const node = graph.nodes.get(nodeId);
			if (!node) continue;

			// Mark this node as used
			node.isUsed = true;

			// Add dependencies to stack
			for (const depId of node.dependencies) {
				if (!visited.has(depId)) {
					stack.push(depId);
				}
			}
		}

		// Special handling for type-only imports
		this.markTypeOnlyDependencies(graph);
	}

	private markSymbolsWithUsagesAsUsed(graph: DependencyGraph, stack: string[]) {
		// Check all files for symbols that have recorded usages
		for (const [filePath, fileNode] of graph.files) {
			for (const symbol of fileNode.symbols) {
				if (symbol.usages.length > 0) {
					// Find corresponding dependency node and mark as used
					const node = Array.from(graph.nodes.values()).find(
						(n) => n.filePath === filePath && n.name === symbol.name
					);
					if (node && !node.isUsed) {
						node.isUsed = true;
						stack.push(node.id);
					}
				}
			}

			// Also check cross-file symbol usages
			if (fileNode.symbolUsages) {
				for (const crossFileUsage of fileNode.symbolUsages) {
					// Resolve the file extension for the import path
					const resolvedImportPath = this.pathResolver.resolveFileExtension(
						crossFileUsage.importPath
					);

					// Find the imported symbol node in the target file
					const importedNode = Array.from(graph.nodes.values()).find(
						(n) => n.filePath === resolvedImportPath && n.name === crossFileUsage.symbolName
					);

					if (importedNode && !importedNode.isUsed) {
						importedNode.isUsed = true;
						stack.push(importedNode.id);
					}
				}
			}
		}
	}

	private markTypeOnlyDependencies(graph: DependencyGraph) {
		// In TypeScript, type-only imports should be marked as used
		// if they're referenced in type annotations
		for (const [filePath, fileNode] of graph.files) {
			for (const importInfo of fileNode.imports) {
				if (importInfo.isTypeOnly) {
					const resolvedPath = importInfo.resolvedPath;
					const importedFile = graph.files.get(resolvedPath);

					if (importedFile) {
						for (const importedSymbol of importInfo.imported) {
							const importedNode = Array.from(graph.nodes.values()).find(
								(node) => node.filePath === resolvedPath && node.name === importedSymbol.name
							);

							if (importedNode) {
								importedNode.isUsed = true;
							}
						}
					}
				}
			}
		}
	}

	private findUnusedExports(graph: DependencyGraph): DependencyNode[] {
		const unusedExports: DependencyNode[] = [];

		for (const node of graph.nodes.values()) {
			// Skip if node is used
			if (node.isUsed) continue;

			// Skip if node is not exported
			if (!node.exportedAs) continue;

			// Skip if this is an entry point
			if (node.isEntryPoint) continue;

			// Skip if this is a re-export (might be used externally)
			if (node.importedFrom) continue;

			// Skip if this export is used locally within the same file
			if (this.isUsedLocally(node, graph)) continue;

			unusedExports.push(node);
		}

		return unusedExports;
	}

	private isUsedLocally(node: DependencyNode, graph: DependencyGraph): boolean {
		// Get the file that contains this node
		const fileNode = graph.files.get(node.filePath);
		if (!fileNode) return false;

		// Check if the symbol has any usages recorded
		const symbol = fileNode.symbols.find(sym => sym.name === node.name);
		if (!symbol) return false;

		// Check if any of the usages are in the same file
		return symbol.usages.some(usage => usage.filePath === node.filePath);
	}

	private findUnusedFiles(graph: DependencyGraph): FileNode[] {
		const unusedFiles: FileNode[] = [];

		for (const [filePath, fileNode] of graph.files) {
			// Skip entry points
			if (fileNode.isEntryPoint) continue;

			// Check if any node in this file is used
			const fileNodes = Array.from(graph.nodes.values()).filter(
				(node) => node.filePath === filePath
			);

			const hasUsedNodes = fileNodes.some((node) => node.isUsed);

			// If no nodes are used and file has no dependents, it's unused
			if (!hasUsedNodes && fileNode.dependents.size === 0) {
				unusedFiles.push(fileNode);
			}
		}

		return unusedFiles;
	}

	private detectCircularDependencies(graph: DependencyGraph): string[][] {
		const visited = new Set<string>();
		const recursionStack = new Set<string>();
		const cycles: string[][] = [];

		const dfs = (nodeId: string, path: string[]): void => {
			if (recursionStack.has(nodeId)) {
				// Found a cycle
				const cycleStart = path.indexOf(nodeId);
				const cycle = path.slice(cycleStart);
				cycles.push([...cycle, nodeId]);
				return;
			}

			if (visited.has(nodeId)) return;

			visited.add(nodeId);
			recursionStack.add(nodeId);
			path.push(nodeId);

			const node = graph.nodes.get(nodeId);
			if (node) {
				for (const depId of node.dependencies) {
					dfs(depId, path);
				}
			}

			recursionStack.delete(nodeId);
			path.pop();
		};

		// Check each node
		for (const nodeId of graph.nodes.keys()) {
			if (!visited.has(nodeId)) {
				dfs(nodeId, []);
			}
		}

		return cycles;
	}

	private generateStats(
		graph: DependencyGraph,
		unusedExports: DependencyNode[],
		unusedFiles: FileNode[],
		circularDependencies: string[][],
		startTime: number
	): AnalysisStats {
		const totalDependencies = Array.from(graph.nodes.values()).reduce(
			(sum, node) => sum + node.dependencies.size,
			0
		);

		return {
			totalFiles: graph.files.size,
			totalSymbols: graph.nodes.size,
			totalDependencies,
			unusedExports: unusedExports.length,
			unusedFiles: unusedFiles.length,
			circularDependencies: circularDependencies.length,
			analysisTime: Date.now() - startTime
		};
	}

	// Additional utility methods for more sophisticated analysis

	findDeadCodeHotspots(graph: DependencyGraph): { filePath: string; unusedCount: number }[] {
		const hotspots = new Map<string, number>();

		for (const node of graph.nodes.values()) {
			if (!node.isUsed && node.exportedAs) {
				const count = hotspots.get(node.filePath) || 0;
				hotspots.set(node.filePath, count + 1);
			}
		}

		return Array.from(hotspots.entries())
			.map(([filePath, unusedCount]) => ({ filePath, unusedCount }))
			.sort((a, b) => b.unusedCount - a.unusedCount);
	}

	findOrphanedFiles(graph: DependencyGraph): FileNode[] {
		const orphanedFiles: FileNode[] = [];

		for (const [filePath, fileNode] of graph.files) {
			// Skip entry points
			if (fileNode.isEntryPoint) continue;

			// If no other files depend on this file, it's orphaned
			if (fileNode.dependents.size === 0) {
				orphanedFiles.push(fileNode);
			}
		}

		return orphanedFiles;
	}

	findLargestUnusedExports(graph: DependencyGraph, limit: number = 10): DependencyNode[] {
		const unusedExports = this.findUnusedExports(graph);

		return unusedExports
			.sort((a, b) => b.metadata.sourceText.length - a.metadata.sourceText.length)
			.slice(0, limit);
	}

	calculateCodeSavings(
		unusedExports: DependencyNode[],
		unusedFiles: FileNode[]
	): {
		linesOfCode: number;
		charactersOfCode: number;
		filesCount: number;
	} {
		let linesOfCode = 0;
		let charactersOfCode = 0;

		// Count from unused exports
		for (const node of unusedExports) {
			const lines = node.metadata.endLine - node.metadata.startLine + 1;
			linesOfCode += lines;
			charactersOfCode += node.metadata.sourceText.length;
		}

		// Count from unused files
		for (const file of unusedFiles) {
			// Rough estimate - could be improved by reading file contents
			linesOfCode += 50; // Average lines per file
			charactersOfCode += 2000; // Average characters per file
		}

		return {
			linesOfCode,
			charactersOfCode,
			filesCount: unusedFiles.length
		};
	}
}
