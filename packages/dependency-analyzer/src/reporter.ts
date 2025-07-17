import chalk from 'chalk';
import * as fs from 'fs';
import * as path from 'path';
import type { AnalysisResult, DependencyNode, FileNode, AnalysisStats } from './types.js';

export interface ReportOptions {
	outputFile?: string;
	format: 'console' | 'json' | 'html' | 'markdown';
	verbose: boolean;
	includeStats: boolean;
	includeCircularDeps: boolean;
	showTopUnused: number;
}

export class Reporter {
	constructor(private options: ReportOptions) {}

	generateReport(result: AnalysisResult): string {
		switch (this.options.format) {
			case 'console':
				return this.generateConsoleReport(result);
			case 'json':
				return this.generateJsonReport(result);
			case 'html':
				return this.generateHtmlReport(result);
			case 'markdown':
				return this.generateMarkdownReport(result);
			default:
				throw new Error(`Unsupported format: ${this.options.format}`);
		}
	}

	saveReport(report: string) {
		if (this.options.outputFile) {
			fs.writeFileSync(this.options.outputFile, report);
			console.log(`Report saved to: ${this.options.outputFile}`);
		} else {
			console.log(report);
		}
	}

	private generateConsoleReport(result: AnalysisResult): string {
		const lines: string[] = [];

		// Header
		lines.push(chalk.bold.cyan('🔍 Dependency Analysis Report'));
		lines.push(chalk.gray('='.repeat(50)));
		lines.push('');

		// Statistics
		if (this.options.includeStats) {
			lines.push(chalk.bold.yellow('📊 Statistics'));
			lines.push(`Total Files: ${chalk.green(result.stats.totalFiles)}`);
			lines.push(`Total Symbols: ${chalk.green(result.stats.totalSymbols)}`);
			lines.push(`Total Dependencies: ${chalk.green(result.stats.totalDependencies)}`);
			lines.push(`Analysis Time: ${chalk.green(result.stats.analysisTime + 'ms')}`);
			lines.push('');
		}

		// Unused Exports
		if (result.unusedExports.length > 0) {
			lines.push(chalk.bold.red(`💀 Unused Exports (${result.unusedExports.length})`));

			const topUnused = result.unusedExports.slice(0, this.options.showTopUnused);
			for (const node of topUnused) {
				const filePath = this.getRelativePath(node.filePath);
				lines.push(
					`${chalk.red('•')} ${chalk.bold(node.name)} ${chalk.gray(`(${node.type})`)} in ${chalk.cyan(filePath)}:${node.metadata.startLine}`
				);

				if (this.options.verbose) {
					lines.push(`  ${chalk.gray('└─')} ${node.metadata.sourceText.split('\n')[0].trim()}`);
				}
			}

			if (result.unusedExports.length > this.options.showTopUnused) {
				lines.push(
					`${chalk.gray('...')} and ${result.unusedExports.length - this.options.showTopUnused} more`
				);
			}
			lines.push('');
		}

		// Unused Files
		if (result.unusedFiles.length > 0) {
			lines.push(chalk.bold.red(`📄 Unused Files (${result.unusedFiles.length})`));

			for (const file of result.unusedFiles) {
				const filePath = this.getRelativePath(file.filePath);
				lines.push(`${chalk.red('•')} ${chalk.cyan(filePath)}`);
			}
			lines.push('');
		}

		// Circular Dependencies
		if (this.options.includeCircularDeps && result.circularDependencies.length > 0) {
			lines.push(
				chalk.bold.yellow(`🔄 Circular Dependencies (${result.circularDependencies.length})`)
			);

			for (const cycle of result.circularDependencies) {
				const nodeNames = cycle.map((id) => {
					const node = result.graph.nodes.get(id);
					return node ? `${node.name} (${this.getRelativePath(node.filePath)})` : id;
				});
				lines.push(`${chalk.yellow('•')} ${nodeNames.join(' → ')}`);
			}
			lines.push('');
		}

		// Summary
		const savings = this.calculateSavings(result.unusedExports, result.unusedFiles);
		lines.push(chalk.bold.green('💡 Potential Savings'));
		lines.push(`Lines of Code: ${chalk.green(savings.linesOfCode)}`);
		lines.push(`Files: ${chalk.green(savings.filesCount)}`);
		lines.push('');

		// Recommendations
		lines.push(chalk.bold.blue('🚀 Recommendations'));
		if (result.unusedExports.length > 0) {
			lines.push(`${chalk.blue('•')} Remove ${result.unusedExports.length} unused exports`);
		}
		if (result.unusedFiles.length > 0) {
			lines.push(`${chalk.blue('•')} Delete ${result.unusedFiles.length} unused files`);
		}
		if (result.circularDependencies.length > 0) {
			lines.push(
				`${chalk.blue('•')} Resolve ${result.circularDependencies.length} circular dependencies`
			);
		}

		return lines.join('\n');
	}

	private generateJsonReport(result: AnalysisResult): string {
		const report = {
			stats: result.stats,
			unusedExports: result.unusedExports.map((node) => ({
				name: node.name,
				type: node.type,
				filePath: node.filePath,
				line: node.metadata.startLine,
				column: node.metadata.startColumn
			})),
			unusedFiles: result.unusedFiles.map((file) => ({
				filePath: file.filePath,
				lastModified: file.lastModified
			})),
			circularDependencies: result.circularDependencies.map((cycle) =>
				cycle.map((id) => {
					const node = result.graph.nodes.get(id);
					return node
						? {
								name: node.name,
								filePath: node.filePath
							}
						: { id };
				})
			),
			savings: this.calculateSavings(result.unusedExports, result.unusedFiles)
		};

		return JSON.stringify(report, null, 2);
	}

	private generateHtmlReport(result: AnalysisResult): string {
		const savings = this.calculateSavings(result.unusedExports, result.unusedFiles);

		return `
<!DOCTYPE html>
<html>
<head>
    <title>Dependency Analysis Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .header { color: #2c3e50; border-bottom: 2px solid #3498db; padding-bottom: 10px; }
        .section { margin: 20px 0; }
        .stat { display: inline-block; margin: 10px; padding: 10px; background: #ecf0f1; border-radius: 5px; }
        .unused-export { margin: 5px 0; padding: 5px; background: #ffebee; border-left: 3px solid #e74c3c; }
        .unused-file { margin: 5px 0; padding: 5px; background: #fff3e0; border-left: 3px solid #ff9800; }
        .circular { margin: 5px 0; padding: 5px; background: #fff9c4; border-left: 3px solid #fbc02d; }
        .code { font-family: monospace; background: #f5f5f5; padding: 2px 4px; border-radius: 3px; }
    </style>
</head>
<body>
    <h1 class="header">🔍 Dependency Analysis Report</h1>
    
    <div class="section">
        <h2>📊 Statistics</h2>
        <div class="stat">Total Files: <strong>${result.stats.totalFiles}</strong></div>
        <div class="stat">Total Symbols: <strong>${result.stats.totalSymbols}</strong></div>
        <div class="stat">Unused Exports: <strong>${result.stats.unusedExports}</strong></div>
        <div class="stat">Unused Files: <strong>${result.stats.unusedFiles}</strong></div>
        <div class="stat">Analysis Time: <strong>${result.stats.analysisTime}ms</strong></div>
    </div>

    <div class="section">
        <h2>💀 Unused Exports (${result.unusedExports.length})</h2>
        ${result.unusedExports
					.map(
						(node) => `
            <div class="unused-export">
                <strong>${node.name}</strong> <em>(${node.type})</em> 
                in <span class="code">${this.getRelativePath(node.filePath)}:${node.metadata.startLine}</span>
            </div>
        `
					)
					.join('')}
    </div>

    <div class="section">
        <h2>📄 Unused Files (${result.unusedFiles.length})</h2>
        ${result.unusedFiles
					.map(
						(file) => `
            <div class="unused-file">
                <span class="code">${this.getRelativePath(file.filePath)}</span>
            </div>
        `
					)
					.join('')}
    </div>

    <div class="section">
        <h2>💡 Potential Savings</h2>
        <div class="stat">Lines of Code: <strong>${savings.linesOfCode}</strong></div>
        <div class="stat">Files: <strong>${savings.filesCount}</strong></div>
    </div>
</body>
</html>`;
	}

	private generateMarkdownReport(result: AnalysisResult): string {
		const lines: string[] = [];
		const savings = this.calculateSavings(result.unusedExports, result.unusedFiles);

		lines.push('# 🔍 Dependency Analysis Report');
		lines.push('');

		// Statistics
		lines.push('## 📊 Statistics');
		lines.push('');
		lines.push(`| Metric | Value |`);
		lines.push(`|--------|--------|`);
		lines.push(`| Total Files | ${result.stats.totalFiles} |`);
		lines.push(`| Total Symbols | ${result.stats.totalSymbols} |`);
		lines.push(`| Total Dependencies | ${result.stats.totalDependencies} |`);
		lines.push(`| Unused Exports | ${result.stats.unusedExports} |`);
		lines.push(`| Unused Files | ${result.stats.unusedFiles} |`);
		lines.push(`| Analysis Time | ${result.stats.analysisTime}ms |`);
		lines.push('');

		// Unused Exports
		if (result.unusedExports.length > 0) {
			lines.push(`## 💀 Unused Exports (${result.unusedExports.length})`);
			lines.push('');

			for (const node of result.unusedExports) {
				const filePath = this.getRelativePath(node.filePath);
				lines.push(
					`- **${node.name}** _(${node.type})_ in \`${filePath}:${node.metadata.startLine}\``
				);
			}
			lines.push('');
		}

		// Unused Files
		if (result.unusedFiles.length > 0) {
			lines.push(`## 📄 Unused Files (${result.unusedFiles.length})`);
			lines.push('');

			for (const file of result.unusedFiles) {
				const filePath = this.getRelativePath(file.filePath);
				lines.push(`- \`${filePath}\``);
			}
			lines.push('');
		}

		// Circular Dependencies
		if (result.circularDependencies.length > 0) {
			lines.push(`## 🔄 Circular Dependencies (${result.circularDependencies.length})`);
			lines.push('');

			for (const cycle of result.circularDependencies) {
				const nodeNames = cycle.map((id) => {
					const node = result.graph.nodes.get(id);
					return node ? `${node.name}` : id;
				});
				lines.push(`- ${nodeNames.join(' → ')}`);
			}
			lines.push('');
		}

		// Potential Savings
		lines.push('## 💡 Potential Savings');
		lines.push('');
		lines.push(`- **Lines of Code**: ${savings.linesOfCode}`);
		lines.push(`- **Files**: ${savings.filesCount}`);
		lines.push('');

		// Recommendations
		lines.push('## 🚀 Recommendations');
		lines.push('');
		if (result.unusedExports.length > 0) {
			lines.push(`- Remove ${result.unusedExports.length} unused exports`);
		}
		if (result.unusedFiles.length > 0) {
			lines.push(`- Delete ${result.unusedFiles.length} unused files`);
		}
		if (result.circularDependencies.length > 0) {
			lines.push(`- Resolve ${result.circularDependencies.length} circular dependencies`);
		}

		return lines.join('\n');
	}

	private calculateSavings(
		unusedExports: DependencyNode[],
		unusedFiles: FileNode[]
	): {
		linesOfCode: number;
		filesCount: number;
	} {
		let linesOfCode = 0;

		// Count from unused exports
		for (const node of unusedExports) {
			const lines = node.metadata.endLine - node.metadata.startLine + 1;
			linesOfCode += lines;
		}

		// Estimate from unused files
		linesOfCode += unusedFiles.length * 50; // Average estimate

		return {
			linesOfCode,
			filesCount: unusedFiles.length
		};
	}

	private getRelativePath(filePath: string): string {
		const cwd = process.cwd();
		return path.relative(cwd, filePath);
	}
}
