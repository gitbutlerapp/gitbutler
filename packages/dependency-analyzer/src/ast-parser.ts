import * as ts from 'typescript';
import * as fs from 'fs';
import * as path from 'path';
import type { IPathResolver } from './path-resolver-interface.js';
import type {
	FileNode,
	ImportInfo,
	ExportInfo,
	SymbolInfo,
	NodeMetadata,
	SymbolUsage
} from './types.js';

export class ASTParser {
	private program!: ts.Program;
	private typeChecker!: ts.TypeChecker;
	private pathResolver: IPathResolver;

	constructor(
		private rootDir: string,
		private tsConfigPath: string,
		pathResolver: IPathResolver
	) {
		this.pathResolver = pathResolver;
		this.initializeTypeScript();
	}

	private initializeTypeScript() {
		const configFile = ts.readConfigFile(this.tsConfigPath, ts.sys.readFile);
		const parsedConfig = ts.parseJsonConfigFileContent(
			configFile.config,
			ts.sys,
			path.dirname(this.tsConfigPath)
		);

		this.program = ts.createProgram(parsedConfig.fileNames, parsedConfig.options);
		this.typeChecker = this.program.getTypeChecker();
	}

	parseFile(filePath: string): FileNode {
		const sourceFile = this.program.getSourceFile(filePath);
		if (!sourceFile) {
			throw new Error(`Could not find source file: ${filePath}`);
		}

		const fileNode: FileNode = {
			filePath,
			imports: [],
			exports: [],
			symbols: [],
			isEntryPoint: false,
			lastModified: fs.statSync(filePath).mtime.getTime(),
			dependencies: new Set(),
			dependents: new Set()
		};

		// Parse imports, exports, and symbols
		this.visitNode(sourceFile, fileNode);

		// Post-process: Link export declarations with their corresponding symbols
		this.linkExportsWithSymbols(fileNode);

		return fileNode;
	}

	private visitNode(node: ts.Node, fileNode: FileNode) {
		switch (node.kind) {
			case ts.SyntaxKind.ImportDeclaration:
				this.parseImportDeclaration(node as ts.ImportDeclaration, fileNode);
				break;
			case ts.SyntaxKind.ExportDeclaration:
				this.parseExportDeclaration(node as ts.ExportDeclaration, fileNode);
				break;
			case ts.SyntaxKind.FunctionDeclaration:
				this.parseFunctionDeclaration(node as ts.FunctionDeclaration, fileNode);
				break;
			case ts.SyntaxKind.ClassDeclaration:
				this.parseClassDeclaration(node as ts.ClassDeclaration, fileNode);
				break;
			case ts.SyntaxKind.InterfaceDeclaration:
				this.parseInterfaceDeclaration(node as ts.InterfaceDeclaration, fileNode);
				break;
			case ts.SyntaxKind.TypeAliasDeclaration:
				this.parseTypeAliasDeclaration(node as ts.TypeAliasDeclaration, fileNode);
				break;
			case ts.SyntaxKind.VariableStatement:
				this.parseVariableStatement(node as ts.VariableStatement, fileNode);
				break;
			case ts.SyntaxKind.CallExpression:
				this.parseCallExpression(node as ts.CallExpression, fileNode);
				break;
			case ts.SyntaxKind.NewExpression:
				this.parseNewExpression(node as ts.NewExpression, fileNode);
				break;
			case ts.SyntaxKind.Identifier:
				this.parseIdentifier(node as ts.Identifier, fileNode);
				break;
		}

		// Recursively visit child nodes
		ts.forEachChild(node, (child) => this.visitNode(child, fileNode));
	}

	private parseImportDeclaration(node: ts.ImportDeclaration, fileNode: FileNode) {
		const moduleSpecifier = node.moduleSpecifier;
		if (!ts.isStringLiteral(moduleSpecifier)) return;

		const importPath = moduleSpecifier.text;
		const resolvedPath = this.pathResolver.resolveImportPath(importPath, fileNode.filePath);

		const importInfo: ImportInfo = {
			specifier: importPath,
			importPath,
			resolvedPath,
			imported: [],
			isTypeOnly: node.importClause?.isTypeOnly || false,
			isDynamic: false
		};

		// Parse import clause
		if (node.importClause) {
			const importClause = node.importClause;

			// Default import
			if (importClause.name) {
				importInfo.imported.push({
					name: importClause.name.text,
					isDefault: true,
					isNamespace: false
				});
			}

			// Named imports
			if (importClause.namedBindings) {
				if (ts.isNamespaceImport(importClause.namedBindings)) {
					// import * as name
					importInfo.imported.push({
						name: importClause.namedBindings.name.text,
						isDefault: false,
						isNamespace: true
					});
				} else if (ts.isNamedImports(importClause.namedBindings)) {
					// import { a, b as c }
					for (const element of importClause.namedBindings.elements) {
						importInfo.imported.push({
							name: element.name.text,
							alias: element.propertyName?.text || undefined,
							isDefault: false,
							isNamespace: false
						});
					}
				}
			}
		}

		fileNode.imports.push(importInfo);
		if (!this.pathResolver.isExternal(importPath)) {
			fileNode.dependencies.add(resolvedPath);
		}
	}

	private parseExportDeclaration(node: ts.ExportDeclaration, fileNode: FileNode) {
		const moduleSpecifier = node.moduleSpecifier;

		if (moduleSpecifier && ts.isStringLiteral(moduleSpecifier)) {
			// Re-export: export { a } from './module'
			const importPath = moduleSpecifier.text;
			const resolvedPath = this.pathResolver.resolveImportPath(importPath, fileNode.filePath);

			if (node.exportClause && ts.isNamedExports(node.exportClause)) {
				for (const element of node.exportClause.elements) {
					const exportInfo: ExportInfo = {
						name: element.name.text,
						type: 'variable', // Will be refined later
						isDefault: false,
						isReExport: true,
						reExportFrom: resolvedPath,
						metadata: this.getNodeMetadata(element)
					};
					fileNode.exports.push(exportInfo);
				}
			}
		} else if (node.exportClause && ts.isNamedExports(node.exportClause)) {
			// Named exports: export { a, b }
			for (const element of node.exportClause.elements) {
				const exportInfo: ExportInfo = {
					name: element.name.text,
					type: 'variable', // Will be refined later
					isDefault: false,
					isReExport: false,
					metadata: this.getNodeMetadata(element)
				};
				fileNode.exports.push(exportInfo);
			}
		}
	}

	private parseFunctionDeclaration(node: ts.FunctionDeclaration, fileNode: FileNode) {
		if (!node.name) return;

		const name = node.name.text;
		const isExported = this.hasExportModifier(node);
		const isDefaultExport = this.hasDefaultExportModifier(node);

		const symbolInfo: SymbolInfo = {
			name,
			type: 'function',
			isExported,
			usages: [],
			metadata: this.getNodeMetadata(node)
		};

		fileNode.symbols.push(symbolInfo);

		if (isExported) {
			const exportInfo: ExportInfo = {
				name,
				type: 'function',
				isDefault: isDefaultExport,
				isReExport: false,
				metadata: this.getNodeMetadata(node)
			};
			fileNode.exports.push(exportInfo);
		}
	}

	private parseClassDeclaration(node: ts.ClassDeclaration, fileNode: FileNode) {
		if (!node.name) return;

		const name = node.name.text;
		const isExported = this.hasExportModifier(node);
		const isDefaultExport = this.hasDefaultExportModifier(node);

		const symbolInfo: SymbolInfo = {
			name,
			type: 'class',
			isExported,
			usages: [],
			metadata: this.getNodeMetadata(node)
		};

		fileNode.symbols.push(symbolInfo);

		if (isExported) {
			const exportInfo: ExportInfo = {
				name,
				type: 'class',
				isDefault: isDefaultExport,
				isReExport: false,
				metadata: this.getNodeMetadata(node)
			};
			fileNode.exports.push(exportInfo);
		}
	}

	private parseInterfaceDeclaration(node: ts.InterfaceDeclaration, fileNode: FileNode) {
		const name = node.name.text;
		const isExported = this.hasExportModifier(node);

		const symbolInfo: SymbolInfo = {
			name,
			type: 'interface',
			isExported,
			usages: [],
			metadata: this.getNodeMetadata(node)
		};

		fileNode.symbols.push(symbolInfo);

		if (isExported) {
			const exportInfo: ExportInfo = {
				name,
				type: 'interface',
				isDefault: false,
				isReExport: false,
				metadata: this.getNodeMetadata(node)
			};
			fileNode.exports.push(exportInfo);
		}
	}

	private parseTypeAliasDeclaration(node: ts.TypeAliasDeclaration, fileNode: FileNode) {
		const name = node.name.text;
		const isExported = this.hasExportModifier(node);

		const symbolInfo: SymbolInfo = {
			name,
			type: 'type',
			isExported,
			usages: [],
			metadata: this.getNodeMetadata(node)
		};

		fileNode.symbols.push(symbolInfo);

		if (isExported) {
			const exportInfo: ExportInfo = {
				name,
				type: 'type',
				isDefault: false,
				isReExport: false,
				metadata: this.getNodeMetadata(node)
			};
			fileNode.exports.push(exportInfo);
		}
	}

	private parseVariableStatement(node: ts.VariableStatement, fileNode: FileNode) {
		const isExported = this.hasExportModifier(node);
		const isDefaultExport = this.hasDefaultExportModifier(node);

		for (const declaration of node.declarationList.declarations) {
			if (ts.isIdentifier(declaration.name)) {
				const name = declaration.name.text;

				const symbolInfo: SymbolInfo = {
					name,
					type: 'variable',
					isExported,
					usages: [],
					metadata: this.getNodeMetadata(declaration)
				};

				fileNode.symbols.push(symbolInfo);

				if (isExported) {
					const exportInfo: ExportInfo = {
						name,
						type: 'variable',
						isDefault: isDefaultExport,
						isReExport: false,
						metadata: this.getNodeMetadata(declaration)
					};
					fileNode.exports.push(exportInfo);
				}
			}
		}
	}

	private parseCallExpression(node: ts.CallExpression, fileNode: FileNode) {
		// Handle dynamic imports
		if (node.expression.kind === ts.SyntaxKind.ImportKeyword) {
			const arg = node.arguments[0];
			if (ts.isStringLiteral(arg)) {
				const importPath = arg.text;
				const resolvedPath = this.pathResolver.resolveImportPath(importPath, fileNode.filePath);

				const importInfo: ImportInfo = {
					specifier: importPath,
					importPath,
					resolvedPath,
					imported: [],
					isTypeOnly: false,
					isDynamic: true
				};

				fileNode.imports.push(importInfo);
				if (!this.pathResolver.isExternal(importPath)) {
					fileNode.dependencies.add(resolvedPath);
				}
			}
		}

		// Handle function calls - track usage of imported/local functions
		if (ts.isIdentifier(node.expression)) {
			const functionName = node.expression.text;
			this.trackSymbolUsage(functionName, fileNode, node);
		}

		// Handle method calls - track usage of object methods
		if (ts.isPropertyAccessExpression(node.expression)) {
			const objectName = ts.isIdentifier(node.expression.expression)
				? node.expression.expression.text
				: undefined;
			const methodName = node.expression.name.text;

			if (objectName) {
				this.trackSymbolUsage(objectName, fileNode, node);
			}
			this.trackSymbolUsage(methodName, fileNode, node);
		}
	}

	private parseNewExpression(node: ts.NewExpression, fileNode: FileNode) {
		// Handle constructor calls - track usage of classes
		if (ts.isIdentifier(node.expression)) {
			const className = node.expression.text;
			this.trackSymbolUsage(className, fileNode, node);
		}
	}

	private parseIdentifier(node: ts.Identifier, fileNode: FileNode) {
		// Skip identifiers that are:
		// 1. Part of declarations (handled elsewhere)
		// 2. Property names in object literals
		// 3. Import/export names (handled elsewhere)
		// 4. Function/class/variable names in their declarations

		const parent = node.parent;
		if (!parent) return;

		// Skip if this identifier is the name being declared
		if (ts.isFunctionDeclaration(parent) && parent.name === node) return;
		if (ts.isClassDeclaration(parent) && parent.name === node) return;
		if (ts.isVariableDeclaration(parent) && parent.name === node) return;
		if (ts.isInterfaceDeclaration(parent) && parent.name === node) return;
		if (ts.isTypeAliasDeclaration(parent) && parent.name === node) return;
		if (ts.isImportSpecifier(parent) && (parent.name === node || parent.propertyName === node))
			return;
		if (ts.isExportSpecifier(parent) && (parent.name === node || parent.propertyName === node))
			return;

		// Skip if this identifier is a property name in an object literal
		if (ts.isPropertyAssignment(parent) && parent.name === node) return;
		if (ts.isPropertyAccessExpression(parent) && parent.name === node) return;

		// Skip if this identifier is part of a call expression that's already handled
		if (ts.isCallExpression(parent) && parent.expression === node) return;
		if (ts.isNewExpression(parent) && parent.expression === node) return;

		// Track usage of this identifier
		this.trackSymbolUsage(node.text, fileNode, node);
	}

	private trackSymbolUsage(symbolName: string, fileNode: FileNode, usageNode: ts.Node) {
		const sourceFile = usageNode.getSourceFile();
		const startPos = sourceFile.getLineAndCharacterOfPosition(usageNode.getStart());

		const usage: SymbolUsage = {
			filePath: fileNode.filePath,
			line: startPos.line + 1,
			column: startPos.character + 1,
			context: 'call'
		};

		// Find if this symbol exists in the current file (local symbol)
		const localSymbol = fileNode.symbols.find((sym) => sym.name === symbolName);
		if (localSymbol) {
			localSymbol.usages.push(usage);
			return; // Found local symbol, no need to check imports
		}

		// Find if this symbol is imported from another file
		for (const importInfo of fileNode.imports) {
			const importedSymbol = importInfo.imported.find((sym) => sym.name === symbolName);
			if (importedSymbol) {
				// Store usage information that will be processed later by the graph builder
				// to connect this usage to the actual exported symbol
				if (!fileNode.symbolUsages) {
					fileNode.symbolUsages = [];
				}
				fileNode.symbolUsages.push({
					symbolName: symbolName,
					importPath: importInfo.resolvedPath,
					usage: usage
				});

				// Ensure dependency is tracked
				if (!this.pathResolver.isExternal(importInfo.importPath)) {
					fileNode.dependencies.add(importInfo.resolvedPath);
				}
				break;
			}
		}
	}

	private hasExportModifier(node: ts.Node): boolean {
		return (
			(ts.canHaveModifiers(node) &&
				ts.getModifiers(node)?.some((mod) => mod.kind === ts.SyntaxKind.ExportKeyword)) ||
			false
		);
	}

	private hasDefaultExportModifier(node: ts.Node): boolean {
		return (
			(ts.canHaveModifiers(node) &&
				ts.getModifiers(node)?.some((mod) => mod.kind === ts.SyntaxKind.DefaultKeyword)) ||
			false
		);
	}

	private getNodeMetadata(node: ts.Node): NodeMetadata {
		const sourceFile = node.getSourceFile();
		const start = node.getStart();
		const end = node.getEnd();
		const startPos = sourceFile.getLineAndCharacterOfPosition(start);
		const endPos = sourceFile.getLineAndCharacterOfPosition(end);

		return {
			startLine: startPos.line + 1,
			endLine: endPos.line + 1,
			startColumn: startPos.character + 1,
			endColumn: endPos.character + 1,
			isDefaultExport: this.hasDefaultExportModifier(node),
			isNamedExport: this.hasExportModifier(node),
			isReExport: false,
			hasJSDoc: ts.getJSDocCommentsAndTags(node).length > 0,
			isPublic: true,
			sourceText: node.getText()
		};
	}

	/**
	 * Links export declarations with their corresponding symbols.
	 * This handles cases like `function foo() {}` followed by `export { foo }`.
	 */
	private linkExportsWithSymbols(fileNode: FileNode) {
		// Find exports that are not re-exports (local exports)
		const localExports = fileNode.exports.filter((exp) => !exp.isReExport);

		for (const exportInfo of localExports) {
			// Find the corresponding symbol
			const symbol = fileNode.symbols.find((sym) => sym.name === exportInfo.name);
			if (symbol && !symbol.isExported) {
				// Mark the symbol as exported
				symbol.isExported = true;
			}
		}
	}
}
