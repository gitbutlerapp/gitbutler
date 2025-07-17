export interface IPathResolver {
	resolveImportPath(importPath: string, fromFile: string): string;
	resolveFileExtension(filePath: string): string;
	isExternal(importPath: string): boolean;
	normalizePath(filePath: string): string;
	getRelativePathFromRoot(filePath: string): string;
}
