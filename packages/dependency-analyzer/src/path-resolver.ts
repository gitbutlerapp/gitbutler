import * as fs from 'fs';
import * as path from 'path';
import type { IPathResolver } from './path-resolver-interface.js';
import type { AnalysisConfig } from './types.js';

export class PathResolver implements IPathResolver {
	private pathMappings: Map<string, string[]> = new Map();
	private baseUrl: string;
	private rootDir: string;

	constructor(private config: AnalysisConfig) {
		this.rootDir = config.rootDir;
		this.baseUrl = this.rootDir;
		this.loadTsConfig();
	}

	private loadTsConfig() {
		try {
			const tsConfigPath = path.resolve(this.config.tsConfigPath);
			const tsConfig = JSON.parse(fs.readFileSync(tsConfigPath, 'utf-8'));

			// Handle extends
			if (tsConfig.extends) {
				const extendedPath = path.resolve(path.dirname(tsConfigPath), tsConfig.extends);
				const extendedConfig = JSON.parse(fs.readFileSync(extendedPath, 'utf-8'));
				// Merge configs (extended first, then override)
				const mergedConfig = { ...extendedConfig, ...tsConfig };
				tsConfig.compilerOptions = {
					...extendedConfig.compilerOptions,
					...tsConfig.compilerOptions
				};
			}

			if (tsConfig.compilerOptions?.baseUrl) {
				this.baseUrl = path.resolve(path.dirname(tsConfigPath), tsConfig.compilerOptions.baseUrl);
			}

			if (tsConfig.compilerOptions?.paths) {
				for (const [pattern, paths] of Object.entries(tsConfig.compilerOptions.paths)) {
					this.pathMappings.set(pattern, paths as string[]);
				}
			}
		} catch (error) {
			console.warn(`Failed to load tsconfig from ${this.config.tsConfigPath}:`, error);
		}
	}

	resolveImportPath(importPath: string, fromFile: string): string {
		// Handle relative imports
		if (importPath.startsWith('./') || importPath.startsWith('../')) {
			return path.resolve(path.dirname(fromFile), importPath);
		}

		// Handle absolute imports with path mappings
		for (const [pattern, mappings] of this.pathMappings) {
			if (this.matchesPattern(importPath, pattern)) {
				const resolvedPath = this.resolveWithMapping(importPath, pattern, mappings);
				if (resolvedPath) return resolvedPath;
			}
		}

		// Handle node_modules (external dependencies)
		if (!importPath.startsWith('/')) {
			// This is likely a node_modules import, return as-is for now
			return importPath;
		}

		// Fallback to baseUrl resolution
		return path.resolve(this.baseUrl, importPath);
	}

	private matchesPattern(importPath: string, pattern: string): boolean {
		// Convert tsconfig glob pattern to regex
		const regexPattern = pattern.replace(/\*/g, '([^/]*)').replace(/\//g, '\\/');

		const regex = new RegExp(`^${regexPattern}$`);
		return regex.test(importPath);
	}

	private resolveWithMapping(
		importPath: string,
		pattern: string,
		mappings: string[]
	): string | null {
		const match = importPath.match(new RegExp(`^${pattern.replace(/\*/g, '([^/]*)')}$`));
		if (!match) return null;

		for (const mapping of mappings) {
			let resolvedMapping = mapping;

			// Replace * with captured groups
			for (let i = 1; i < match.length; i++) {
				resolvedMapping = resolvedMapping.replace('*', match[i]);
			}

			const fullPath = path.resolve(this.baseUrl, resolvedMapping);

			// Try different extensions
			const extensions = ['.ts', '.js', '.svelte', '.tsx', '.jsx'];
			for (const ext of extensions) {
				const withExt = fullPath + ext;
				if (fs.existsSync(withExt)) {
					return withExt;
				}
			}

			// Try index files
			for (const ext of extensions) {
				const indexPath = path.join(fullPath, `index${ext}`);
				if (fs.existsSync(indexPath)) {
					return indexPath;
				}
			}

			// If directory exists, assume it's valid
			if (fs.existsSync(fullPath) && fs.statSync(fullPath).isDirectory()) {
				return fullPath;
			}
		}

		return null;
	}

	resolveFileExtension(filePath: string): string {
		const extensions = ['.ts', '.js', '.svelte', '.tsx', '.jsx', '.d.ts'];

		// If already has extension, return as-is
		if (extensions.some((ext) => filePath.endsWith(ext))) {
			return filePath;
		}

		// Try adding extensions (prioritize TypeScript and Svelte)
		for (const ext of ['.ts', '.svelte', '.tsx', '.js', '.jsx', '.d.ts']) {
			const withExt = filePath + ext;
			if (fs.existsSync(withExt)) {
				return withExt;
			}
		}

		// Try index files (prioritize TypeScript and Svelte)
		for (const ext of ['.ts', '.svelte', '.tsx', '.js', '.jsx', '.d.ts']) {
			const indexPath = path.join(filePath, `index${ext}`);
			if (fs.existsSync(indexPath)) {
				return indexPath;
			}
		}

		// Handle SvelteKit route files
		const svelteKitExtensions = ['+page.svelte', '+page.ts', '+layout.svelte', '+layout.ts'];
		for (const ext of svelteKitExtensions) {
			const routePath = path.join(filePath, ext);
			if (fs.existsSync(routePath)) {
				return routePath;
			}
		}

		return filePath;
	}

	isExternal(importPath: string): boolean {
		// Check if it's a node_modules import
		return (
			!importPath.startsWith('.') &&
			!importPath.startsWith('/') &&
			!this.isPathMappingImport(importPath)
		);
	}

	private isPathMappingImport(importPath: string): boolean {
		for (const pattern of this.pathMappings.keys()) {
			if (this.matchesPattern(importPath, pattern)) {
				return true;
			}
		}
		return false;
	}

	normalizePath(filePath: string): string {
		return path.resolve(filePath);
	}

	getRelativePathFromRoot(filePath: string): string {
		return path.relative(this.rootDir, filePath);
	}
}
