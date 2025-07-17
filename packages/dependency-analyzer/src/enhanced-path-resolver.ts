import { parse } from 'comment-json';
import * as fs from 'fs';
import * as path from 'path';
import type { IPathResolver } from './path-resolver-interface.js';
import type { AnalysisConfig } from './types.js';

interface TsConfig {
	extends?: string;
	compilerOptions?: {
		baseUrl?: string;
		paths?: Record<string, string[]>;
	};
}

class PathEntry {
	private readonly absoluteTarget: string;

	constructor(
		configDirectory: string,
		private readonly key: string,
		target: string
	) {
		this.absoluteTarget = path.normalize(path.join(configDirectory, target));
	}

	private hasWildcard(pathStr: string): boolean {
		return pathStr.endsWith('*');
	}

	private removeGlob(pathStr: string): string {
		if (this.hasWildcard(pathStr)) {
			return pathStr.slice(0, -1);
		}
		return pathStr;
	}

	tryResolveImport(importPath: string, fromFile: string): string | undefined {
		// Handle exact matches first
		if (this.key === importPath) {
			return this.absoluteTarget;
		}

		// Handle glob patterns
		if (this.hasWildcard(this.key) && this.hasWildcard(this.absoluteTarget)) {
			const keyBase = this.removeGlob(this.key);

			if (importPath.startsWith(keyBase)) {
				const remainder = importPath.substring(keyBase.length);
				const targetBase = this.removeGlob(this.absoluteTarget);
				return path.join(targetBase, remainder);
			}
		}

		return undefined;
	}
}

class ConfigPaths {
	private readonly paths: Record<string, PathEntry[]> = {};

	pushPaths(configDirectory: string, paths: Record<string, string[]>): void {
		for (const [key, relativePaths] of Object.entries(paths)) {
			this.paths[key] = [];

			for (const target of relativePaths) {
				const entry = new PathEntry(configDirectory, key, target);
				this.paths[key]!.push(entry);
			}
		}
	}

	tryResolveImport(importPath: string, fromFile: string): string | undefined {
		for (const entries of Object.values(this.paths)) {
			for (const entry of entries) {
				const result = entry.tryResolveImport(importPath, fromFile);
				if (result) {
					return result;
				}
			}
		}
		return undefined;
	}

	get empty(): boolean {
		return Object.keys(this.paths).length === 0;
	}
}

export class EnhancedPathResolver implements IPathResolver {
	private configPaths: ConfigPaths;
	private baseUrl: string;
	private rootDir: string;

	constructor(private config: AnalysisConfig) {
		this.rootDir = config.rootDir;
		this.baseUrl = this.rootDir;
		this.configPaths = new ConfigPaths();
		this.loadTsConfigChain();
	}

	private loadTsConfigChain(): void {
		const configs = this.findConfigs(this.config.tsConfigPath);

		for (const [configDirectory, tsConfig] of configs) {
			// Set baseUrl from the most specific config
			if (tsConfig.compilerOptions?.baseUrl) {
				this.baseUrl = path.resolve(configDirectory, tsConfig.compilerOptions.baseUrl);
			}

			// Accumulate paths from all configs (more specific overrides less specific)
			if (tsConfig.compilerOptions?.paths) {
				this.configPaths.pushPaths(configDirectory, tsConfig.compilerOptions.paths);
			}
		}
	}

	private findConfigs(startPath: string): [string, TsConfig][] {
		const configs: [string, TsConfig][] = [];
		let currentPath = startPath;

		while (true) {
			const configDirectory = path.dirname(currentPath);
			const configPath = currentPath;

			if (!fs.existsSync(configPath)) {
				break;
			}

			try {
				const configContent = fs.readFileSync(configPath, 'utf-8');
				const config = parse(configContent) as TsConfig;

				// Add to beginning to maintain order (base -> extended)
				configs.unshift([configDirectory, config]);

				if (config.extends) {
					let extendsPath = config.extends;
					if (!extendsPath.endsWith('.json')) {
						extendsPath = `${extendsPath}.json`;
					}
					currentPath = path.resolve(configDirectory, extendsPath);
				} else {
					break;
				}
			} catch (error) {
				console.warn(`Failed to parse tsconfig at ${configPath}:`, error);
				break;
			}
		}

		return configs;
	}

	resolveImportPath(importPath: string, fromFile: string): string {
		// Handle relative imports
		if (importPath.startsWith('./') || importPath.startsWith('../')) {
			return path.resolve(path.dirname(fromFile), importPath);
		}

		// Try path mappings first
		const mappedPath = this.configPaths.tryResolveImport(importPath, fromFile);
		if (mappedPath) {
			return this.resolveFileExtension(mappedPath);
		}

		// Handle node_modules (external dependencies)
		if (!importPath.startsWith('/') && !this.isPathMappingPattern(importPath)) {
			return importPath; // External dependency
		}

		// Fallback to baseUrl resolution
		const resolved = path.resolve(this.baseUrl, importPath);
		return this.resolveFileExtension(resolved);
	}

	private isPathMappingPattern(importPath: string): boolean {
		// Check if import matches any path mapping pattern
		// Try to resolve the import path - if it resolves, it's a path mapping
		const resolved = this.configPaths.tryResolveImport(importPath, '');
		return resolved !== undefined;
	}

	resolveFileExtension(filePath: string): string {
		const extensions = ['.ts', '.svelte', '.tsx', '.js', '.jsx', '.d.ts'];

		// Special handling for .svelte files that might be .svelte.ts
		if (filePath.endsWith('.svelte')) {
			const svelteTs = filePath + '.ts';
			if (fs.existsSync(svelteTs)) {
				return svelteTs;
			}
			// If .svelte.ts doesn't exist, check if .svelte exists
			if (fs.existsSync(filePath)) {
				return filePath;
			}
		}

		// If already has extension and exists, return as-is
		if (extensions.some((ext) => filePath.endsWith(ext))) {
			if (fs.existsSync(filePath)) {
				return filePath;
			}
		}

		// Try adding extensions (prioritize TypeScript and Svelte)
		for (const ext of extensions) {
			const withExt = filePath + ext;
			if (fs.existsSync(withExt)) {
				return withExt;
			}
		}

		// Try index files
		for (const ext of extensions) {
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
		// Handle undefined/null import paths
		if (!importPath || typeof importPath !== 'string') {
			return true;
		}

		// Check if it's a node_modules import
		return (
			!importPath.startsWith('.') &&
			!importPath.startsWith('/') &&
			!this.isPathMappingPattern(importPath)
		);
	}

	normalizePath(filePath: string): string {
		return path.resolve(filePath);
	}

	getRelativePathFromRoot(filePath: string): string {
		return path.relative(this.rootDir, filePath);
	}
}
