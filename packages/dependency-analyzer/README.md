# @gitbutler/dependency-analyzer

A robust TypeScript dependency graph analyzer for dead code detection. Built specifically for complex TypeScript codebases with Svelte components, tsconfig path mappings, and modern project structures.

## Features

- 🔍 **Deep AST Analysis** - Analyzes TypeScript AST for precise dependency tracking
- 🎯 **Path Resolution** - Handles complex tsconfig path mappings and module resolution
- 🧩 **Svelte Support** - First-class support for Svelte components and their dependencies
- 💀 **Dead Code Detection** - Mark-and-sweep algorithm to find unused exports and files
- 🔄 **Circular Dependencies** - Detects circular dependencies in your codebase
- 📊 **Rich Reporting** - Multiple output formats (console, JSON, HTML, markdown)
- ⚡ **Performance** - Efficient analysis even for large codebases

## Installation

```bash
npm install @gitbutler/dependency-analyzer
```

## Usage

### Command Line Interface

```bash
# Quick analysis
npx dependency-analyzer quick

# Full analysis with options
npx dependency-analyzer analyze ./apps/desktop \
  --config ./apps/desktop/tsconfig.json \
  --format html \
  --output report.html \
  --verbose

# Analyze specific patterns
npx dependency-analyzer analyze . \
  --include "src/**/*.ts" "src/**/*.svelte" \
  --exclude "**/*.test.*" \
  --entry-points "src/main.ts" "src/routes/+layout.ts"
```

### Programmatic API

```typescript
import { DependencyAnalyzer } from '@gitbutler/dependency-analyzer';

const analyzer = new DependencyAnalyzer({
	rootDir: './apps/desktop',
	tsConfigPath: './apps/desktop/tsconfig.json',
	includePatterns: ['src/**/*.ts', 'src/**/*.svelte'],
	excludePatterns: ['**/*.test.*'],
	entryPoints: ['src/main.ts']
});

// Get analysis results
const result = await analyzer.analyze();
console.log(`Found ${result.unusedExports.length} unused exports`);

// Generate report
await analyzer.generateReport({
	format: 'html',
	outputFile: 'dependency-report.html',
	verbose: true
});
```

### Quick Analysis Function

```typescript
import { analyzeProject } from '@gitbutler/dependency-analyzer';

await analyzeProject('./apps/desktop', {
	format: 'console',
	verbose: true,
	includeStats: true
});
```

## Configuration

### Analysis Config

```typescript
interface AnalysisConfig {
	rootDir: string; // Project root directory
	tsConfigPath: string; // Path to tsconfig.json
	includePatterns: string[]; // Glob patterns for files to analyze
	excludePatterns: string[]; // Glob patterns for files to exclude
	entryPoints: string[]; // Entry point files
	followDynamicImports: boolean; // Follow dynamic import() calls
	analyzeTypeUsage: boolean; // Analyze TypeScript type usage
	includeSvelteComponents: boolean; // Include Svelte components
	includeTestFiles: boolean; // Include test files
}
```

### Report Options

```typescript
interface ReportOptions {
	outputFile?: string; // Output file path
	format: 'console' | 'json' | 'html' | 'markdown';
	verbose: boolean; // Show detailed information
	includeStats: boolean; // Include analysis statistics
	includeCircularDeps: boolean; // Include circular dependencies
	showTopUnused: number; // Number of top unused exports to show
}
```

## Output Examples

### Console Output

```
🔍 Dependency Analysis Report
==================================================

📊 Statistics
Total Files: 156
Total Symbols: 1,247
Total Dependencies: 3,891
Analysis Time: 2,341ms

💀 Unused Exports (23)
• calculateMetrics (function) in src/lib/utils/metrics.ts:45
• UserPreferences (interface) in src/lib/types/user.ts:12
• formatDate (function) in src/lib/utils/date.ts:8
...

📄 Unused Files (5)
• src/lib/legacy/oldUtils.ts
• src/components/deprecated/OldModal.svelte
...

💡 Potential Savings
Lines of Code: 847
Files: 5

🚀 Recommendations
• Remove 23 unused exports
• Delete 5 unused files
• Resolve 2 circular dependencies
```

### JSON Output

```json
{
	"stats": {
		"totalFiles": 156,
		"totalSymbols": 1247,
		"unusedExports": 23,
		"unusedFiles": 5,
		"analysisTime": 2341
	},
	"unusedExports": [
		{
			"name": "calculateMetrics",
			"type": "function",
			"filePath": "src/lib/utils/metrics.ts",
			"line": 45,
			"column": 1
		}
	],
	"unusedFiles": [
		{
			"filePath": "src/lib/legacy/oldUtils.ts",
			"lastModified": 1640995200000
		}
	],
	"savings": {
		"linesOfCode": 847,
		"filesCount": 5
	}
}
```

## Architecture

The analyzer uses a multi-phase approach:

1. **File Discovery** - Glob patterns find all relevant files
2. **AST Parsing** - TypeScript compiler API extracts symbols and dependencies
3. **Path Resolution** - Resolves imports using tsconfig path mappings
4. **Graph Construction** - Builds dependency graph with nodes and edges
5. **Dead Code Analysis** - Mark-and-sweep algorithm from entry points
6. **Reporting** - Generates reports in various formats

## Advanced Usage

### Custom Entry Points

```typescript
const analyzer = new DependencyAnalyzer({
	rootDir: './apps/desktop',
	entryPoints: [
		'src/main.ts',
		'src/routes/+layout.ts',
		'src/hooks.client.ts',
		'src/service-worker.ts'
	]
});
```

### GitButler-Specific Patterns

The analyzer includes special handling for GitButler patterns:

- **Svelte Context Injection**: Tracks `getContext()` calls
- **Redux Toolkit Query**: Understands RTK Query patterns
- **Service Dependencies**: Handles dependency injection patterns

### Integration with CI/CD

```yaml
# .github/workflows/dead-code-check.yml
name: Dead Code Check
on: [push, pull_request]

jobs:
  dead-code:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
        with:
          node-version: '18'
      - run: npm ci
      - run: npx dependency-analyzer analyze --format json --output dead-code-report.json
      - run: |
          if [ $(jq '.stats.unusedExports' dead-code-report.json) -gt 0 ]; then
            echo "Found unused exports"
            exit 1
          fi
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Run the linter and tests
6. Submit a pull request

## License

MIT License - see LICENSE file for details.
