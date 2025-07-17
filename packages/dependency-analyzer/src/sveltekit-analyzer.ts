import { PathResolver } from './path-resolver.js';
import * as fs from 'fs';
import * as path from 'path';
import type {
	DependencyGraph,
	FileNode,
	AnalysisConfig,
	ImportInfo,
	ExportInfo,
	SymbolInfo
} from './types.js';

export class SvelteKitAnalyzer {
	private pathResolver: PathResolver;

	constructor(private config: AnalysisConfig) {
		this.pathResolver = new PathResolver(config);
	}

	enhanceGraphForSvelteKit(graph: DependencyGraph): void {
		// Detect SvelteKit-specific entry points
		this.detectSvelteKitEntryPoints(graph);

		// Mark SvelteKit framework exports as used
		this.markFrameworkExportsAsUsed(graph);

		// Enhance Svelte component analysis
		this.enhanceSvelteComponents(graph);

		// Track context and store dependencies
		this.trackContextDependencies(graph);

		// Analyze reactive dependencies
		this.analyzeReactiveDependencies(graph);

		// Handle SvelteKit-specific patterns
		this.handleSvelteKitPatterns(graph);
	}

	private detectSvelteKitEntryPoints(graph: DependencyGraph): void {
		let entryPointCount = 0;

		for (const [filePath, fileNode] of graph.files) {
			const relativePath = path.relative(this.config.rootDir, filePath);

			// SvelteKit specific detection - simpler and more reliable
			const isSvelteKitEntryPoint = this.isSvelteKitEntryPoint(relativePath);

			if (isSvelteKitEntryPoint && !fileNode.isEntryPoint) {
				fileNode.isEntryPoint = true;
				graph.entryPoints.add(filePath);
				entryPointCount++;
				console.log(`🚀 Detected SvelteKit entry point: ${relativePath}`);
			}

			// Also check for any file that exports a load function or actions
			if (
				(relativePath.includes('+page.ts') || relativePath.includes('+layout.ts')) &&
				!fileNode.isEntryPoint
			) {
				if (this.hasLoadFunction(filePath) || this.hasActions(filePath)) {
					fileNode.isEntryPoint = true;
					graph.entryPoints.add(filePath);
					entryPointCount++;
					console.log(`🚀 Detected SvelteKit load/action entry point: ${relativePath}`);
				}
			}
		}

		console.log(`📍 Total entry points detected: ${entryPointCount}`);
	}

	private hasLoadFunction(filePath: string): boolean {
		try {
			const content = fs.readFileSync(filePath, 'utf-8');
			return /export\s+(?:async\s+)?function\s+load\s*\(/g.test(content);
		} catch {
			return false;
		}
	}

	private hasActions(filePath: string): boolean {
		try {
			const content = fs.readFileSync(filePath, 'utf-8');
			return /export\s+const\s+actions\s*=/g.test(content);
		} catch {
			return false;
		}
	}

	private markFrameworkExportsAsUsed(graph: DependencyGraph): void {
		// SvelteKit special exports that are consumed by the framework
		const svelteKitSpecialExports = new Set([
			'ssr',
			'prerender',
			'csr',
			'trailingSlash',
			'load',
			'actions',
			'entries',
			'fallback',
			'config',
			'handle',
			'handleError',
			'handleFetch'
		]);

		for (const [filePath, fileNode] of graph.files) {
			const relativePath = path.relative(this.config.rootDir, filePath);

			// Check if this is a SvelteKit special file (routes, hooks, etc.)
			const isSvelteKitFile =
				relativePath.includes('src/routes/') ||
				relativePath.includes('src/hooks.') ||
				relativePath.includes('src/app.') ||
				relativePath.includes('src/service-worker');

			if (isSvelteKitFile) {
				// Mark framework exports as used by creating artificial usage
				for (const exportInfo of fileNode.exports) {
					if (svelteKitSpecialExports.has(exportInfo.name)) {
						// Mark as framework-consumed by adding to entry points
						fileNode.isEntryPoint = true;
						graph.entryPoints.add(filePath);

						// Find corresponding symbol and mark as used
						const symbol = fileNode.symbols.find((sym) => sym.name === exportInfo.name);
						if (symbol) {
							symbol.usages.push({
								filePath: 'FRAMEWORK_CONSUMED',
								line: 0,
								column: 0,
								context: 'sveltekit-framework'
							});
						}
					}
				}
			}
		}
	}

	private enhanceSvelteComponents(graph: DependencyGraph): void {
		for (const [filePath, fileNode] of graph.files) {
			if (filePath.endsWith('.svelte')) {
				this.analyzeSvelteComponent(filePath, fileNode, graph);
			}
		}
	}

	private analyzeSvelteComponent(
		filePath: string,
		fileNode: FileNode,
		graph: DependencyGraph
	): void {
		try {
			const content = fs.readFileSync(filePath, 'utf-8');

			// Parse component usage in template
			this.parseComponentUsage(content, fileNode, graph);

			// Parse store usage
			this.parseStoreUsage(content, fileNode, graph);

			// Parse context usage
			this.parseContextUsage(content, fileNode, graph);

			// Parse event handlers
			this.parseEventHandlers(content, fileNode, graph);

			// Parse reactive statements
			this.parseReactiveStatements(content, fileNode, graph);
		} catch (error) {
			console.warn(`Failed to analyze Svelte component ${filePath}:`, error);
		}
	}

	private parseComponentUsage(content: string, fileNode: FileNode, graph: DependencyGraph): void {
		// Find all component usages in template
		const componentRegex = /<(\w+)(?:\s[^>]*)?(?:>[\s\S]*?<\/\1>|\/?>)/g;
		let match;

		while ((match = componentRegex.exec(content)) !== null) {
			const componentName = match[1];

			// Skip HTML elements
			if (this.isHtmlElement(componentName)) continue;

			// Find corresponding import
			const importInfo = fileNode.imports.find((imp) =>
				imp.imported.some((sym) => sym.name === componentName)
			);

			if (importInfo) {
				// Create stronger dependency link
				const resolvedPath = this.pathResolver.resolveFileExtension(importInfo.resolvedPath);
				if (graph.files.has(resolvedPath)) {
					fileNode.dependencies.add(resolvedPath);
					graph.files.get(resolvedPath)!.dependents.add(fileNode.filePath);
				}
			}
		}
	}

	private parseStoreUsage(content: string, fileNode: FileNode, graph: DependencyGraph): void {
		// Find store subscriptions: $storeName
		const storeRegex = /\$(\w+)/g;
		let match;

		while ((match = storeRegex.exec(content)) !== null) {
			const storeName = match[1];

			// Find corresponding import
			const importInfo = fileNode.imports.find((imp) =>
				imp.imported.some((sym) => sym.name === storeName)
			);

			if (importInfo) {
				const resolvedPath = this.pathResolver.resolveFileExtension(importInfo.resolvedPath);
				if (graph.files.has(resolvedPath)) {
					fileNode.dependencies.add(resolvedPath);
					graph.files.get(resolvedPath)!.dependents.add(fileNode.filePath);
				}
			}
		}
	}

	private parseContextUsage(content: string, fileNode: FileNode, graph: DependencyGraph): void {
		// Find getContext calls
		const getContextRegex = /getContext\s*\(\s*['"`]([^'"`]+)['"`]\s*\)/g;
		let match;

		while ((match = getContextRegex.exec(content)) !== null) {
			const contextKey = match[1];

			// Find setContext calls in other files
			for (const [otherFilePath, otherFileNode] of graph.files) {
				if (otherFilePath === fileNode.filePath) continue;

				try {
					const otherContent = fs.readFileSync(otherFilePath, 'utf-8');
					const setContextRegex = new RegExp(`setContext\\s*\\(\\s*['"\`]${contextKey}['"\`]`, 'g');

					if (setContextRegex.test(otherContent)) {
						// Create dependency from context consumer to provider
						fileNode.dependencies.add(otherFilePath);
						otherFileNode.dependents.add(fileNode.filePath);
					}
				} catch (error) {
					// Skip files that can't be read
				}
			}
		}
	}

	private parseEventHandlers(content: string, fileNode: FileNode, graph: DependencyGraph): void {
		// Find event handler references: on:click={functionName}
		const eventRegex = /on:\w+\s*=\s*\{([^}]+)\}/g;
		let match;

		while ((match = eventRegex.exec(content)) !== null) {
			const handlerCode = match[1];

			// Extract function names from handler code
			const functionRegex = /\b(\w+)\s*\(/g;
			let funcMatch;

			while ((funcMatch = functionRegex.exec(handlerCode)) !== null) {
				const functionName = funcMatch[1];

				// Find corresponding import or local symbol
				const importInfo = fileNode.imports.find((imp) =>
					imp.imported.some((sym) => sym.name === functionName)
				);

				if (importInfo) {
					const resolvedPath = this.pathResolver.resolveFileExtension(importInfo.resolvedPath);
					if (graph.files.has(resolvedPath)) {
						fileNode.dependencies.add(resolvedPath);
						graph.files.get(resolvedPath)!.dependents.add(fileNode.filePath);
					}
				}
			}
		}
	}

	private parseReactiveStatements(
		content: string,
		fileNode: FileNode,
		graph: DependencyGraph
	): void {
		// Find reactive statements: $: variable = ...
		const reactiveRegex = /\$:\s*(\w+)\s*=/g;
		let match;

		while ((match = reactiveRegex.exec(content)) !== null) {
			const variableName = match[1];

			// This creates internal dependencies within the component
			// For now, we'll just track that reactive statements exist
		}
	}

	private trackContextDependencies(graph: DependencyGraph): void {
		// Create a map of context keys to their providers
		const contextProviders = new Map<string, Set<string>>();
		const contextConsumers = new Map<string, Set<string>>();

		// First pass: find all context providers and consumers
		for (const [filePath, fileNode] of graph.files) {
			try {
				const content = fs.readFileSync(filePath, 'utf-8');

				// Find setContext calls
				const setContextRegex = /setContext\s*\(\s*['"`]([^'"`]+)['"`]/g;
				let match;

				while ((match = setContextRegex.exec(content)) !== null) {
					const contextKey = match[1];
					if (!contextProviders.has(contextKey)) {
						contextProviders.set(contextKey, new Set());
					}
					contextProviders.get(contextKey)!.add(filePath);
				}

				// Find getContext calls
				const getContextRegex = /getContext\s*\(\s*['"`]([^'"`]+)['"`]/g;
				while ((match = getContextRegex.exec(content)) !== null) {
					const contextKey = match[1];
					if (!contextConsumers.has(contextKey)) {
						contextConsumers.set(contextKey, new Set());
					}
					contextConsumers.get(contextKey)!.add(filePath);
				}
			} catch (error) {
				// Skip files that can't be read
			}
		}

		// Second pass: create dependencies
		for (const [contextKey, providers] of contextProviders) {
			const consumers = contextConsumers.get(contextKey);
			if (consumers) {
				for (const providerPath of providers) {
					for (const consumerPath of consumers) {
						if (providerPath !== consumerPath) {
							const consumerNode = graph.files.get(consumerPath);
							const providerNode = graph.files.get(providerPath);

							if (consumerNode && providerNode) {
								consumerNode.dependencies.add(providerPath);
								providerNode.dependents.add(consumerPath);
							}
						}
					}
				}
			}
		}
	}

	private analyzeReactiveDependencies(graph: DependencyGraph): void {
		// Analyze reactive dependencies in Svelte components
		for (const [filePath, fileNode] of graph.files) {
			if (filePath.endsWith('.svelte')) {
				try {
					const content = fs.readFileSync(filePath, 'utf-8');

					// Find reactive statements and their dependencies
					const reactiveRegex = /\$:\s*([^=]+)\s*=\s*([^;]+)/g;
					let match;

					while ((match = reactiveRegex.exec(content)) !== null) {
						const leftSide = match[1];
						const rightSide = match[2];

						// Extract variable references from right side
						const variableRegex = /\b(\w+)\b/g;
						let varMatch;

						while ((varMatch = variableRegex.exec(rightSide)) !== null) {
							const variableName = varMatch[1];

							// Check if this variable is imported
							const importInfo = fileNode.imports.find((imp) =>
								imp.imported.some((sym) => sym.name === variableName)
							);

							if (importInfo) {
								const resolvedPath = this.pathResolver.resolveFileExtension(
									importInfo.resolvedPath
								);
								if (graph.files.has(resolvedPath)) {
									fileNode.dependencies.add(resolvedPath);
									graph.files.get(resolvedPath)!.dependents.add(filePath);
								}
							}
						}
					}
				} catch (error) {
					// Skip files that can't be read
				}
			}
		}
	}

	private handleSvelteKitPatterns(graph: DependencyGraph): void {
		// Handle SvelteKit-specific patterns
		for (const [filePath, fileNode] of graph.files) {
			const relativePath = path.relative(this.config.rootDir, filePath);

			// Handle load functions
			if (relativePath.includes('+page.ts') || relativePath.includes('+layout.ts')) {
				this.analyzeLoadFunction(filePath, fileNode, graph);
			}

			// Handle actions
			if (relativePath.includes('+page.server.ts') || relativePath.includes('+layout.server.ts')) {
				this.analyzeActions(filePath, fileNode, graph);
			}

			// Handle route grouping
			if (relativePath.includes('routes/')) {
				this.analyzeRouteStructure(filePath, fileNode, graph);
			}
		}
	}

	private analyzeLoadFunction(filePath: string, fileNode: FileNode, graph: DependencyGraph): void {
		try {
			const content = fs.readFileSync(filePath, 'utf-8');

			// Find load function exports
			const loadRegex = /export\s+(?:async\s+)?function\s+load\s*\(/g;

			if (loadRegex.test(content)) {
				// Load functions are always used by SvelteKit
				fileNode.isEntryPoint = true;
				graph.entryPoints.add(filePath);
			}
		} catch (error) {
			// Skip files that can't be read
		}
	}

	private analyzeActions(filePath: string, fileNode: FileNode, graph: DependencyGraph): void {
		try {
			const content = fs.readFileSync(filePath, 'utf-8');

			// Find action exports
			const actionsRegex = /export\s+const\s+actions\s*=/g;

			if (actionsRegex.test(content)) {
				// Actions are always used by SvelteKit
				fileNode.isEntryPoint = true;
				graph.entryPoints.add(filePath);
			}
		} catch (error) {
			// Skip files that can't be read
		}
	}

	private analyzeRouteStructure(
		filePath: string,
		fileNode: FileNode,
		graph: DependencyGraph
	): void {
		// Analyze route structure and create dependencies between related route files
		const routeDir = path.dirname(filePath);

		// Find related route files in the same directory
		const routeFiles = [
			path.join(routeDir, '+page.svelte'),
			path.join(routeDir, '+page.ts'),
			path.join(routeDir, '+page.server.ts'),
			path.join(routeDir, '+layout.svelte'),
			path.join(routeDir, '+layout.ts'),
			path.join(routeDir, '+layout.server.ts')
		];

		for (const routeFile of routeFiles) {
			if (routeFile !== filePath && graph.files.has(routeFile)) {
				// Create loose coupling between route files
				fileNode.dependencies.add(routeFile);
				graph.files.get(routeFile)!.dependents.add(filePath);
			}
		}
	}

	private isHtmlElement(tagName: string): boolean {
		const htmlElements = new Set([
			'div',
			'span',
			'a',
			'p',
			'h1',
			'h2',
			'h3',
			'h4',
			'h5',
			'h6',
			'ul',
			'ol',
			'li',
			'img',
			'button',
			'input',
			'form',
			'label',
			'select',
			'option',
			'textarea',
			'table',
			'tr',
			'td',
			'th',
			'nav',
			'main',
			'section',
			'article',
			'aside',
			'header',
			'footer',
			'svg',
			'path',
			'circle',
			'rect',
			'g',
			'text',
			'line',
			'polygon'
		]);
		return htmlElements.has(tagName.toLowerCase());
	}

	private isSvelteKitEntryPoint(relativePath: string): boolean {
		// Check if it's in the routes directory
		if (!relativePath.startsWith('src/routes/')) {
			// Check for special SvelteKit files
			return (
				relativePath === 'src/hooks.client.ts' ||
				relativePath === 'src/hooks.server.ts' ||
				relativePath === 'src/app.html' ||
				relativePath === 'src/service-worker.ts' ||
				relativePath === 'src/service-worker.js'
			);
		}

		// All SvelteKit route files are entry points
		return (
			relativePath.includes('+page.svelte') ||
			relativePath.includes('+page.ts') ||
			relativePath.includes('+page.server.ts') ||
			relativePath.includes('+layout.svelte') ||
			relativePath.includes('+layout.ts') ||
			relativePath.includes('+layout.server.ts') ||
			relativePath.includes('+error.svelte')
		);
	}

	private matchesGlob(filePath: string, pattern: string): boolean {
		// Convert glob pattern to regex, handling special characters
		let regexPattern = pattern
			// Escape regex special characters first (except * and ?)
			.replace(/[.+^${}()|[\]\\]/g, '\\$&');

		// Handle ** first (must come before single *)
		// ** matches zero or more path segments (including none)
		regexPattern = regexPattern.replace(/\*\*/g, '(?:.*?)');

		// Handle single *
		regexPattern = regexPattern.replace(/\*/g, '[^/]*');

		// Handle ?
		regexPattern = regexPattern.replace(/\?/g, '.');

		// For patterns ending with /** or /**/file, make the path separator optional
		regexPattern = regexPattern.replace(/\(\?\:.*?\)\//g, '(?:.*?/|)');

		const regex = new RegExp(`^${regexPattern}$`);
		return regex.test(filePath);
	}
}
