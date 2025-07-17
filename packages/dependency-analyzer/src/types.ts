export interface DependencyNode {
	id: string;
	filePath: string;
	name: string;
	type: 'function' | 'class' | 'interface' | 'type' | 'variable' | 'component' | 'export';
	exportedAs?: string;
	importedFrom?: string;
	dependencies: Set<string>;
	dependents: Set<string>;
	isEntryPoint: boolean;
	isUsed: boolean;
	metadata: NodeMetadata;
}

export interface NodeMetadata {
	startLine: number;
	endLine: number;
	startColumn: number;
	endColumn: number;
	isDefaultExport: boolean;
	isNamedExport: boolean;
	isReExport: boolean;
	hasJSDoc: boolean;
	isPublic: boolean;
	sourceText: string;
}

export interface FileNode {
	filePath: string;
	imports: ImportInfo[];
	exports: ExportInfo[];
	symbols: SymbolInfo[];
	isEntryPoint: boolean;
	lastModified: number;
	dependencies: Set<string>;
	dependents: Set<string>;
	symbolUsages?: CrossFileSymbolUsage[];
}

export interface CrossFileSymbolUsage {
	symbolName: string;
	importPath: string;
	usage: SymbolUsage;
}

export interface ImportInfo {
	specifier: string;
	importPath: string;
	resolvedPath: string;
	imported: ImportedSymbol[];
	isTypeOnly: boolean;
	isDynamic: boolean;
}

export interface ImportedSymbol {
	name: string;
	alias?: string | undefined;
	isDefault: boolean;
	isNamespace: boolean;
}

export interface ExportInfo {
	name: string;
	type: 'function' | 'class' | 'interface' | 'type' | 'variable' | 'component';
	isDefault: boolean;
	isReExport: boolean;
	reExportFrom?: string | undefined;
	metadata: NodeMetadata;
}

export interface SymbolInfo {
	name: string;
	type: 'function' | 'class' | 'interface' | 'type' | 'variable' | 'component';
	isExported: boolean;
	usages: SymbolUsage[];
	metadata: NodeMetadata;
}

export interface SymbolUsage {
	filePath: string;
	line: number;
	column: number;
	context: 'import' | 'call' | 'reference' | 'type' | 'jsx' | 'sveltekit-framework';
}

export interface DependencyGraph {
	nodes: Map<string, DependencyNode>;
	files: Map<string, FileNode>;
	entryPoints: Set<string>;
	rootDir: string;
	tsConfigPath: string;
	pathMappings: Map<string, string[]>;
}

export interface AnalysisResult {
	graph: DependencyGraph;
	unusedExports: DependencyNode[];
	unusedFiles: FileNode[];
	circularDependencies: string[][];
	stats: AnalysisStats;
}

export interface AnalysisStats {
	totalFiles: number;
	totalSymbols: number;
	totalDependencies: number;
	unusedExports: number;
	unusedFiles: number;
	circularDependencies: number;
	analysisTime: number;
}

export interface AnalysisConfig {
	rootDir: string;
	tsConfigPath: string;
	includePatterns: string[];
	excludePatterns: string[];
	entryPoints: string[];
	followDynamicImports: boolean;
	analyzeTypeUsage: boolean;
	includeSvelteComponents: boolean;
	includeTestFiles: boolean;
	removeExports?: boolean;
}
