#!/usr/bin/env node

import { analyzeProject } from './index.js';
import { Command } from 'commander';
import * as path from 'path';

const program = new Command();

program
	.name('dependency-analyzer')
	.description('Analyze TypeScript dependencies and find dead code')
	.version('0.1.0');

program
	.command('analyze')
	.description('Analyze a TypeScript project for dead code')
	.argument('[directory]', 'Project directory to analyze', process.cwd())
	.option('-c, --config <path>', 'Path to tsconfig.json')
	.option('-o, --output <file>', 'Output file for report')
	.option('-f, --format <format>', 'Report format (console, json, html, markdown)', 'console')
	.option('-v, --verbose', 'Show verbose output', false)
	.option('--no-stats', 'Exclude statistics from report')
	.option('--no-circular', 'Exclude circular dependencies from report')
	.option('--top <number>', 'Show top N unused exports', '10')
	.option('--include <patterns...>', 'Include patterns for files to analyze')
	.option('--exclude <patterns...>', 'Exclude patterns for files to skip')
	.option('--entry-points <files...>', 'Entry point files')
	.option('--no-dynamic-imports', "Don't follow dynamic imports")
	.option('--no-types', "Don't analyze type usage")
	.option('--no-svelte', "Don't analyze Svelte components")
	.option('--include-tests', 'Include test files in analysis')
	.option('--remove-exports', 'Remove unused export statements from files')
	.action(async (directory, options) => {
		try {
			const rootDir = path.resolve(directory);
			const tsConfigPath = options.config ? path.resolve(options.config) : undefined;

			await analyzeProject(rootDir, {
				tsConfigPath,
				includePatterns: options.include,
				excludePatterns: options.exclude,
				entryPoints: options.entryPoints,
				followDynamicImports: !options.noDynamicImports,
				analyzeTypeUsage: !options.noTypes,
				includeSvelteComponents: !options.noSvelte,
				includeTestFiles: options.includeTests,
				format: options.format,
				verbose: options.verbose,
				includeStats: options.stats,
				includeCircularDeps: options.circular,
				showTopUnused: parseInt(options.top, 10),
				outputFile: options.output,
				removeExports: options.removeExports
			});
		} catch (error) {
			console.error('Error:', error);
			process.exit(1);
		}
	});

program
	.command('quick')
	.description('Quick analysis with default settings')
	.argument('[directory]', 'Project directory to analyze', process.cwd())
	.action(async (directory) => {
		try {
			const rootDir = path.resolve(directory);
			await analyzeProject(rootDir);
		} catch (error) {
			console.error('Error:', error);
			process.exit(1);
		}
	});

program.parse();
