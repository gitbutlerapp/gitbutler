const SEPARATOR = '/';
const SEPARATOR_WIN = '\\';
const EXT_SEPARATOR = '.';

export function isWindowsPath(filePath: string): boolean {
	// This is a simple check, but it should be enough for our purposes.
	const trimmed = filePath.trim();
	return (
		trimmed.includes(SEPARATOR_WIN) &&
		!(trimmed.includes(`${SEPARATOR_WIN} `) && trimmed.includes(SEPARATOR))
	);
}

export function getSeparator(filePath: string): string {
	return isWindowsPath(filePath) ? SEPARATOR_WIN : SEPARATOR;
}

export function splitPath(filePath: string): string[] {
	const sep = getSeparator(filePath);
	return filePath.split(sep);
}

export interface FilePathInfo {
	fileName: string;
	extension: string;
	directoryPath: string;
}

export function getFilePathInfo(filePath: string): FilePathInfo | undefined {
	if (!filePath) return undefined;
	const parts = splitPath(filePath);
	if (parts.length === 0) return undefined;

	const fileName = parts[parts.length - 1]!;
	const hasExtension = fileName.includes(EXT_SEPARATOR);
	const extension = hasExtension ? fileName.split(EXT_SEPARATOR).pop() || '' : '';

	const sep = getSeparator(filePath);
	const directoryPath = parts.slice(0, parts.length - 1).join(sep);

	return { fileName, extension, directoryPath };
}
