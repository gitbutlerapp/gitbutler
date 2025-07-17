import * as ts from 'typescript';
import * as fs from 'fs';
import type { IPathResolver } from './path-resolver-interface.js';
import type { FileNode, ImportInfo, ExportInfo, SymbolInfo, NodeMetadata } from './types.js';

export class SvelteParser {
	constructor(private pathResolver: IPathResolver) {}

	parseFile(filePath: string): FileNode {
		const content = fs.readFileSync(filePath, 'utf-8');
		const lastModified = fs.statSync(filePath).mtime.getTime();

		const fileNode: FileNode = {
			filePath,
			imports: [],
			exports: [],
			symbols: [],
			isEntryPoint: false,
			lastModified,
			dependencies: new Set(),
			dependents: new Set()
		};

		// Parse script sections
		this.parseScriptSections(content, fileNode);

		// Parse script content with TypeScript AST for better usage tracking
		this.parseScriptUsage(content, fileNode);

		// Parse template dependencies (component usage)
		this.parseTemplateDependencies(content, fileNode);

		// Add component as default export
		const componentName = this.getComponentName(filePath);
		const exportInfo: ExportInfo = {
			name: componentName,
			type: 'component',
			isDefault: true,
			isReExport: false,
			metadata: this.createMetadata(0, 0, content)
		};
		fileNode.exports.push(exportInfo);

		const symbolInfo: SymbolInfo = {
			name: componentName,
			type: 'component',
			isExported: true,
			usages: [],
			metadata: this.createMetadata(0, 0, content)
		};
		fileNode.symbols.push(symbolInfo);

		return fileNode;
	}

	private parseScriptSections(content: string, fileNode: FileNode) {
		// Match both <script> and <script lang="ts">
		const scriptRegex = /<script[^>]*>([\s\S]*?)<\/script>/g;
		let match;

		while ((match = scriptRegex.exec(content)) !== null) {
			const scriptContent = match[1];
			this.parseScriptContent(scriptContent, fileNode);
		}
	}

	private parseScriptContent(scriptContent: string, fileNode: FileNode) {
		// Parse imports - comprehensive regex to handle all TypeScript import patterns
		const importRegex =
			/import\s+(?:(type)\s+)?(?:(\w+)(?:\s*,\s*)?)?(?:\{([^}]+)\})?(?:\*\s+as\s+(\w+))?\s+from\s+['"]([^'"]+)['"]|import\s+['"]([^'"]+)['"]/g;
		let importMatch;

		while ((importMatch = importRegex.exec(scriptContent)) !== null) {
			const [
				fullMatch,
				typeKeyword,
				defaultImport,
				namedImports,
				namespaceImport,
				importPath,
				sideEffectImportPath
			] = importMatch;
			const actualImportPath = importPath || sideEffectImportPath;

			// Skip if we don't have a valid import path
			if (!actualImportPath) continue;

			const resolvedPath = this.pathResolver.resolveImportPath(actualImportPath, fileNode.filePath);

			const importInfo: ImportInfo = {
				specifier: actualImportPath,
				importPath: actualImportPath,
				resolvedPath,
				imported: [],
				isTypeOnly: !!typeKeyword || fullMatch.includes('type'),
				isDynamic: false
			};

			if (defaultImport) {
				importInfo.imported.push({
					name: defaultImport,
					isDefault: true,
					isNamespace: false
				});
			}

			if (namedImports) {
				const imports = namedImports.split(',').map((imp) => imp.trim());
				for (const imp of imports) {
					const [name, alias] = imp.split(' as ').map((s) => s.trim());

					// Handle 'type SomeName' imports - strip the 'type ' prefix
					const isTypeImport = name.startsWith('type ');
					const cleanName = isTypeImport ? name.substring(5) : name;

					importInfo.imported.push({
						name: alias || cleanName,
						alias: alias ? cleanName : undefined,
						isDefault: false,
						isNamespace: false
					});
				}
			}

			if (namespaceImport) {
				const name = namespaceImport.replace('*', '').replace('as', '').trim();
				importInfo.imported.push({
					name,
					isDefault: false,
					isNamespace: true
				});
			}

			fileNode.imports.push(importInfo);
			if (!this.pathResolver.isExternal(importPath)) {
				fileNode.dependencies.add(resolvedPath);
			}
		}

		// Parse exports
		const exportRegex = /export\s+(?:const|let|var|function|class)\s+(\w+)/g;
		let exportMatch;

		while ((exportMatch = exportRegex.exec(scriptContent)) !== null) {
			const [, name] = exportMatch;

			const exportInfo: ExportInfo = {
				name,
				type: 'variable', // Simplified for now
				isDefault: false,
				isReExport: false,
				metadata: this.createMetadata(0, 0, scriptContent)
			};
			fileNode.exports.push(exportInfo);

			const symbolInfo: SymbolInfo = {
				name,
				type: 'variable',
				isExported: true,
				usages: [],
				metadata: this.createMetadata(0, 0, scriptContent)
			};
			fileNode.symbols.push(symbolInfo);
		}

		// Parse getContext calls (GitButler pattern)
		const getContextRegex = /getContext\(([^)]+)\)/g;
		let contextMatch;

		while ((contextMatch = getContextRegex.exec(scriptContent)) !== null) {
			const contextName = contextMatch[1].replace(/['"`]/g, '');
			// This creates an implicit dependency on the context provider
			// We'll handle this in the dependency graph builder
		}
	}

	private parseTemplateDependencies(content: string, fileNode: FileNode) {
		// Parse component usage in template - improved regex to handle self-closing and nested tags
		const componentRegex = /<(\w+)(?:\s[^>]*)?(?:>[\s\S]*?<\/\1>|\/?>)/g;
		let componentMatch;

		const usedComponents = new Set<string>();

		while ((componentMatch = componentRegex.exec(content)) !== null) {
			const componentName = componentMatch[1];

			// Skip HTML elements
			if (this.isHtmlElement(componentName)) continue;

			// Skip if already processed
			if (usedComponents.has(componentName)) continue;
			usedComponents.add(componentName);

			// Find corresponding import
			const importInfo = fileNode.imports.find((imp) =>
				imp.imported.some((sym) => sym.name === componentName)
			);

			if (importInfo) {
				// Ensure this component dependency is tracked
				const resolvedPath = this.pathResolver.resolveFileExtension(importInfo.resolvedPath);
				if (!this.pathResolver.isExternal(importInfo.importPath)) {
					fileNode.dependencies.add(resolvedPath);
				}

				// Add usage information
				const symbol = fileNode.symbols.find((sym) => sym.name === componentName);
				if (symbol) {
					symbol.usages.push({
						filePath: fileNode.filePath,
						line: this.getLineNumber(content, componentMatch.index),
						column: componentMatch.index,
						context: 'jsx'
					});
				}
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
			'footer'
		]);
		return htmlElements.has(tagName.toLowerCase());
	}

	private getComponentName(filePath: string): string {
		const basename = filePath.split('/').pop() || '';
		return basename.replace(/\.svelte$/, '');
	}

	private getLineNumber(content: string, index: number): number {
		return content.substring(0, index).split('\n').length;
	}

	private createMetadata(startLine: number, endLine: number, content: string): NodeMetadata {
		return {
			startLine,
			endLine,
			startColumn: 0,
			endColumn: 0,
			isDefaultExport: true,
			isNamedExport: false,
			isReExport: false,
			hasJSDoc: false,
			isPublic: true,
			sourceText: content
		};
	}

	private parseScriptUsage(content: string, fileNode: FileNode) {
		// Extract script content and parse it with TypeScript AST for better usage tracking
		const scriptRegex = /<script[^>]*>([\s\S]*?)<\/script>/g;
		let match;

		while ((match = scriptRegex.exec(content)) !== null) {
			const scriptContent = match[1];
			this.parseScriptContentWithAST(scriptContent, fileNode);
		}
	}

	private parseScriptContentWithAST(scriptContent: string, fileNode: FileNode) {
		try {
			// Create a temporary TypeScript source file
			const sourceFile = ts.createSourceFile(
				`${fileNode.filePath}.temp.ts`,
				scriptContent,
				ts.ScriptTarget.Latest,
				true
			);

			// Track symbol usage in the script content
			this.visitScriptNodeForUsage(sourceFile, fileNode);
		} catch (error) {
			// If TypeScript parsing fails, fall back to basic tracking
			console.warn(
				`Failed to parse script content with TypeScript AST in ${fileNode.filePath}:`,
				error
			);
		}
	}

	private visitScriptNodeForUsage(node: ts.Node, fileNode: FileNode) {
		switch (node.kind) {
			case ts.SyntaxKind.CallExpression:
				this.parseCallExpressionUsage(node as ts.CallExpression, fileNode);
				break;
			case ts.SyntaxKind.NewExpression:
				this.parseNewExpressionUsage(node as ts.NewExpression, fileNode);
				break;
			case ts.SyntaxKind.Identifier:
				this.parseIdentifierUsage(node as ts.Identifier, fileNode);
				break;
		}

		// Recursively visit child nodes
		ts.forEachChild(node, (child) => this.visitScriptNodeForUsage(child, fileNode));
	}

	private parseCallExpressionUsage(node: ts.CallExpression, fileNode: FileNode) {
		// Handle function calls - track usage of imported/local functions
		if (ts.isIdentifier(node.expression)) {
			const functionName = node.expression.text;
			this.trackSymbolUsageInScript(functionName, fileNode, node);
		}

		// Handle method calls - track usage of object methods
		if (ts.isPropertyAccessExpression(node.expression)) {
			const objectName = ts.isIdentifier(node.expression.expression)
				? node.expression.expression.text
				: undefined;

			if (objectName) {
				this.trackSymbolUsageInScript(objectName, fileNode, node);
			}
		}
	}

	private parseNewExpressionUsage(node: ts.NewExpression, fileNode: FileNode) {
		// Handle constructor calls - track usage of classes
		if (ts.isIdentifier(node.expression)) {
			const className = node.expression.text;
			this.trackSymbolUsageInScript(className, fileNode, node);
		}
	}

	private parseIdentifierUsage(node: ts.Identifier, fileNode: FileNode) {
		// Skip identifiers that are part of declarations or property access
		const parent = node.parent;
		if (!parent) return;

		// Skip if this identifier is the name being declared
		if (ts.isFunctionDeclaration(parent) && parent.name === node) return;
		if (ts.isClassDeclaration(parent) && parent.name === node) return;
		if (ts.isVariableDeclaration(parent) && parent.name === node) return;
		if (ts.isImportSpecifier(parent) && (parent.name === node || parent.propertyName === node))
			return;
		if (ts.isExportSpecifier(parent) && (parent.name === node || parent.propertyName === node))
			return;

		// Skip if this identifier is a property name
		if (ts.isPropertyAssignment(parent) && parent.name === node) return;
		if (ts.isPropertyAccessExpression(parent) && parent.name === node) return;

		// Skip if this identifier is the function being called (but not if it's an argument)
		if (ts.isCallExpression(parent) && parent.expression === node) return;
		if (ts.isNewExpression(parent) && parent.expression === node) return;

		// Track usage of this identifier
		this.trackSymbolUsageInScript(node.text, fileNode, node);
	}

	private trackSymbolUsageInScript(symbolName: string, fileNode: FileNode, usageNode: ts.Node) {
		const sourceFile = usageNode.getSourceFile();
		const startPos = sourceFile.getLineAndCharacterOfPosition(usageNode.getStart());

		const usage = {
			filePath: fileNode.filePath,
			line: startPos.line + 1,
			column: startPos.character + 1,
			context: 'call' as const
		};

		// Find if this symbol exists in the current file (local symbol)
		const localSymbol = fileNode.symbols.find((sym) => sym.name === symbolName);
		if (localSymbol) {
			localSymbol.usages.push(usage);
			return;
		}

		// Find if this symbol is imported from another file
		for (const importInfo of fileNode.imports) {
			const importedSymbol = importInfo.imported.find((sym) => sym.name === symbolName);
			if (importedSymbol) {
				// Store usage information for cross-file processing
				if (!fileNode.symbolUsages) {
					fileNode.symbolUsages = [];
				}
				fileNode.symbolUsages.push({
					symbolName: symbolName,
					importPath: importInfo.resolvedPath,
					usage: usage
				});

				if (!this.pathResolver.isExternal(importInfo.importPath)) {
					fileNode.dependencies.add(importInfo.resolvedPath);
				}
				break;
			}
		}
	}
}
